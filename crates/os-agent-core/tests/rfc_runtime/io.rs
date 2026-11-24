//! §§3.1, 3.4, 4.1–4.4 and 9.5. Observe the inherited descriptors from both sides.
use super::{Fixture, expect_code, jsonl, scripted_provider, success_text, tool, with_tools};
use serde_json::{Value, json};
use std::fs;

#[test]
fn cli_version_usage_and_success_have_distinct_stream_contracts() {
    let fx = Fixture::new();
    let version = fx.run(fx.agent().arg("--version"));
    expect_code(&version, 0);
    let line = String::from_utf8(version.stdout).unwrap();
    assert!(
        line.contains(env!("CARGO_PKG_VERSION")) && line.contains("ir_version=1"),
        "{line}"
    );
    for args in [
        &[][..],
        &["--no-such-option"],
        &["--approval", "maybe", "case.agent"],
        &["--max-turns", "-1", "case.agent"],
    ] {
        let output = fx.run(fx.agent().args(args));
        expect_code(&output, 2);
        assert!(
            output.stdout.is_empty() && !output.stderr.is_empty(),
            "{args:?}"
        );
    }
    assert!(!fx.capture.exists());
}

#[test]
fn args_stdin_and_sparse_extra_descriptors_are_ordered_and_trace_is_separate() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    fs::write(root.join("case.agent"), "body").unwrap();
    for (file, bytes) in [
        ("input", "primary"),
        ("four", "four"),
        ("six", "six"),
        ("nine", "nine"),
    ] {
        fs::write(root.join(file), bytes).unwrap();
    }
    let result=fx.run(fx.command_for("/bin/sh").args(["-c",
        r#"exec "$1" case.agent -- "two words" end <input 9<nine 4<four 6<six 7>write-only 3>trace"#,
        "fixture",env!("CARGO_BIN_EXE_agent")]));
    expect_code(&result, 0);
    assert!(result.stdout.is_empty());
    let ir = jsonl(&fx.capture);
    let user = ir.iter().find(|m| m["role"] == "user").unwrap()["text"]
        .as_str()
        .unwrap();
    let (_, mut tail) = user.split_once("<args>two words end</args>").unwrap();
    tail = tail.split_once("primary").unwrap().1;
    for (fd, contents) in [(4, "four"), (6, "six"), (9, "nine")] {
        let tag = format!("<input fd=\"{fd}\">");
        let (before, section) = tail.split_once(&tag).unwrap();
        assert!(before.trim().is_empty(), "{user}");
        let (body, after) = section.split_once("</input>").unwrap();
        // The RFC specifies wrapping and ordering, without specifying padding newlines.
        assert!(
            body == contents || body == format!("\n{contents}\n"),
            "{body:?}"
        );
        tail = after;
    }
    assert!(tail.trim().is_empty());
    assert!(
        !user.contains("fd=\"7\"") && !user.contains("fd=\"3\""),
        "{user}"
    );
    let trace = jsonl(&root.join("trace"));
    assert_eq!(trace.first().unwrap()["type"], "run_start");
    assert_eq!(trace.last().unwrap()["type"], "run_end");
    assert!(trace.iter().all(|r| r["v"] == 1
        && r["pid"].is_number()
        && r["ppid"].is_number()
        && r["agent"] == "case"
        && r["ts"].is_string()));
}

#[test]
fn repeated_payload_redirection_appends_raw_bytes_in_both_stderr_modes() {
    for mode in ["auto", "explicit"] {
        let fx = Fixture::new();
        let root = &fx.dir.0;
        fs::write(root.join("case.agent"), "body").unwrap();
        scripted_provider(
            &fx,
            &[
                with_tools([
                    tool(
                        "one",
                        r#"printf 'A\000B' > "$AGENT_OUT"; printf diagnostic-one > "$AGENT_ERR"; printf captured-one"#,
                    ),
                    tool(
                        "two",
                        r#"printf 'C\377D' > "$AGENT_OUT"; printf diagnostic-two > "$AGENT_ERR"; printf captured-two"#,
                    ),
                ]),
                success_text("assistant-only"),
            ],
        );
        let result = fx.run(fx.command_for("/bin/sh").args([
            "-c",
            r#"exec "$1" --shell /bin/sh -y --stderr "$2" case.agent -- go >payload 3>trace"#,
            "fixture",
            env!("CARGO_BIN_EXE_agent"),
            mode,
        ]));
        expect_code(&result, 0);
        assert_eq!(fs::read(root.join("payload")).unwrap(), b"A\0BC\xffD");
        let stderr = String::from_utf8(result.stderr).unwrap();
        for expected in ["diagnostic-one", "diagnostic-two"] {
            assert!(stderr.contains(expected), "{mode} {stderr}");
        }
        assert_eq!(
            stderr.contains("assistant-only"),
            mode == "auto",
            "{stderr}"
        );
        let trace = jsonl(&root.join("trace"));
        assert!(
            trace
                .iter()
                .any(|e| e["type"] == "text_delta" && e["text"] == "assistant-only")
        );
        let ir = jsonl(&root.join("request-2"));
        let results: Vec<_> = ir.iter().filter(|m| m["role"] == "tool").collect();
        assert_eq!(results.len(), 2);
        for (m, id, out) in [
            (results[0], "one", "captured-one"),
            (results[1], "two", "captured-two"),
        ] {
            let tid = m["turn_id"].as_u64().unwrap();
            assert_eq!(m["tool_call_id"], id);
            assert_eq!(m["text"], format!("[t{tid}] exit=0\n{out}"));
            assert_ne!(m["is_error"], json!(true));
        }
    }
}

#[test]
fn nested_tool_receives_control_channels_tools_path_and_no_parent_wal_variable() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    fs::write(root.join("case.agent"), "body").unwrap();
    let tools = root.join(".local/libexec/agent/tools");
    fs::create_dir_all(&tools).unwrap();
    fs::write(tools.join("aclr"), "").unwrap();
    let cmd = r#"printf '%s\n' "${AGENT_CONTEXT-unset}" "$AGENT_OUT" "$AGENT_ERR" "$AGENT_CTL" "${PATH%%:*}" "$AGENT_MODEL"; if read -r x; then exit 23; fi; exit 0"#;
    scripted_provider(&fx, &[with_tools([tool("env", cmd)]), success_text("done")]);
    let result = fx.run(
        fx.agent()
            .env("AGENT_CONTEXT", "session.wal")
            .env("AGENT_MODEL", "child-visible")
            .args(["--shell", "/bin/sh", "-y", "case.agent", "--", "go"]),
    );
    expect_code(&result, 0);
    assert!(root.join("session.wal").is_file());
    let ir = jsonl(&root.join("request-2"));
    let m = ir.iter().find(|m| m["role"] == "tool").unwrap();
    assert_ne!(m["is_error"], Value::Bool(true));
    let text = m["text"].as_str().unwrap();
    let actual: Vec<_> = text.lines().skip(1).collect();
    assert_eq!(
        actual,
        [
            "unset",
            "/dev/fd/4",
            "/dev/fd/5",
            "/dev/fd/6",
            tools.to_str().unwrap(),
            "child-visible"
        ]
    );
}

#[test]
fn script_arguments_accept_an_optional_separator_and_preserve_option_like_literals() {
    let fx = Fixture::new();
    fs::write(fx.dir.0.join("case.agent"), "body").unwrap();
    for (args, user) in [
        (&["case.agent", "hello", "two words"][..], "hello two words"),
        (
            &["case.agent", "--", "hello", "two words"][..],
            "hello two words",
        ),
        (&["case.agent", "--", "-q", "--version"][..], "-q --version"),
    ] {
        expect_code(&fx.run(fx.agent().args(args)), 0);
        assert_eq!(
            jsonl(&fx.capture).last().unwrap()["text"],
            format!("<args>{user}</args>")
        );
    }
}
