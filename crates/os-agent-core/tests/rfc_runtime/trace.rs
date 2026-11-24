//! RFC §§4.1 and 9.5. Observe where the agent actually sends each class of data.
use super::{Fixture, TempDir, done, expect_code, scripted_provider};
use serde_json::{Value, json};
use std::fs;
use std::io::Read;
use std::os::unix::{fs::PermissionsExt, net::UnixListener};
use std::path::Path;

const PROMPT: &str = "private-fixture-system-text";
const KEY: &str = "trace-test-placeholder-credential";

fn fixture() -> Fixture {
    let fx = Fixture::new();
    fs::write(fx.dir.0.join("case.agent"), PROMPT).unwrap();
    scripted_provider(
        &fx,
        &[vec![
            json!({"type":"reasoning_delta","text":"thinking"}),
            json!({"type":"text_delta","text":"visible"}),
            done(),
        ]],
    );
    // The provider receives the placeholder by the same route as a configured credential.
    let config = fx.dir.0.join("config/agent/config.toml");
    let mut contents = fs::read_to_string(&config).unwrap();
    contents.push_str("\napi_key_env = \"TRACE_FIXTURE_KEY\"\n");
    fs::write(config, contents).unwrap();
    fx
}

fn verify(trace: &str, deltas: bool, wire: bool) {
    assert!(trace.ends_with('\n'));
    assert!(!trace.contains(KEY));
    assert_eq!(trace.contains(PROMPT), wire, "full IR in trace");
    let rows: Vec<Value> = trace
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let types: Vec<_> = rows.iter().map(|r| r["type"].as_str().unwrap()).collect();
    assert_eq!(types.first(), Some(&"run_start"));
    assert_eq!(types.last(), Some(&"run_end"));
    assert!(types.contains(&"llm_request_start") && types.contains(&"llm_response"));
    for (kind, text) in [("reasoning_delta", "thinking"), ("text_delta", "visible")] {
        let frames: Vec<_> = rows.iter().filter(|r| r["type"] == kind).collect();
        if deltas {
            assert_eq!(frames.len(), 1);
            assert_eq!(frames[0]["text"], text);
        } else {
            assert!(frames.is_empty());
        }
    }
}

#[test]
fn regular_fd3_separates_events_deltas_and_wire_and_omits_provider_key() {
    for (level, deltas, wire) in [
        (None, true, false),
        (Some("events"), false, false),
        (Some("deltas"), true, false),
        (Some("wire"), true, true),
    ] {
        let fx = fixture();
        let path = fx.dir.0.join("trace");
        fs::write(&path, "").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let mut cmd = fx.command_for("/bin/sh");
        cmd.args([
            "-c",
            r#"exec "$1" -q case.agent -- input 3>trace"#,
            "fixture",
            env!("CARGO_BIN_EXE_agent"),
        ])
        .env("TRACE_FIXTURE_KEY", KEY);
        if let Some(level) = level {
            cmd.env("AGENT_TRACE_LEVEL", level);
        }
        expect_code(&fx.run(&mut cmd), 0);
        verify(&fs::read_to_string(path).unwrap(), deltas, wire);
    }
}

#[test]
fn wire_trace_only_writes_full_ir_into_a_private_file() {
    for mode in [0o644, 0o660] {
        let fx = fixture();
        let path = fx.dir.0.join("trace");
        fs::write(&path, "").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
        let mut cmd = fx.command_for("/bin/sh");
        cmd.env("AGENT_TRACE_LEVEL", "wire")
            .env("TRACE_FIXTURE_KEY", KEY)
            .args([
                "-c",
                r#"exec "$1" -q case.agent -- input 3>trace"#,
                "fixture",
                env!("CARGO_BIN_EXE_agent"),
            ]);
        let _ = fx.run(&mut cmd);
        let observed = fs::read_to_string(&path).unwrap();
        let permissions = fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert!(!observed.contains(KEY));
        assert!(
            !observed.contains(PROMPT) || permissions == 0o600,
            "wire data present in a file with mode {permissions:04o}"
        );
    }
}

#[test]
fn journal_receives_events_on_fd3_fallback_and_never_receives_wire_ir() {
    for level in [None, Some("wire")] {
        let fx = fixture();
        // sun_path on macOS is 104 bytes, while the default TMPDIR can be longer.
        let runtime = TempDir::under(Path::new("/tmp"));
        let journal = runtime.0.join("agent");
        fs::create_dir(&journal).unwrap();
        let sock = UnixListener::bind(journal.join("journal.sock")).unwrap();
        sock.set_nonblocking(true).unwrap();
        let mut cmd = fx.command_for("/bin/sh");
        cmd.env("XDG_RUNTIME_DIR", &runtime.0)
            .env("TRACE_FIXTURE_KEY", KEY)
            .args([
                "-c",
                r#"exec "$1" -q case.agent -- input 3</dev/null"#,
                "fixture",
                env!("CARGO_BIN_EXE_agent"),
            ]);
        if let Some(level) = level {
            cmd.env("AGENT_TRACE_LEVEL", level);
        }
        expect_code(&fx.run(&mut cmd), 0);
        let (mut reader, _) = sock
            .accept()
            .expect("agent should have connected during its run");
        // The writer has exited. All bytes and EOF must already be available.
        reader.set_nonblocking(true).unwrap();
        let mut events = String::new();
        reader.read_to_string(&mut events).unwrap();
        // The RFC leaves the downgrade policy open; either events or deltas is safe.
        let has_deltas = level.is_some()
            && events
                .lines()
                .map(|l| serde_json::from_str::<Value>(l).unwrap())
                .any(|v| v["type"] == "text_delta");
        verify(&events, has_deltas, false);
    }
}
