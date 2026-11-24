//! §§6.2, 6.4 and 9.1. Feed hand-written wire records to the reader so a
//! matching mistake in the production serializer cannot validate the reader.
use super::{Fixture, TempDir, expect_code, jsonl};
use os_agent_core::wal::WalStore;
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

const TS: &str = "2026-01-01T00:00:00Z";
fn section(kind: &str, turn: u64, version: u32) -> Value {
    let mut row = json!({"type":kind,"ts":TS,"ir_version":version,"config":{"stderr":"auto","approval":"ask"}});
    if kind == "checkpoint" {
        row["turn_id"] = json!(turn);
    }
    row
}
fn msg(role: &str, tid: u64, text: &str) -> Value {
    json!({"type":"message","role":role,"turn_id":tid,"ts":TS,"text":text})
}
fn lines(rows: &[Value]) -> Vec<u8> {
    rows.iter()
        .flat_map(|r| {
            let mut bytes = serde_json::to_vec(r).unwrap();
            bytes.push(b'\n');
            bytes
        })
        .collect()
}
fn replay(path: &Path) -> os_agent_core::wal::ReplayedSession {
    WalStore::open(path).unwrap().load_active_window().unwrap()
}

// RFC §6.2
#[test]
fn every_partial_byte_write_of_a_final_record_fails_without_modifying_history() {
    let dir = TempDir::new();
    let path = dir.0.join("wal");
    let prefix = lines(&[section("session_init", 0, 1), msg("system", 0, "frozen")]);
    let tail = serde_json::to_vec(&msg("user", 1, "断点 🦀 \n \"quoted\"")).unwrap();
    for cut in 0..=tail.len() {
        let input = [prefix.as_slice(), &tail[..cut]].concat();
        fs::write(&path, &input).unwrap();
        let result = WalStore::open(&path).unwrap().load_active_window();
        if cut == 0 {
            assert_eq!(result.unwrap().messages.len(), 1);
        } else if cut < tail.len() {
            let err = result.unwrap_err();
            assert_eq!(err.exit_code().as_i32(), 1, "cut {cut}: {err}");
        } else {
            // A writer commits a complete record with a trailing newline.
            let mut committed = input.clone();
            committed.push(b'\n');
            fs::write(&path, &committed).unwrap();
            assert_eq!(replay(&path).messages.len(), 2);
            continue;
        }
        assert_eq!(fs::read(&path).unwrap(), input, "cut {cut}");
    }
}

// RFC §6.2
#[test]
fn checkpoint_short_circuits_archived_history_and_handles_reverse_read_boundaries() {
    let dir = TempDir::new();
    let path = dir.0.join("wal");
    for size in [65_535, 65_536, 65_537, 131_073] {
        let frozen = "界".repeat(size / 3) + &"x".repeat(size % 3);
        let summary = "<checkpoint>\nsummary\n</checkpoint>";
        let active = lines(&[
            section("checkpoint", 70, 1),
            msg("system", 70, &frozen),
            msg("user", 71, summary),
        ]);
        fs::write(
            &path,
            [b"unreadable archived record\xff\n".as_slice(), &active].concat(),
        )
        .unwrap();
        let window = replay(&path);
        assert_eq!(window.last_checkpoint_turn, 70);
        assert_eq!(window.next_turn_id, 72);
        let rows = serde_json::to_value(window.messages).unwrap();
        assert_eq!(rows.as_array().unwrap().len(), 2);
        assert_eq!(rows[0]["text"], frozen);
        assert_eq!(rows[1]["text"], summary);
    }
}

// RFC §6.1, §6.2
#[test]
fn overlapping_folds_keep_the_surviving_prefix_and_never_recycle_turn_ids() {
    let dir = TempDir::new();
    let path = dir.0.join("wal");
    let fold = |from, to, summary| json!({"type":"fold","ts":TS,"from_turn":from,"to_turn":to,"summary":summary});
    let mut rows = vec![section("session_init", 0, 1), msg("system", 0, "frozen")];
    for i in 1..=5 {
        rows.push(msg("user", i, &format!("u{i}")));
    }
    rows.extend([
        fold(3, 5, "old"),
        msg("user", 6, "more"),
        fold(2, 6, "new"),
        msg("user", 7, "later"),
    ]);
    fs::write(&path, lines(&rows)).unwrap();
    let window = replay(&path);
    assert_eq!(window.next_turn_id, 8);
    let actual: Vec<_> = window
        .messages
        .iter()
        .map(|m| {
            let v = serde_json::to_value(m).unwrap();
            (m.turn_id(), v["text"].as_str().unwrap().to_owned())
        })
        .collect();
    let expected = [
        (0, "frozen"),
        (1, "u1"),
        (2, "<folded turns=\"2..6\">new</folded>"),
        (7, "later"),
    ];
    assert_eq!(actual, expected.map(|(t, s)| (t, s.to_owned())));
}

// RFC §6.2, §6.4
#[test]
fn cli_rejects_corrupted_tails_and_unknown_section_versions_before_inference() {
    for (kind, version, suffix, code) in [
        ("session_init", 0, &b""[..], 2),
        ("checkpoint", 2, &b""[..], 2),
        ("session_init", 1, &b"{\"type\":"[..], 1),
        ("session_init", 1, &b"\xff"[..], 1),
    ] {
        let fx = Fixture::new();
        let root = &fx.dir.0;
        fs::write(root.join("case.agent"), "body").unwrap();
        let mut input = lines(&[section(kind, 0, version), msg("system", 0, "frozen")]);
        input.extend_from_slice(suffix);
        let path = root.join("history");
        fs::write(&path, &input).unwrap();
        let output = fx.run(fx.agent().args(["-c", "history", "case.agent", "--", "go"]));
        expect_code(&output, code);
        assert!(output.stdout.is_empty() && !output.stderr.is_empty() && !fx.capture.exists());
        assert_eq!(fs::read(path).unwrap(), input);
    }
}

// RFC §6.2
#[test]
fn nonblocking_wal_lock_excludes_second_process_and_is_released_on_close() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    fs::write(root.join("case.agent"), "body").unwrap();
    let path = root.join("history");
    let held = WalStore::open(&path).unwrap();
    let args = ["-c", "history", "case.agent", "--", "go"];
    let refused = fx.run(fx.agent().args(args));
    expect_code(&refused, 1);
    assert!(refused.stdout.is_empty() && !refused.stderr.is_empty());
    assert_eq!(fs::metadata(&path).unwrap().len(), 0);
    assert!(!fx.capture.exists());
    drop(held);
    expect_code(&fx.run(fx.agent().args(args)), 0);
    assert_eq!(jsonl(&path)[0]["type"], "session_init");
}

// RFC §6.2, §9.1
#[test]
fn resumed_session_reuses_frozen_prefix_and_appends_configuration_drift_once() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    let script = root.join("case.agent");
    fs::write(&script, "first instruction").unwrap();
    let wal = root.join("history");
    expect_code(
        &fx.run(
            fx.agent()
                .args(["-c", "history", "case.agent", "--", "one"]),
        ),
        0,
    );
    let original = fs::read(&wal).unwrap();
    let first = jsonl(&fx.capture);
    fs::write(script, "revised instruction").unwrap();
    for label in ["two", "three"] {
        expect_code(
            &fx.run(
                fx.agent()
                    .args(["-q", "-c", "history", "case.agent", "--", label]),
            ),
            0,
        );
        assert!(fs::read(&wal).unwrap().starts_with(&original));
        let ir = jsonl(&fx.capture);
        assert_eq!(ir[1], first[1]);
        let updates: Vec<_> = ir
            .iter()
            .skip(2)
            .filter(|r| r["role"] == "system")
            .collect();
        assert_eq!(updates.len(), 1);
        assert!(
            updates[0]["text"]
                .as_str()
                .unwrap()
                .starts_with("<runtime_update>")
        );
        assert_eq!(
            jsonl(&wal)
                .iter()
                .filter(|r| r["type"] == "runtime_update")
                .count(),
            1
        );
    }
}

/// An intentionally forward-only oracle for §6.2. It truncates a Vec when a
/// fold occurs and never reads past events again; the implementation replays
/// the same history in reverse from disk.
#[derive(Clone)]
struct FoldHistory {
    rows: Vec<Value>,
    visible: Vec<(u64, String, String)>,
    last_id: u64,
    input_tokens: u64,
    output_tokens: u64,
}

impl FoldHistory {
    fn append(&mut self, role: &str) {
        self.last_id += 1;
        let id = self.last_id;
        let text = format!("{role}-{id}");
        let mut row = msg(role, id, &text);
        if role == "assistant" {
            // Totals depend on IDs so a dropped or double-counted row is observable.
            row["usage"] = json!({"input_tokens":id, "output_tokens":1});
            self.input_tokens += id;
            self.output_tokens += 1;
        }
        self.rows.push(row);
        self.visible.push((id, role.to_owned(), text));
    }

    fn fold_at(&mut self, index: usize) {
        let from = self.visible[index].0;
        let to = self.last_id;
        let summary = format!("summary-{}-{from}", self.rows.len());
        self.rows
            .push(json!({"type":"fold","ts":TS,"from_turn":from,"to_turn":to,"summary":summary}));
        self.visible.truncate(index);
        self.visible.push((
            from,
            "user".into(),
            format!("<folded turns=\"{from}..{to}\">{summary}</folded>"),
        ));
    }

    fn check(&self, path: &Path) {
        fs::write(path, lines(&self.rows)).unwrap();
        let replayed = replay(path);
        assert_eq!(replayed.next_turn_id, self.last_id + 1);
        assert_eq!(replayed.session_usage.input_tokens, self.input_tokens);
        assert_eq!(replayed.session_usage.output_tokens, self.output_tokens);
        let actual: Vec<_> = replayed
            .messages
            .iter()
            .map(|m| {
                let row = serde_json::to_value(m).unwrap();
                (
                    m.turn_id(),
                    row["role"].as_str().unwrap().to_owned(),
                    row["text"].as_str().unwrap().to_owned(),
                )
            })
            .collect();
        assert_eq!(actual, self.visible, "history: {:?}", self.rows);
        let live_usage: Vec<_> = self
            .visible
            .iter()
            .filter(|(_, role, _)| role == "assistant")
            .map(|(id, _, _)| *id)
            .collect();
        assert_eq!(
            replayed
                .assistant_usages
                .keys()
                .copied()
                .collect::<Vec<_>>(),
            live_usage
        );
    }
}

// RFC §6.1, §6.2, §9.1
#[test]
fn all_live_anchors_across_three_successive_folds_match_forward_replay_and_usage() {
    let dir = TempDir::new();
    let path = dir.0.join("model.wal");
    let initial = FoldHistory {
        rows: vec![section("session_init", 0, 1), msg("system", 0, "frozen")],
        visible: vec![(0, "system".into(), "frozen".into())],
        last_id: 0,
        input_tokens: 0,
        output_tokens: 0,
    };
    let mut states = vec![initial];
    for _ in 0..3 {
        let mut next = Vec::new();
        for mut state in states {
            state.append("user");
            state.append("assistant");
            state.check(&path);
            for i in 1..state.visible.len() {
                let mut branch = state.clone();
                branch.fold_at(i);
                branch.check(&path);
                next.push(branch);
            }
        }
        states = next;
    }
    // 2 first-boundary choices, then 3 or 4, then 3..6; every generated leaf is checked.
    assert_eq!(states.len(), 30);
}
