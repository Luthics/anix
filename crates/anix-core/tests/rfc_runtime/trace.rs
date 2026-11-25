//! RFC §§4.1 and 9.5. Observe where Anix actually sends each class of data.
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

// RFC v1.1.1 §5.1, §6.1, §9.5
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
                        r#"printf b >> ran; printf '%s\n' '{"op":"fold","from_turn":1,"summary":"recap"}' > "$ANIX_CTL""#,
                    ),
                ]),
                success_text("finished"),
            ],
        );
        let out = fx.run(fx.command_for("/bin/sh").env("ANIX_TRACE_LEVEL", "events").args([
            "-c", r#"exec "$1" -q --trace-fd=3 --approval "$2" --shell /bin/sh -c history case.agent -- go 3>trace"#,
            "fixture", env!("CARGO_BIN_EXE_anix"), policy,
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
    let config = fx.dir.0.join("config/anix/config.toml");
    let mut contents = fs::read_to_string(&config).unwrap();
    contents.push_str("\napi_key_env = \"TRACE_FIXTURE_KEY\"\n");
    fs::write(config, contents).unwrap();
    let provider = fx.dir.0.join("record-provider");
    fs::rename(&provider, fx.dir.0.join("scripted-provider")).unwrap();
    super::executable(
        &provider,
        r#"#!/bin/sh
printf '%s' "$ANIX_PROVIDER_API_KEY" > received-key
exec "$HOME/scripted-provider"
"#,
    );
    fx
}

fn verify(trace: &str, deltas: Option<bool>, wire: bool) {
    assert!(trace.ends_with('\n'));
    assert!(!trace.contains(KEY));
    assert_eq!(trace.contains(PROMPT), wire, "full IR in trace");
    let rows: Vec<Value> = trace
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let start = &rows[0];
    assert_eq!(start["agent"], "case");
    assert!(
        start["session_id"]
            .as_str()
            .is_some_and(|sid| !sid.is_empty())
    );
    for row in &rows {
        assert_eq!(row["v"], 1);
        assert_eq!(row["agent"], start["agent"]);
        assert_eq!(row["pid"], start["pid"]);
        assert_eq!(row["ppid"], std::process::id());
        assert!(row["pid"].as_u64().is_some_and(|pid| pid > 0));
        time::OffsetDateTime::parse(
            row["ts"].as_str().unwrap(),
            &time::format_description::well_known::Rfc3339,
        )
        .expect("RFC 3339 trace timestamp");
    }
    let types: Vec<_> = rows.iter().map(|r| r["type"].as_str().unwrap()).collect();
    assert_eq!(types.first(), Some(&"run_start"));
    assert_eq!(types.last(), Some(&"run_end"));
    assert!(types.contains(&"llm_request_start") && types.contains(&"llm_response"));
    let includes_deltas = deltas.unwrap_or_else(|| types.contains(&"text_delta"));
    for (kind, text) in [("reasoning_delta", "thinking"), ("text_delta", "visible")] {
        let frames: Vec<_> = rows.iter().filter(|r| r["type"] == kind).collect();
        if includes_deltas {
            assert_eq!(frames.len(), 1);
            assert_eq!(frames[0]["text"], text);
        } else {
            assert!(frames.is_empty());
        }
    }
}

// RFC v1.1.1 §9.5, §4.1
#[test]
fn explicit_fd_separates_events_deltas_and_wire_and_omits_provider_key() {
    for (level, deltas, wire) in [
        (None, None, false),
        (Some("events"), Some(false), false),
        (Some("deltas"), Some(true), false),
        (Some("wire"), Some(true), true),
    ] {
        let fx = fixture();
        let path = fx.dir.0.join("trace");
        fs::write(&path, "").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let mut cmd = fx.command_for("/bin/sh");
        cmd.args([
            "-c",
            r#"exec "$1" -q --trace-fd=3 case.agent -- input 3>trace"#,
            "fixture",
            env!("CARGO_BIN_EXE_anix"),
        ])
        .env("TRACE_FIXTURE_KEY", KEY);
        if let Some(level) = level {
            cmd.env("ANIX_TRACE_LEVEL", level);
        }
        expect_code(&fx.run(&mut cmd), 0);
        assert_eq!(
            fs::read_to_string(fx.dir.0.join("received-key")).unwrap(),
            KEY
        );
        let trace = fs::read_to_string(&path).unwrap();
        verify(&trace, deltas, wire);
        if wire {
            let records = jsonl(&path);
            let strings: Vec<_> = records
                .iter()
                .flat_map(|record| record.as_object().unwrap().values())
                .filter_map(Value::as_str)
                .collect();
            let request = fs::read_to_string(fx.dir.0.join("request-1")).unwrap();
            assert!(
                strings.contains(&request.as_str()),
                "wire trace lost the input IR"
            );
            let reply = fs::read_to_string(fx.dir.0.join("reply-1")).unwrap();
            for frame in reply.lines() {
                assert!(
                    strings.contains(&frame),
                    "wire trace lost provider frame: {frame}"
                );
            }
        }
    }
}

// RFC v1.1.1 §4.1, §9.5
#[test]
fn journal_without_an_explicit_fd_receives_lifecycle_events_and_no_wire_records() {
    for level in [None, Some("wire")] {
        let fx = fixture();
        // sun_path on macOS is 104 bytes, while the default TMPDIR can be longer.
        let runtime = TempDir::under(Path::new("/tmp"));
        let journal = runtime.0.join("anix");
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
                env!("CARGO_BIN_EXE_anix"),
            ]);
        if let Some(level) = level {
            cmd.env("ANIX_TRACE_LEVEL", level);
        }
        expect_code(&fx.run(&mut cmd), 0);
        let (mut reader, _) = sock
            .accept()
            .expect("Anix should have connected during its run");
        // The writer has exited. All bytes and EOF must already be available.
        reader.set_nonblocking(true).unwrap();
        let mut events = String::new();
        reader.read_to_string(&mut events).unwrap();
        // §9.5 recommends a default level but always excludes raw wire data on the journal.
        verify(&events, None, false);
    }
}

// RFC v1.1.1 §9.5
#[test]
fn disconnected_trace_recipient_does_not_change_payload_stderr_or_exit_status() {
    use std::os::fd::OwnedFd;
    use std::os::unix::net::UnixStream;
    use std::process::Stdio;
    for connected_at_launch in [true, false] {
        let fx = Fixture::new();
        fs::write(fx.dir.0.join("case.agent"), "trace disconnect fixture").unwrap();
        scripted_provider(
            &fx,
            &[
                with_tools([tool(
                    "streams",
                    r#"printf 'A\000B' > "$ANIX_OUT"; printf 'human note\n' > "$ANIX_ERR""#,
                )]),
                success_text("done"),
            ],
        );
        let (writer, reader) = UnixStream::pair().unwrap();
        // Use stdin to transfer an O_RDWR endpoint into the wrapper; the actual agent
        // receives that endpoint only on fd 3 and has /dev/null on fd 0.
        let mut cmd = fx.command_for("/bin/sh");
        cmd.stdin(Stdio::from(OwnedFd::from(writer))).args([
            "-c",
            r#"exec 3<&0; exec "$1" -q -y --shell /bin/sh --trace-fd=3 case.agent -- go </dev/null"#,
            "fixture",
            env!("CARGO_BIN_EXE_anix"),
        ]);
        let out = if connected_at_launch {
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
            out
        } else {
            drop(reader);
            fx.run(&mut cmd)
        };
        expect_code(&out, 0);
        assert_eq!(out.stdout, b"A\0B");
        assert_eq!(out.stderr, b"human note\n");
        assert!(fx.dir.0.join("request-2").exists());
    }
}

// RFC v1.1.1 §4.1, §9.5
#[test]
fn explicit_trace_fd_takes_precedence_over_an_available_journal() {
    let fx = fixture();
    let runtime = TempDir::under(Path::new("/tmp"));
    let journal = runtime.0.join("anix");
    fs::create_dir(&journal).unwrap();
    let listener = UnixListener::bind(journal.join("journal.sock")).unwrap();
    listener.set_nonblocking(true).unwrap();
    let mut cmd = fx.command_for("/bin/sh");
    cmd.env("XDG_RUNTIME_DIR", &runtime.0)
        .env("ANIX_TRACE_FD", "8")
        .env("ANIX_TRACE_LEVEL", "events")
        .args([
            "-c",
            r#"exec "$1" -q case.agent -- hello 8>trace"#,
            "fixture",
            env!("CARGO_BIN_EXE_anix"),
        ]);
    expect_code(&fx.run(&mut cmd), 0);
    verify(
        &fs::read_to_string(fx.dir.0.join("trace")).unwrap(),
        Some(false),
        false,
    );
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}
