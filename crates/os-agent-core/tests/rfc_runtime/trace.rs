//! RFC §§4.1 and 9.5. Observe where the agent actually sends each class of data.
use super::{
    Fixture, TempDir, done, expect_code, jsonl, scripted_provider, success_text, tool, with_tools,
};
use serde_json::{Value, json};
use std::fs;
use std::io::Read;
use std::os::unix::{fs::PermissionsExt, net::UnixListener};
use std::path::Path;

const PROMPT: &str = "private-fixture-system-text";
const KEY: &str = "trace-test-placeholder-credential";

// RFC §5.1, §6.1, §9.5
#[test]
fn events_distinguish_tool_approval_from_execution_and_record_an_applied_fold() {
    for policy in ["auto", "deny"] {
        let fx = Fixture::new();
        let root = &fx.dir.0;
        fs::write(root.join("case.agent"), "trace fixture").unwrap();
        scripted_provider(
            &fx,
            &[
                with_tools([
                    tool("first", "printf a >> ran"),
                    tool(
                        "second",
                        r#"printf b >> ran; printf '%s\n' '{"op":"fold","from_turn":1,"summary":"recap"}' > "$AGENT_CTL""#,
                    ),
                ]),
                success_text("finished"),
            ],
        );
        let out = fx.run(fx.command_for("/bin/sh").env("AGENT_TRACE_LEVEL", "events").args([
            "-c", r#"exec "$1" -q --approval "$2" --shell /bin/sh -c history case.agent -- go 3>trace"#,
            "fixture", env!("CARGO_BIN_EXE_agent"), policy,
        ]));
        expect_code(&out, 0);
        let trace = jsonl(&root.join("trace"));
        let kinds: Vec<_> = trace.iter().map(|r| r["type"].as_str().unwrap()).collect();
        let tool_events: Vec<_> = kinds
            .iter()
            .copied()
            .filter(|k| matches!(*k, "tool_approval" | "tool_start" | "tool_end"))
            .collect();
        let folded = jsonl(&root.join("history"))
            .iter()
            .any(|r| r["type"] == "fold");
        assert_eq!(
            kinds.iter().filter(|k| **k == "fold").count(),
            usize::from(folded)
        );
        if policy == "auto" {
            assert_eq!(
                tool_events,
                [
                    "tool_approval",
                    "tool_start",
                    "tool_end",
                    "tool_approval",
                    "tool_start",
                    "tool_end"
                ]
            );
            assert_eq!(fs::read(root.join("ran")).unwrap(), b"ab");
            assert!(folded);
        } else {
            assert_eq!(tool_events, ["tool_approval", "tool_approval"]);
            assert!(!root.join("ran").exists() && !folded);
        }
    }
}

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

// RFC §9.5, §4.1
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

// RFC §9.5
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

// RFC §4.1, §9.5
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

// RFC §9.5
#[test]
fn disconnected_trace_recipient_does_not_change_payload_stderr_or_exit_status() {
    use std::os::fd::OwnedFd;
    use std::os::unix::net::UnixStream;
    use std::process::Stdio;
    for connected_at_launch in [true, false] {
        let fx = fixture();
        let (writer, reader) = UnixStream::pair().unwrap();
        // Use stdin to transfer an O_RDWR endpoint into the wrapper; the actual agent
        // receives that endpoint only on fd 3 and has /dev/null on fd 0.
        let mut cmd = fx.command_for("/bin/sh");
        cmd.stdin(Stdio::from(OwnedFd::from(writer))).args([
            "-c",
            r#"exec 3<&0; exec "$1" -q case.agent -- go </dev/null"#,
            "fixture",
            env!("CARGO_BIN_EXE_agent"),
        ]);
        if connected_at_launch {
            let reader = std::thread::spawn(move || {
                let mut reader = reader;
                reader
                    .set_read_timeout(Some(std::time::Duration::from_secs(12)))
                    .unwrap();
                let mut byte = [0; 1];
                reader.read_exact(&mut byte).unwrap();
            });
            let out = fx.run(&mut cmd);
            reader.join().unwrap();
            expect_code(&out, 0);
            assert!(out.stdout.is_empty() && out.stderr.is_empty());
        } else {
            drop(reader);
            let out = fx.run(&mut cmd);
            expect_code(&out, 0);
            assert!(out.stdout.is_empty() && out.stderr.is_empty());
        }
        assert!(fx.dir.0.join("request-1").exists());
    }
}
