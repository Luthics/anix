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
    let prefix = lines(&[section("session_init", 0, 1), msg("system", 0, "frozen")]);
    let tail = serde_json::to_vec(&msg("user", 1, "断点 🦀 \n \"quoted\"")).unwrap();
    for cut in 0..tail.len() {
        let path = dir.0.join(format!("cut-{cut}.wal"));
        let input = [prefix.as_slice(), &tail[..cut]].concat();
        fs::write(&path, &input).unwrap();
        let result = WalStore::open(&path).unwrap().load_active_window();
        if cut == 0 {
            assert_eq!(result.unwrap().messages.len(), 1);
        } else {
            let err = result.unwrap_err();
            assert_eq!(err.exit_code().as_i32(), 1, "cut {cut}: {err}");
        }
        assert_eq!(fs::read(&path).unwrap(), input, "cut {cut}");
    }
    let committed = dir.0.join("committed.wal");
    fs::write(&committed, [prefix, tail, vec![b'\n']].concat()).unwrap();
    assert_eq!(replay(&committed).messages.len(), 2);
}

// RFC §6.2, §6.4
#[test]
fn resuming_a_complete_last_object_without_lf_never_joins_two_wal_records() {
    let fx = Fixture::new();
    fs::write(fx.dir.0.join("case.agent"), "fixture").unwrap();
    let history = fx.dir.0.join("history");
    let run = |message: &str| {
        fx.run(
            fx.agent()
                .args(["-q", "-c", "history", "case.agent", "--", message]),
        )
    };
    expect_code(&run("seed"), 0);
    let mut prefix = fs::read(&history).unwrap();
    assert_eq!(prefix.pop(), Some(b'\n'));
    fs::write(&history, &prefix).unwrap();
    fs::remove_file(&fx.capture).unwrap();

    let result = run("after missing newline");
    let bytes = fs::read(&history).unwrap();
    // The missing-LF policy is open. Both rejection and acceptance must
    // preserve the existing prefix. A successful append must remain JSONL.
    assert!(bytes.starts_with(&prefix));
    if !result.status.success() {
        expect_code(&result, 1);
        assert!(!result.stderr.is_empty());
        assert_eq!(bytes, prefix);
        assert!(!fx.capture.exists());
        return;
    }
    assert!(bytes.ends_with(b"\n"));
    let mut users = Vec::new();
    for (i, line) in bytes[..bytes.len() - 1].split(|&b| b == b'\n').enumerate() {
        let row: Value = serde_json::from_slice(line).unwrap_or_else(|e| {
            panic!(
                "successful resume left invalid JSONL on line {}: {e}",
                i + 1
            )
        });
        if row["role"] == "user" {
            users.push(row["text"].as_str().unwrap().to_owned());
        }
    }
    assert_eq!(
        users,
        ["<args>seed</args>", "<args>after missing newline</args>"]
    );
    expect_code(&run("resume again"), 0);
}

// RFC §6.2
#[test]
fn checkpoint_short_circuits_archived_history_and_handles_reverse_read_boundaries() {
    let dir = TempDir::new();
    for size in [65_535, 65_536, 65_537, 131_073] {
        let path = dir.0.join(format!("boundary-{size}.wal"));
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

    fn check(&self) {
        // Each generated history represents an independent session.
        let dir = TempDir::new();
        let path = dir.0.join("model.wal");
        fs::write(&path, lines(&self.rows)).unwrap();
        let replayed = replay(&path);
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
            state.check();
            for i in 1..state.visible.len() {
                let mut branch = state.clone();
                branch.fold_at(i);
                branch.check();
                next.push(branch);
            }
        }
        states = next;
    }
    // 2 first-boundary choices, then 3 or 4, then 3..6; every generated leaf is checked.
    assert_eq!(states.len(), 30);
}

// RFC §6.2, §9.1
#[test]
fn resume_compares_both_contract_fields_after_a_sparse_runtime_update() {
    let mut discrepancies = Vec::new();
    for kind in ["session_init", "checkpoint"] {
        for (stored_patch, stderr, approval, changed) in [
            (json!({"stderr":"explicit"}), "explicit", "ask", false),
            (json!({"stderr":"explicit"}), "explicit", "deny", true),
            (json!({"approval":"deny"}), "auto", "deny", false),
            (json!({"approval":"deny"}), "explicit", "deny", true),
        ] {
            let fx = Fixture::new();
            let root = &fx.dir.0;
            fs::write(root.join("case.agent"), "current script").unwrap();
            let start = if kind == "session_init" { 0 } else { 20 };
            let mut rows = vec![
                section(kind, start, 1),
                msg("system", start, "frozen prefix"),
            ];
            if kind == "checkpoint" {
                rows.push(msg(
                    "user",
                    start + 1,
                    "<checkpoint>\nsummary\n</checkpoint>",
                ));
            } else {
                rows.push(msg("user", 1, "first user message"));
            }
            rows.push(json!({"type":"runtime_update","turn_id":start+2,"ts":TS,
                             "content":"stored update","config":stored_patch}));
            let original = lines(&rows);
            let history = root.join("history");
            fs::write(&history, &original).unwrap();
            for input in ["first", "second"] {
                expect_code(
                    &fx.run(fx.agent().args([
                        "--stderr",
                        stderr,
                        "--approval",
                        approval,
                        "-c",
                        "history",
                        "case.agent",
                        "--",
                        input,
                    ])),
                    0,
                );
                assert!(fs::read(&history).unwrap().starts_with(&original));
                let wal = jsonl(&history);
                let updates: Vec<_> = wal
                    .iter()
                    .filter(|r| r["type"] == "runtime_update")
                    .collect();
                let ir = jsonl(&fx.capture);
                assert_eq!(ir[1]["text"], "frozen prefix");
                let ir_updates: Vec<_> = ir
                    .iter()
                    .skip(2)
                    .filter(|r| r["role"] == "system")
                    .collect();
                assert_eq!(ir_updates.len(), updates.len());
                for (event, message) in updates.iter().zip(ir_updates) {
                    assert_eq!(message["turn_id"], event["turn_id"]);
                    assert_eq!(
                        message["text"],
                        format!(
                            "<runtime_update>{}</runtime_update>",
                            event["content"].as_str().unwrap()
                        )
                    );
                }
                if updates.len() != 1 + usize::from(changed) {
                    discrepancies.push(format!("{kind}, {stored_patch}, resumed as {stderr}/{approval} ({input}): {} updates", updates.len()));
                }
            }
        }
    }
    assert!(
        discrepancies.is_empty(),
        "incorrect drift decisions: {discrepancies:?}"
    );
}

// RFC §6.1, §6.2, §9.1
#[test]
fn running_out_of_uint64_turn_ids_never_reuses_a_committed_anchor() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    fs::write(root.join("case.agent"), "fixture").unwrap();
    let history = root.join("history");
    let start = u64::MAX - 4;
    let prefix = lines(&[
        section("checkpoint", start, 1),
        msg("system", start, "frozen"),
        msg("user", start + 1, "<checkpoint>\nprior\n</checkpoint>"),
        msg("assistant", start + 2, "old answer"),
    ]);
    fs::write(&history, &prefix).unwrap();
    let run = |text: &str| fx.run(fx.agent().args(["-c", "history", "case.agent", "--", text]));
    // Exactly two IDs remain for a normal user/assistant exchange.
    expect_code(&run("last pair"), 0);
    let before = fs::read(&history).unwrap();
    assert!(before.starts_with(&prefix));
    let records = jsonl(&history);
    assert_eq!(records[records.len() - 2]["turn_id"], u64::MAX - 1);
    assert_eq!(records.last().unwrap()["turn_id"], u64::MAX);
    fs::remove_file(&fx.capture).unwrap();
    let exhausted = run("no unused ID");
    assert!(fs::read(&history).unwrap().starts_with(&before));
    let additional: Vec<_> = jsonl(&history)
        .into_iter()
        .skip(records.len())
        .filter(|r| r.get("turn_id").is_some())
        .collect();
    assert!(
        additional.is_empty(),
        "records appended after u64::MAX: {additional:?}"
    );
    assert!(
        !fx.capture.exists(),
        "provider received a request with an exhausted turn counter"
    );
    assert!(!exhausted.status.success());
}

// RFC §6.2, §9.1
#[test]
fn checkpoint_recovery_only_dispatches_a_complete_context_at_each_record_boundary() {
    let old = [
        section("session_init", 0, 1),
        msg("system", 0, "old frozen"),
        msg("user", 1, "history before compression"),
        msg("assistant", 2, "previous answer"),
    ];
    let new = [
        section("checkpoint", 3, 1),
        msg("system", 3, "new frozen"),
        msg("user", 4, "<checkpoint>\nsaved summary\n</checkpoint>"),
    ];
    let mut broken = Vec::new();
    for committed in 0..=new.len() {
        let fx = Fixture::new();
        let root = &fx.dir.0;
        fs::write(root.join("case.agent"), "current script").unwrap();
        let history = root.join("history");
        let before = [lines(&old), lines(&new[..committed])].concat();
        fs::write(&history, &before).unwrap();
        let out = fx.run(
            fx.agent()
                .args(["-c", "history", "case.agent", "--", "resume"]),
        );
        assert!(fs::read(&history).unwrap().starts_with(&before));
        if committed == 0 || committed == new.len() {
            expect_code(&out, 0);
        }
        if !fx.capture.exists() {
            assert!(
                !out.status.success(),
                "resume succeeded without serving the new input"
            );
            continue;
        }
        let rows = jsonl(&fx.capture);
        assert_eq!(rows[0]["type"], "header");
        let context = &rows[1..];
        let last = context.last().unwrap();
        assert_eq!(last["role"], "user");
        assert_eq!(last["text"], "<args>resume</args>");
        let prior_visible = context
            .first()
            .is_some_and(|r| r["role"] == "system" && r["text"] == "old frozen")
            && context
                .iter()
                .any(|r| r["text"] == "history before compression")
            && context.iter().any(|r| r["text"] == "previous answer");
        let checkpoint_visible = context.first().is_some_and(|r| r["role"] == "system")
            && context.get(1).is_some_and(|r| {
                r["role"] == "user"
                    && r["text"].as_str().is_some_and(|s| {
                        s.starts_with("<checkpoint>\n") && s.ends_with("\n</checkpoint>")
                    })
            });
        match committed {
            0 => assert!(prior_visible),
            3 => {
                assert!(checkpoint_visible);
                assert_eq!(context[0]["text"], new[1]["text"]);
                assert_eq!(context[1]["text"], new[2]["text"]);
                assert!(!context.iter().any(|r| r["text"] == "previous answer"));
            }
            _ if !(prior_visible || checkpoint_visible) => {
                broken.push((committed, context.to_vec()))
            }
            _ => {}
        }
    }
    assert!(
        broken.is_empty(),
        "provider saw incomplete checkpoint contexts: {broken:?}"
    );
}

// RFC §6.2, §9.1
#[test]
fn initial_section_interruption_never_dispatches_without_a_frozen_system_message() {
    let initial = [
        section("session_init", 0, 1),
        msg("system", 0, "original frozen prefix"),
    ];
    // Run the complete and empty controls before checking the interrupted case.
    for committed in [2, 0, 1] {
        let fx = Fixture::new();
        let root = &fx.dir.0;
        fs::write(root.join("case.agent"), "script on restart").unwrap();
        let history = root.join("history");
        let prefix = lines(&initial[..committed]);
        fs::write(&history, &prefix).unwrap();
        let out = fx.run(fx.agent().args([
            "-c",
            "history",
            "--stderr",
            "auto",
            "--approval",
            "ask",
            "case.agent",
            "--",
            "resume",
        ]));
        assert!(fs::read(&history).unwrap().starts_with(&prefix));
        if committed == 1 && !fx.capture.exists() {
            assert!(!out.status.success());
            continue;
        }
        expect_code(&out, 0);
        let expected = if committed == 2 {
            "original frozen prefix"
        } else {
            "script on restart"
        };
        let ir = jsonl(&fx.capture);
        assert_eq!(ir[0]["type"], "header");
        assert_eq!(
            ir[1]["role"], "system",
            "committed rows={committed}: {ir:?}"
        );
        assert_eq!(ir[1]["text"], expected);
        assert_eq!(ir[1]["turn_id"], 0);
        assert_eq!(ir.last().unwrap()["text"], "<args>resume</args>");
        let wal = jsonl(&history);
        assert!(
            wal.iter()
                .any(|r| r["role"] == "system" && r["turn_id"] == 0 && r["text"] == expected)
        );
    }
}
