//! RFC v1.1.1 session routing across the CLI, WAL, Trace and provider processes.
use super::{
    Fixture, executable, expect_code, jsonl, scripted_provider, success_text, tool, with_tools,
};
use serde_json::Value;
use std::{fs, os::unix::fs::symlink};
use uuid::Uuid;

fn launch(fx: &Fixture, context: Option<&str>) -> Value {
    let mut cmd = fx.command_for("/bin/sh");
    cmd.args([
        "-c",
        r#"exec "$@" 3>trace"#,
        "fixture",
        env!("CARGO_BIN_EXE_anix"),
        "-q",
        "-y",
        "--trace-fd=3",
        "--shell",
        "/bin/sh",
    ]);
    if let Some(wal) = context {
        cmd.args(["-c", wal]);
    }
    expect_code(&fx.run(cmd.args(["case.agent", "--", "hi"])), 0);
    let run = jsonl(&fx.dir.0.join("trace"))
        .into_iter()
        .find(|e| e["type"] == "run_start")
        .unwrap();
    assert_eq!(run["provider"], "fixture");
    assert_eq!(run["model"], "fixture-model");
    assert_eq!(run["vision"], true);
    assert_eq!(run["inbox_mode"], false);
    run
}

fn session_id(start: &Value) -> &str {
    start["session_id"]
        .as_str()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| panic!("missing session_id in run_start: {start}"))
}

fn assert_uuid_v4(sid: &str) {
    let id = Uuid::parse_str(sid).unwrap_or_else(|e| panic!("invalid session UUID {sid:?}: {e}"));
    assert_eq!(id.get_version(), Some(uuid::Version::Random));
    assert_eq!(id.get_variant(), uuid::Variant::RFC4122);
}

fn oneshot_fixture(replies: &[Vec<Value>]) -> Fixture {
    let fx = Fixture::new();
    fs::write(fx.dir.0.join("case.agent"), "session fixture").unwrap();
    scripted_provider(&fx, replies);
    let provider = fx.dir.0.join("record-provider");
    fs::rename(&provider, fx.dir.0.join("scripted-provider")).unwrap();
    executable(
        &provider,
        r#"#!/bin/sh
printf '%s\n' "${ANIX_SESSION_ID-}" >> "$HOME/provider-ids"
exec "$HOME/scripted-provider"
"#,
    );
    fx
}

const RECORD_COPROCESS: &str = r#"#!/bin/sh
printf '%s\n' "${ANIX_SESSION_ID-}" >> provider-ids
while IFS= read -r req; do
    printf '%s\n' "$req" >> envelopes
    printf '%s\n' '{"type":"text_delta","text":"ok"}' '{"type":"done","finish_reason":"stop"}'
done
"#;

fn coprocess_fixture(provider_script: &str) -> Fixture {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    fs::write(root.join("case.agent"), "session fixture").unwrap();
    let config = root.join("config/anix/config.toml");
    fs::write(
        &config,
        fs::read_to_string(&config)
            .unwrap()
            .replace("oneshot", "coprocess"),
    )
    .unwrap();
    executable(&root.join("record-provider"), provider_script);
    fx
}

fn provider_ids(fx: &Fixture) -> Vec<String> {
    fs::read_to_string(fx.dir.0.join("provider-ids"))
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

// RFC v1.1.1 §1.4, §3.4, §6.2, §9.2, §9.5
#[test]
fn stateless_runs_get_distinct_uuid_v4s_while_all_provider_calls_in_a_run_share_one() {
    let fx = oneshot_fixture(&[
        with_tools([tool(
            "observe",
            r#"printf '%s' "${ANIX_SESSION_ID-unset}" > shell-id"#,
        )]),
        success_text("first complete"),
        success_text("second complete"),
    ]);
    let first = launch(&fx, None);
    let second = launch(&fx, None);
    let a = session_id(&first);
    let b = session_id(&second);
    assert_uuid_v4(a);
    assert_uuid_v4(b);
    assert_ne!(a, b);
    assert_eq!(provider_ids(&fx), [a, a, b]);
    assert_eq!(fs::read(fx.dir.0.join("shell-id")).unwrap(), b"unset");
}

// RFC v1.1.1 §6.1, §6.2, §9.2, §9.5
#[test]
fn persisted_id_survives_builtin_compaction_checkpoint_alias_and_subsequent_resume() {
    let fx = oneshot_fixture(&[
        with_tools([tool(
            "compact",
            r#"printf '%s\n' '{"op":"compact"}' > "$ANIX_CTL""#,
        )]),
        success_text("compact summary"),
        success_text("first complete"),
        success_text("resumed"),
    ]);
    let root = &fx.dir.0;
    let first = launch(&fx, Some("history"));
    let sid = session_id(&first);
    assert_uuid_v4(sid);
    assert_eq!(first["fresh_session"], true);
    let history = root.join("history");
    let rows = jsonl(&history);
    assert_eq!(rows[0]["type"], "session_init");
    assert_eq!(rows[0]["session_id"], sid);
    let checkpoints: Vec<_> = rows.iter().filter(|r| r["type"] == "checkpoint").collect();
    assert_eq!(checkpoints.len(), 1);
    assert_eq!(checkpoints[0]["session_id"], sid);
    assert!(
        jsonl(&root.join("request-2"))[0]["tools"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let prefix = fs::read(&history).unwrap();
    symlink(&history, root.join("alias")).unwrap();
    let second = launch(&fx, Some("alias"));
    assert_eq!(session_id(&second), sid);
    assert_eq!(second["fresh_session"], false);
    assert_eq!(provider_ids(&fx), [sid; 4]);
    assert!(fs::read(history).unwrap().starts_with(&prefix));
}

// RFC v1.1.1 §1.4, §6.2, §9.2, §9.5
#[test]
fn legacy_wal_uses_a_stable_canonical_path_id_across_aliases_and_restarts() {
    let fx = oneshot_fixture(&[
        success_text("one"),
        success_text("two"),
        success_text("three"),
    ]);
    let old = concat!(
        "{\"type\":\"session_init\",\"ir_version\":1,\"ts\":\"2026-01-01T00:00:00Z\",\"config\":{\"stderr\":\"explicit\",\"approval\":\"auto\"}}\n",
        "{\"type\":\"message\",\"turn_id\":0,\"ts\":\"2026-01-01T00:00:00Z\",\"role\":\"system\",\"text\":\"frozen\"}\n"
    );
    let root = &fx.dir.0;
    for name in ["old", "independent"] {
        fs::write(root.join(name), old).unwrap();
    }
    symlink("old", root.join("alias")).unwrap();
    let first = launch(&fx, Some("./old"));
    let alias = launch(&fx, Some("alias"));
    let independent = launch(&fx, Some("independent"));
    let a = session_id(&first);
    let b = session_id(&independent);
    assert_eq!(session_id(&alias), a);
    assert_ne!(a, b);
    assert_eq!(provider_ids(&fx), [a, a, b]);
    for name in ["old", "independent"] {
        assert!(
            fs::read(root.join(name))
                .unwrap()
                .starts_with(old.as_bytes())
        );
    }
}

// RFC v1.1.1 §1.4, §6.2, §9.2, §9.5
#[test]
fn latest_checkpoint_supplies_exact_stored_id_to_trace_env_and_coprocess_envelope() {
    let fx = coprocess_fixture(RECORD_COPROCESS);
    let root = &fx.dir.0;
    // A non-UUID stored identifier is legal. Reading before this complete checkpoint
    // would encounter the deliberately unreadable archived record.
    let sid = "imported-session-73";
    let prefix = concat!(
        "unreadable archived bytes\n",
        "{\"type\":\"checkpoint\",\"turn_id\":17,\"ir_version\":1,\"ts\":\"2026-01-01T00:00:00Z\",\"session_id\":\"imported-session-73\",\"config\":{\"stderr\":\"explicit\",\"approval\":\"auto\",\"vision\":true}}\n",
        "{\"type\":\"message\",\"turn_id\":17,\"role\":\"system\",\"text\":\"frozen\",\"ts\":\"2026-01-01T00:00:00Z\"}\n",
        "{\"type\":\"message\",\"turn_id\":18,\"role\":\"user\",\"text\":\"<checkpoint>\\nsummary\\n</checkpoint>\",\"ts\":\"2026-01-01T00:00:00Z\"}\n"
    );
    fs::write(root.join("history"), prefix).unwrap();
    let start = launch(&fx, Some("history"));
    assert_eq!(session_id(&start), sid);
    assert_eq!(provider_ids(&fx), [sid]);
    let envelopes = jsonl(&root.join("envelopes"));
    assert_eq!(envelopes.len(), 1);
    assert_eq!(envelopes[0]["session_id"], sid);
    assert_eq!(envelopes[0]["messages"][0]["text"], "frozen");
    assert!(
        fs::read(root.join("history"))
            .unwrap()
            .starts_with(prefix.as_bytes())
    );
}

// RFC v1.1.1 §1.4, §3.4, §6.2, §9.2, §9.5
#[test]
fn nested_anix_gets_its_own_id_and_the_parent_resumes_with_its_original_id() {
    let command =
        r#""$HOME/anix" -q --trace-fd=8 --provider inner child.agent -- nested 8>inner-trace"#;
    let fx = oneshot_fixture(&[
        with_tools([tool("child", command)]),
        success_text("parent complete"),
    ]);
    let root = &fx.dir.0;
    symlink(env!("CARGO_BIN_EXE_anix"), root.join("anix")).unwrap();
    fs::write(root.join("child.agent"), "child prompt").unwrap();
    let config = root.join("config/anix/config.toml");
    fs::write(
        &config,
        fs::read_to_string(&config).unwrap()
            + "\n[providers.inner]\nbin='inner-provider'\nmode='oneshot'\ntimeout_sec=8\n",
    )
    .unwrap();
    executable(
        &root.join("inner-provider"),
        r#"#!/bin/sh
/bin/cat > child-request
printf '%s' "${ANIX_SESSION_ID-}" > child-id
printf '%s\n' '{"type":"text_delta","text":"child complete"}' '{"type":"done","finish_reason":"stop"}'
"#,
    );
    let parent = launch(&fx, Some("outer.wal"));
    let child = jsonl(&root.join("inner-trace"))
        .into_iter()
        .find(|e| e["type"] == "run_start")
        .unwrap();
    let parent_sid = session_id(&parent);
    let child_sid = session_id(&child);
    assert_uuid_v4(child_sid);
    assert_ne!(parent_sid, child_sid);
    assert_eq!(child["provider"], "inner");
    assert_eq!(
        fs::read_to_string(root.join("child-id")).unwrap(),
        child_sid
    );
    assert_eq!(provider_ids(&fx), [parent_sid; 2]);
    assert_eq!(
        jsonl(&root.join("child-request")).last().unwrap()["text"],
        "<args>nested</args>"
    );
    assert_eq!(
        jsonl(&root.join("outer.wal")).last().unwrap()["text"],
        "parent complete"
    );
}

// RFC v1.1.1 §4.2, §6.2, §9.2, §9.5
#[test]
fn inbox_keeps_one_session_id_for_both_messages_in_a_single_coprocess() {
    let fx = coprocess_fixture(RECORD_COPROCESS);
    let root = &fx.dir.0;
    fs::create_dir_all(root.join("mail/new")).unwrap();
    fs::create_dir_all(root.join("mail/tmp")).unwrap();
    for name in ["100-20-2", "100-20-1"] {
        super::inbox::publish(root, name, name);
    }
    expect_code(&fx.run(fx.command_for("/bin/sh").args([
        "-c",r#"exec "$1" -q --trace-fd=3 --inbox mail --idle-timeout 1 -c history case.agent 3>trace"#,
        "fixture",env!("CARGO_BIN_EXE_anix")])), 0);
    let trace = jsonl(&root.join("trace"));
    let start = trace.iter().find(|r| r["type"] == "run_start").unwrap();
    assert_eq!(start["inbox_mode"], true);
    let sid = session_id(start);
    assert_eq!(provider_ids(&fx), [sid]);
    assert_eq!(jsonl(&root.join("history"))[0]["session_id"], sid);
    let requests = jsonl(&root.join("envelopes"));
    assert_eq!(requests.len(), 2);
    for (req, name) in requests.iter().zip(["100-20-1", "100-20-2"]) {
        assert_eq!(req["session_id"], sid);
        assert!(
            req["messages"].as_array().unwrap().last().unwrap()["text"]
                .as_str()
                .unwrap()
                .contains(name)
        );
        assert!(root.join("mail/done").join(name).is_file());
    }
    // The protocol labels requests independently of the shared session routing identifier.
    for req in &requests {
        assert!(req["request_id"].is_string());
    }
}

// RFC v1.1.1 §1.4, §6.1, §6.2, §9.2, §9.5
#[test]
fn first_checkpoint_in_a_legacy_wal_persists_the_fallback_id_even_if_the_wal_moves() {
    let fx = oneshot_fixture(&[
        with_tools([tool(
            "compact",
            r#"printf '%s\n' '{"op":"compact"}' > "$ANIX_CTL""#,
        )]),
        success_text("upgraded summary"),
        success_text("after checkpoint"),
        success_text("after move"),
    ]);
    let root = &fx.dir.0;
    let old = concat!(
        "{\"type\":\"session_init\",\"ir_version\":1,\"config\":{\"approval\":\"auto\",\"stderr\":\"explicit\"},\"ts\":\"2026-01-01T00:00:00Z\"}\n",
        "{\"type\":\"message\",\"role\":\"system\",\"text\":\"frozen\",\"turn_id\":0,\"ts\":\"2026-01-01T00:00:01Z\"}\n"
    );
    fs::write(root.join("legacy"), old).unwrap();
    let before = launch(&fx, Some("legacy"));
    let sid = session_id(&before);
    let bytes = fs::read(root.join("legacy")).unwrap();
    assert!(bytes.starts_with(old.as_bytes()));
    let sections = jsonl(&root.join("legacy"));
    let checkpoint = sections.iter().find(|r| r["type"] == "checkpoint").unwrap();
    assert_eq!(checkpoint["session_id"], sid);
    fs::rename(root.join("legacy"), root.join("relocated")).unwrap();
    let after = launch(&fx, Some("relocated"));
    assert_eq!(session_id(&after), sid);
    assert_eq!(provider_ids(&fx), [sid; 4]);
    assert!(
        fs::read(root.join("relocated"))
            .unwrap()
            .starts_with(&bytes)
    );
}

// RFC v1.1.1 §3.1, §3.4, §6.1, §6.2, §9.2, §9.5
#[test]
fn empty_context_overrides_a_saved_wal_and_keeps_one_ephemeral_id_through_compaction() {
    let fx = oneshot_fixture(&[
        success_text("saved"),
        with_tools([tool(
            "compact",
            r#"printf '%s\n' '{"op":"compact"}' > "$ANIX_CTL""#,
        )]),
        success_text("summary"),
        success_text("ephemeral"),
        success_text("resumed saved"),
    ]);
    let root = &fx.dir.0;
    fs::write(
        root.join("case.agent"),
        "---\ncontext: saved.wal\n---\nbody",
    )
    .unwrap();
    let saved = launch(&fx, None);
    let saved_id = session_id(&saved);
    let original = fs::read(root.join("saved.wal")).unwrap();
    let ephemeral = launch(&fx, Some(""));
    let ephemeral_id = session_id(&ephemeral);
    assert_uuid_v4(ephemeral_id);
    assert_ne!(saved_id, ephemeral_id);
    assert_eq!(fs::read(root.join("saved.wal")).unwrap(), original);
    let compactor_request = jsonl(&root.join("request-3"));
    assert_eq!(compactor_request[0]["tools"], serde_json::json!([]));
    let resumed = launch(&fx, None);
    assert_eq!(session_id(&resumed), saved_id);
    assert_eq!(
        provider_ids(&fx),
        [saved_id, ephemeral_id, ephemeral_id, ephemeral_id, saved_id]
    );
}

// RFC v1.1.1 §6.2, §9.2, §9.5
#[test]
fn restarting_a_coprocess_between_turns_preserves_session_and_tool_history() {
    let fx = coprocess_fixture(
        r#"#!/bin/sh
set -eu
printf '%s\n' "${ANIX_SESSION_ID-}" >> provider-ids
IFS= read -r req
printf '%s\n' "$req" >> envelopes
if [ ! -f first-turn ]; then
    : > first-turn
    exec 0<&-
    printf '%s\n' '{"type":"tool_call","id":"inspect","name":"msh","arguments":{"cmd":"printf observed"}}' '{"type":"done","finish_reason":"tool_calls"}'
else
    printf '%s\n' '{"type":"text_delta","text":"recovered"}' '{"type":"done","finish_reason":"stop"}'
    /bin/cat > /dev/null
fi
"#,
    );
    let start = launch(&fx, Some("history"));
    let sid = session_id(&start);
    let root = &fx.dir.0;
    assert_eq!(provider_ids(&fx), [sid; 2]);
    let frames = jsonl(&root.join("envelopes"));
    assert_eq!(frames.len(), 2);
    assert!(frames.iter().all(|f| f["session_id"] == sid));
    let tool_result = frames[1]["messages"].as_array().unwrap().last().unwrap();
    assert_eq!(tool_result["role"], "tool");
    assert_eq!(tool_result["tool_call_id"], "inspect");
    assert!(tool_result["text"].as_str().unwrap().contains("observed"));
    let history = jsonl(&root.join("history"));
    assert_eq!(history[0]["session_id"], sid);
    assert_eq!(history.last().unwrap()["text"], "recovered");
}
