//! §§3.1, 3.4, 4.1–4.4 and 9.5. Observe the inherited descriptors from both sides.
use super::{Fixture, expect_code, jsonl, scripted_provider, success_text, tool, with_tools};
use serde_json::{Value, json};
use std::fs;

// RFC v1.1 §3.1, §4.1, §6.5
#[test]
fn cli_version_usage_and_success_have_distinct_stream_contracts() {
    let fx = Fixture::new();
    let version = fx.run(fx.anix().arg("--version"));
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
        let output = fx.run(fx.anix().args(args));
        expect_code(&output, 2);
        assert!(
            output.stdout.is_empty() && !output.stderr.is_empty(),
            "{args:?}"
        );
    }
    assert!(!fx.capture.exists());
}

// RFC v1.1 §4.1, §9.5
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
        r#"exec "$1" --trace-fd=3 case.agent -- "two words" end <input 9<nine 4<four 6<six 7>write-only 3>trace"#,
        "fixture",env!("CARGO_BIN_EXE_anix")]));
    expect_code(&result, 0);
    assert_eq!(result.stdout, b"ok");
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

// RFC v1.1 §4.1, §4.3
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
                        r#"printf 'A\000B' > "$ANIX_OUT"; printf diagnostic-one > "$ANIX_ERR"; printf captured-one"#,
                    ),
                    tool(
                        "two",
                        r#"printf 'C\377D' > "$ANIX_OUT"; printf diagnostic-two > "$ANIX_ERR"; printf captured-two"#,
                    ),
                ]),
                success_text("assistant-only"),
            ],
        );
        let result = fx.run(fx.command_for("/bin/sh").args([
            "-c",
            r#"exec "$1" --trace-fd=3 --output explicit --shell /bin/sh -y --stderr "$2" case.agent -- go >payload 3>trace"#,
            "fixture",
            env!("CARGO_BIN_EXE_anix"),
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

// RFC v1.1 §2, §3.4, §4.3
#[test]
fn tool_receives_control_channels_tools_path_and_no_parent_model_or_wal() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    fs::write(root.join("case.agent"), "body").unwrap();
    let tools = root.join(".local/libexec/anix/tools");
    fs::create_dir_all(&tools).unwrap();
    fs::write(tools.join("aclr"), "").unwrap();
    let cmd = r#"printf '%s\n' "${ANIX_CONTEXT-unset}" "$ANIX_OUT" "$ANIX_ERR" "$ANIX_CTL" "${PATH%%:*}" "${ANIX_MODEL-unset}"; if read -r x; then exit 23; fi; exit 0"#;
    scripted_provider(&fx, &[with_tools([tool("env", cmd)]), success_text("done")]);
    let result = fx.run(
        fx.anix()
            .env("ANIX_CONTEXT", "session.wal")
            .env("ANIX_MODEL", "parent-only")
            .args([
                "--output",
                "explicit",
                "--shell",
                "/bin/sh",
                "-y",
                "case.agent",
                "--",
                "go",
            ]),
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
            "unset"
        ]
    );
}

// RFC v1.1 §3.1, §4.1
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
        expect_code(&fx.run(fx.anix().args(args)), 0);
        assert_eq!(
            jsonl(&fx.capture).last().unwrap()["text"],
            format!("<args>{user}</args>")
        );
    }
}

// RFC v1.1 §4.1, §4.3
#[test]
fn every_static_fd_access_mode_is_probed_and_consumed_input_files_are_closed_before_spawn() {
    use super::executable;
    // Check the original file identity, because a child can legitimately reuse a
    // low-numbered descriptor for a different file or pipe after exec.
    let inspect = r#"for n in 4 5 6 7 8 9; do
    if [ -f "$HOME/readable-$n" ] && [ "/dev/fd/$n" -ef "$HOME/source-$n" ]; then
        printf '%s\n' "$n" >> "$HOME/leaked"
    fi
done"#;
    for rotation in 0..4 {
        let fx = Fixture::new();
        let root = &fx.dir.0;
        fs::write(root.join("case.agent"), "body").unwrap();
        let hook = root.join("config/anix/hooks/pre-context.d/70-inspect");
        executable(&hook, &format!("#!/bin/sh\n{inspect}\n/bin/cat\n"));
        scripted_provider(
            &fx,
            &[with_tools([tool("inspect", inspect)]), success_text("done")],
        );
        let provider = root.join("record-provider");
        let body = fs::read_to_string(&provider).unwrap().replacen(
            "set -eu",
            &format!("set -eu\n{inspect}"),
            1,
        );
        fs::write(provider, body).unwrap();
        let mut redirects = String::new();
        let mut expected = Vec::new();
        for n in 4..=9 {
            let mode = (n + rotation) % 4;
            let file = format!("source-{n}");
            fs::write(root.join(&file), format!("value-{n}")).unwrap();
            redirects.push_str(&match mode {
                0 => format!(" {n}<&-"),
                1 => format!(" {n}<{file}"),
                2 => format!(" {n}>{file}"),
                _ => format!(" {n}<>{file}"),
            });
            if mode == 1 || mode == 3 {
                expected.push(n);
                fs::write(root.join(format!("readable-{n}")), "").unwrap();
            }
        }
        let script = format!("exec \"$1\" -y -q --shell /bin/sh case.agent -- initial{redirects}");
        expect_code(
            &fx.run(fx.command_for("/bin/sh").args([
                "-c",
                &script,
                "fixture",
                env!("CARGO_BIN_EXE_anix"),
            ])),
            0,
        );
        let req = jsonl(&root.join("request-1"));
        let user = req.last().unwrap()["text"].as_str().unwrap();
        assert_eq!(user.matches("<input fd=").count(), expected.len(), "{user}");
        let mut tail = user;
        for n in expected {
            let tag = format!("<input fd=\"{n}\">");
            let section = tail.split_once(&tag).unwrap().1;
            let (value, rest) = section.split_once("</input>").unwrap();
            assert_eq!(value.trim_matches('\n'), format!("value-{n}"));
            tail = rest;
        }
        assert!(
            !root.join("leaked").exists(),
            "consumed input inherited by child"
        );
    }
}

// RFC v1.1 §3.4, §4.3
#[test]
fn shell_environment_contains_only_observability_effective_approval_and_routed_fds() {
    use super::executable;
    use std::collections::BTreeMap;

    // A harmless allowlisted fixture lets us inspect the real environment under
    // ask and deny as well as auto, without prompting or changing policy.
    for (cli, yes, inherited_approval, log, trace, expected) in [
        (&[][..], None, None, None, None, "ask"),
        (
            &["-y"][..],
            None,
            Some("deny"),
            Some("info"),
            Some("events"),
            "auto",
        ),
        (&[][..], Some("true"), None, Some(""), Some(""), "auto"),
        (
            &["--approval", "deny"][..],
            Some("1"),
            Some("auto"),
            Some("trace"),
            Some("deltas"),
            "deny",
        ),
    ] {
        let fx = Fixture::new();
        let root = &fx.dir.0;
        let config = root.join("config/anix/config.toml");
        fs::write(
            &config,
            fs::read_to_string(&config).unwrap() + "\n[approval]\nsafe_commands=['observe']\n",
        )
        .unwrap();
        fs::write(root.join("case.agent"), "body").unwrap();
        executable(
            &root.join("observe"),
            "#!/bin/sh\n/usr/bin/env > observed\n",
        );
        scripted_provider(
            &fx,
            &[with_tools([tool("t", "observe")]), success_text("finished")],
        );
        let mut cmd = fx.anix();
        cmd.args(cli).envs([
            ("ANIX_CONTEXT", "parent.wal"),
            ("ANIX_MODEL", "parent-model"),
            ("ANIX_VISION", "false"),
            ("ANIX_PROVIDER", "fixture"),
            ("ANIX_MAX_TURNS", "6"),
            ("ANIX_MAX_TOKENS", "8192"),
            ("ANIX_MAX_OUTPUT_TOKENS", "140"),
            ("ANIX_COMPACT_THRESHOLD", "0"),
            ("ANIX_IDLE_TIMEOUT", "7"),
            ("ANIX_SHELL", "/bin/sh"),
            ("ANIX_STDERR", "explicit"),
            ("ANIX_OPAQUE_TEST_KEY", "private-fixture"),
            ("ANIX_PROVIDER_API_KEY", "fixture-only"),
            ("ANIX_OUT", "wrong-4"),
            ("ANIX_ERR", "wrong-5"),
            ("ANIX_CTL", "wrong-6"),
            ("OPENAI_API_KEY", "external-placeholder"),
            ("MY_ANIX_MODEL", "ordinary"),
            ("ANIXED_FIXTURE", "ordinary-prefix"),
            ("ANIX_", "prefix-only"),
        ]);
        for (name, value) in [
            ("ANIX_YES", yes),
            ("ANIX_APPROVAL", inherited_approval),
            ("ANIX_LOG", log),
            ("ANIX_TRACE_LEVEL", trace),
        ] {
            if let Some(v) = value {
                cmd.env(name, v);
            }
        }
        if expected == "deny" {
            cmd.env("ANIX_INBOX", "parent-mail")
                .args(["--idle-timeout", "1"]);
        }
        expect_code(&fx.run(cmd.args(["case.agent", "--", "go"])), 0);
        let raw = fs::read_to_string(root.join("observed")).unwrap();
        let vars: BTreeMap<_, _> = raw.lines().map(|l| l.split_once('=').unwrap()).collect();
        for (key, value) in [
            ("OPENAI_API_KEY", "external-placeholder"),
            ("MY_ANIX_MODEL", "ordinary"),
            ("ANIXED_FIXTURE", "ordinary-prefix"),
        ] {
            assert_eq!(vars.get(key), Some(&value));
        }
        let actual: BTreeMap<_, _> = vars
            .into_iter()
            .filter(|(k, _)| k.starts_with("ANIX_"))
            .collect();
        let mut allowed = BTreeMap::from([
            ("ANIX_ERR", "/dev/fd/5"),
            ("ANIX_CTL", "/dev/fd/6"),
            ("ANIX_APPROVAL", expected),
        ]);
        if let Some(v) = log {
            allowed.insert("ANIX_LOG", v);
        }
        if let Some(v) = trace {
            allowed.insert("ANIX_TRACE_LEVEL", v);
        }
        assert_eq!(actual, allowed, "cli={cli:?} yes={yes:?}");
        assert!(root.join("parent.wal").is_file());
    }
}

// RFC v1.1.1 §4.1, §4.3
#[test]
fn unterminated_deltas_are_separated_from_tool_status_diagnostics_and_usage() {
    for kind in ["reasoning_delta", "text_delta"] {
        for mode in ["auto", "explicit"] {
            let fx = Fixture::new();
            let root = &fx.dir.0;
            fs::write(root.join("case.agent"), "fixture").unwrap();
            // Keep the diagnostic out of the command text so a status line
            // echoing the command cannot satisfy the fd 5 assertion.
            fs::write(root.join("note"), "from-diagnostic-channel\n").unwrap();
            let mut first = vec![json!({"type":kind,"text":"before-tool"})];
            first.extend(with_tools([tool(
                "call",
                r#"/bin/cat note > "$ANIX_ERR"; printf 'P\000Q' > "$ANIX_OUT""#,
            )]));
            scripted_provider(&fx, &[first, success_text("final-fragment")]);
            let output = fx.run(fx.anix().args([
                "--output",
                "explicit",
                "--stderr",
                mode,
                "--shell",
                "/bin/sh",
                "-y",
                "case.agent",
                "--",
                "go",
            ]));
            expect_code(&output, 0);
            assert_eq!(output.stdout, b"P\0Q");
            let stderr = String::from_utf8(output.stderr).unwrap();
            if mode == "explicit" {
                assert_eq!(stderr, "from-diagnostic-channel\n");
                continue;
            }
            let (_, next) = stderr
                .split_once("before-tool")
                .unwrap_or_else(|| panic!("missing {kind}: {stderr:?}"));
            let (status, after) = next
                .split_once("from-diagnostic-channel\n")
                .unwrap_or_else(|| panic!("missing diagnostic: {stderr:?}"));
            assert!(
                status.starts_with('\n') && status.ends_with('\n'),
                "{stderr:?}"
            );
            assert!(!status.trim().is_empty(), "missing tool status: {stderr:?}");
            assert!(after.contains("final-fragment\n[usage] "), "{stderr:?}");
            assert!(after.ends_with('\n'));
        }
    }
}
