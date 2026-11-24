//! §§4.2, 4.4, 5.1, 6.4, 8.3 and 9.3–9.6.
use super::{Fixture, expect_code, jsonl, scripted_provider, success_text, tool, with_tools};
use serde_json::json;
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};

use super::executable;

// RFC §4.2, §6.2
#[test]
fn inbox_observes_only_published_files_in_ascii_order_and_moves_them_after_commit() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    fs::write(root.join("case.agent"), "queue").unwrap();
    for d in ["tmp", "new"] {
        let p = root.join("mail").join(d);
        fs::create_dir_all(&p).unwrap();
    }
    let entries = [("z-last", "z"), ("A-first", "A"), ("a-middle", "a")];
    fs::write(root.join("mail/tmp/unpublished"), "partial").unwrap();
    for (name, body) in entries {
        let tmp = root.join("mail/tmp").join(name);
        fs::write(&tmp, body).unwrap();
        fs::rename(tmp, root.join("mail/new").join(name)).unwrap();
    }
    scripted_provider(
        &fx,
        &[
            success_text("one"),
            success_text("two"),
            success_text("three"),
        ],
    );
    let out = fx.run(fx.agent().args([
        "-q",
        "--inbox",
        "mail",
        "--idle-timeout",
        "1",
        "-c",
        "history",
        "case.agent",
    ]));
    expect_code(&out, 0);
    assert!(out.stdout.is_empty());
    for dir in ["cur", "done"] {
        assert_eq!(
            fs::metadata(root.join("mail").join(dir))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }
    assert_eq!(
        fs::read_to_string(root.join("mail/tmp/unpublished")).unwrap(),
        "partial"
    );
    let sorted = [entries[1], entries[2], entries[0]];
    let log = jsonl(&root.join("history"));
    let logged: Vec<_> = log.iter().filter(|m| m["role"] == "user").collect();
    assert_eq!(logged.len(), 3);
    for (i, (name, body)) in sorted.iter().enumerate() {
        let ir = jsonl(&root.join(format!("request-{}", i + 1)));
        let users: Vec<_> = ir.iter().filter(|m| m["role"] == "user").collect();
        assert_eq!(users.len(), i + 1);
        let text = users[i]["text"].as_str().unwrap();
        let content = text
            .strip_prefix(&format!("<input source=\"{name}\">"))
            .unwrap()
            .strip_suffix("</input>")
            .unwrap()
            .trim_matches('\n');
        assert_eq!(content, *body);
        assert_eq!(logged[i]["text"], text);
        assert_eq!(
            fs::read_to_string(root.join("mail/done").join(name)).unwrap(),
            *body
        );
        for location in ["new", "cur"] {
            assert!(!root.join("mail").join(location).join(name).exists());
        }
    }
}

// RFC §5.1, §6.4
#[test]
fn deny_and_fail_on_deny_differ_in_exit_status_tool_reply_and_side_effects() {
    for (options, code, runs) in [
        (&["--approval", "deny"][..], 0, false),
        (&["--approval", "deny", "--fail-on-deny"][..], 42, false),
        (&["--approval", "auto"][..], 0, true),
        (&["-y"][..], 0, true),
    ] {
        let fx = Fixture::new();
        let root = &fx.dir.0;
        fs::write(root.join("case.agent"), "body").unwrap();
        scripted_provider(
            &fx,
            &[
                with_tools([tool("t", "printf yes > ran")]),
                success_text("done"),
            ],
        );
        let r =
            fx.run(
                fx.agent()
                    .args(options)
                    .args(["--shell", "/bin/sh", "case.agent", "--", "go"]),
            );
        expect_code(&r, code);
        assert_eq!(root.join("ran").exists(), runs);
        if code == 42 {
            assert!(!root.join("request-2").exists());
            continue;
        }
        let ir = jsonl(&root.join("request-2"));
        let reply = ir.iter().find(|v| v["role"] == "tool").unwrap();
        assert_eq!(reply["tool_call_id"], "t");
        if !runs {
            assert_eq!(reply["is_error"], true);
            assert!(
                reply["text"]
                    .as_str()
                    .unwrap()
                    .contains("Error: Command denied by approval=deny policy")
            );
        }
    }
}

// RFC §3.3, §6.4
#[test]
fn max_turns_stops_before_another_request_and_success_on_last_turn_is_allowed() {
    for first_is_final in [true, false] {
        let fx = Fixture::new();
        let root = &fx.dir.0;
        fs::write(root.join("case.agent"), "body").unwrap();
        let reply = if first_is_final {
            success_text("finished")
        } else {
            with_tools([tool("t", "pwd")])
        };
        scripted_provider(&fx, &[reply]);
        let out = fx.run(fx.agent().args([
            "--max-turns",
            "1",
            "-y",
            "--shell",
            "/bin/sh",
            "case.agent",
            "--",
            "go",
        ]));
        expect_code(&out, if first_is_final { 0 } else { 124 });
        assert_eq!(fs::read_to_string(root.join("count")).unwrap().trim(), "1");
    }
}

// RFC §8.3, §9.3
#[test]
fn pre_context_hooks_merge_in_ascii_order_with_override_and_two_masks() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    let g = root.join("config/agent/hooks/pre-context.d");
    let p = root.join(".agent/hooks/pre-context.d");
    fs::write(root.join("case.agent"), "body").unwrap();
    for (dir, name, label) in [
        (&g, "40-base", "g40"),
        (&g, "41-replace", "wrong"),
        (&p, "41-replace", "p41"),
        (&g, "42-mask", "wrong"),
        (&g, "43-link", "wrong"),
        (&p, "44-local", "p44"),
    ] {
        executable(
            &dir.join(name),
            &format!("#!/bin/sh\nprintf '%s\\n' {label} >> order\n/bin/cat\n"),
        );
    }
    fs::write(p.join("42-mask"), "").unwrap();
    symlink("/dev/null", p.join("43-link")).unwrap();
    for dir in [&g, &p] {
        fs::write(
            dir.join("45-disabled"),
            "#!/bin/sh\nprintf disabled >> order\n",
        )
        .unwrap();
    }
    expect_code(&fx.run(fx.agent().args(["trust", "."])), 0);
    expect_code(&fx.run(fx.agent().args(["case.agent", "--", "go"])), 0);
    assert_eq!(
        fs::read_to_string(root.join("order")).unwrap(),
        "g40\np41\np44\n"
    );
}

// RFC §8.3
#[test]
fn new_and_modified_project_hooks_require_explicit_trust() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    let hook = root.join(".agent/hooks/pre-context.d/40-test");
    fs::write(root.join("case.agent"), "body").unwrap();
    for label in ["first", "revised"] {
        executable(
            &hook,
            &format!("#!/bin/sh\nprintf '{label}' > ran\n/bin/cat\n"),
        );
        let out = fx.run(fx.agent().args(["case.agent", "--", "go"]));
        expect_code(&out, 42);
        assert!(!root.join("ran").exists() && !fx.capture.exists());
        expect_code(&fx.run(fx.agent().args(["trust", "."])), 0);
        let registry = trust_registry(root);
        let real = fs::canonicalize(&hook).unwrap();
        use sha2::{Digest, Sha256};
        let digest = format!("{:x}", Sha256::digest(fs::read(&hook).unwrap()));
        let entry = &registry["hooks"][real.to_str().unwrap()];
        assert_eq!(registry["version"], 1);
        assert_eq!(entry["sha256"], digest);
        assert!(entry["trusted_at"].is_string());
        expect_code(&fx.run(fx.agent().args(["case.agent", "--", "go"])), 0);
        assert_eq!(fs::read_to_string(root.join("ran")).unwrap(), label);
        fs::remove_file(root.join("ran")).unwrap();
        fs::remove_file(&fx.capture).unwrap();
    }
}

// RFC §8.3, §6.1
#[test]
fn project_provider_and_compactor_each_require_trust_after_a_content_change() {
    use sha2::{Digest, Sha256};
    for which in ["adapter", "summarizer"] {
        let fx = Fixture::new();
        let root = &fx.dir.0;
        fs::write(root.join("case.agent"), "body").unwrap();
        let project = root.join(".agent");
        fs::create_dir_all(&project).unwrap();
        let bin = root.join(which);
        let config = if which == "adapter" {
            let body = fs::read_to_string(root.join("record-provider")).unwrap();
            executable(&bin, &body);
            format!("[providers.fixture]\nbin='./{which}'\n")
        } else {
            executable(
                &bin,
                "#!/bin/sh\n/bin/cat > compact-input\nprintf summary\n",
            );
            let compact = r#"printf '%s\n' '{"op":"compact"}' > "$AGENT_CTL""#;
            scripted_provider(
                &fx,
                &[
                    with_tools([tool("compact", compact)]),
                    success_text("finished"),
                ],
            );
            format!("[compact]\nbin='./{which}'\n")
        };
        fs::write(project.join("config.toml"), config).unwrap();
        let run = || {
            fs::write(root.join("count"), "0\n").unwrap();
            fx.run(
                fx.agent()
                    .args(["-q", "-y", "--shell", "/bin/sh", "case.agent", "--", "go"]),
            )
        };
        let real = fs::canonicalize(&bin).unwrap();
        let check_denial = || {
            let result = run();
            expect_code(&result, 42);
            assert!(
                String::from_utf8(result.stderr)
                    .unwrap()
                    .contains(real.to_str().unwrap())
            );
        };
        let capture = if which == "adapter" {
            &fx.capture
        } else {
            &root.join("compact-input")
        };
        check_denial();
        assert!(!capture.exists());
        expect_code(&fx.run(fx.agent().args(["trust", "."])), 0);
        let registry = trust_registry(root);
        let digest = format!("{:x}", Sha256::digest(fs::read(&bin).unwrap()));
        assert_eq!(registry["hooks"][real.to_str().unwrap()]["sha256"], digest);
        expect_code(&run(), 0);
        assert!(capture.is_file());
        fs::remove_file(capture).unwrap();
        fs::write(
            &bin,
            fs::read_to_string(&bin).unwrap() + "\n# fixture revision\n",
        )
        .unwrap();
        check_denial();
        assert!(!capture.exists());
    }
}

fn trust_registry(root: &std::path::Path) -> serde_json::Value {
    serde_json::from_slice(&fs::read(root.join(".local/state/agent/trusted.json")).unwrap())
        .unwrap()
}

// RFC §9.3, §9.5
#[test]
fn post_tool_filters_the_model_result_and_on_exit_failure_keeps_primary_status() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    fs::write(root.join("case.agent"), "body").unwrap();
    executable(
        &root.join("config/agent/hooks/post-tool.d/40-filter"),
        r#"#!/bin/sh
/bin/cat > post.json
printf '%s\n' '{"turn_id":3,"tool_call_id":"t","cmd":"printf before","exit_code":0,"output":"filtered"}'
"#,
    );
    executable(
        &root.join("config/agent/hooks/on-exit.d/40-exit"),
        "#!/bin/sh\n/bin/cat > exit.json\nexit 7\n",
    );
    scripted_provider(
        &fx,
        &[
            with_tools([tool("t", "printf before")]),
            success_text("done"),
        ],
    );
    let out = fx.run(fx.command_for("/bin/sh").args([
        "-c",
        r#"exec "$1" -y --shell /bin/sh case.agent -- go 3>trace"#,
        "fixture",
        env!("CARGO_BIN_EXE_agent"),
    ]));
    expect_code(&out, 0);
    let post = &jsonl(&root.join("post.json"))[0];
    assert_eq!(post["cmd"], "printf before");
    assert_eq!(post["exit_code"], 0);
    assert_eq!(post["tool_call_id"], "t");
    assert_eq!(post["output"], "before");
    let ir = jsonl(&root.join("request-2"));
    let reply = ir.iter().find(|m| m["role"] == "tool").unwrap();
    assert!(post["turn_id"].is_u64());
    assert_eq!(post["turn_id"], reply["turn_id"]);
    let tool = reply["text"].as_str().unwrap();
    assert!(tool.ends_with("filtered") && !tool.contains("before"));
    let exit = &jsonl(&root.join("exit.json"))[0];
    assert_eq!(exit["exit_code"], 0);
    assert_eq!(exit["turns"], 2);
    assert!(
        jsonl(&root.join("trace"))
            .iter()
            .any(|e| e["type"] == "warning")
    );
}

// RFC §4.4, §9.4, §9.6
#[test]
fn frozen_context_lists_skill_metadata_and_tip_paths_but_leaves_bodies_on_disk() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    fs::write(root.join("case.agent"), "body").unwrap();
    let hooks = root.join("config/agent/hooks/pre-context.d");
    for name in ["00-posix-runtime", "20-skills", "30-tips"] {
        fs::remove_file(hooks.join(name)).unwrap();
    }
    for (dir, desc) in [
        ("config/agent/skills/demo", "global-desc"),
        (".agent/skills/demo", "project-desc"),
    ] {
        let p = root.join(dir);
        fs::create_dir_all(&p).unwrap();
        fs::write(
            p.join("SKILL.md"),
            format!("---\nname: demo\ndescription: '{desc}'\n---\nPRIVATE-BODY"),
        )
        .unwrap();
    }
    let tips = root.join(".agent/tips");
    fs::create_dir_all(&tips).unwrap();
    fs::write(tips.join("how-to-goal.md"), "PROJECT-BODY").unwrap();
    fs::write(tips.join("how-to-loop.md"), "").unwrap();
    symlink("/dev/null", tips.join("how-to-sub-agent.md")).unwrap();
    fs::write(tips.join(".hidden.md"), "hidden").unwrap();
    expect_code(
        &fx.run(fx.agent().args(["-q", "case.agent", "--", "go"])),
        0,
    );
    let ir = jsonl(&fx.capture);
    let sys = ir[1]["text"].as_str().unwrap();
    assert!(
        sys.contains("<posix_runtime>") && sys.contains("$AGENT_OUT") && sys.contains("$AGENT_ERR")
    );
    assert!(sys.contains("<available_skills>") && sys.contains("project-desc"));
    assert!(
        sys.contains("<runtime_tips>")
            && sys.contains(
                fs::canonicalize(tips.join("how-to-goal.md"))
                    .unwrap()
                    .to_str()
                    .unwrap()
            )
    );
    for excluded in [
        "global-desc",
        "PRIVATE-BODY",
        "PROJECT-BODY",
        "how-to-loop.md",
        "how-to-sub-agent.md",
        ".hidden.md",
    ] {
        assert!(!sys.contains(excluded), "{excluded}: {sys}");
    }
    assert!(
        root.join(".local/state/agent/builtin-tips/how-to-goal.md")
            .is_file()
    );
}

// RFC §9.2, §9.5
#[test]
fn provider_failure_after_partial_text_retries_with_unchanged_ir_and_emits_trace_evidence() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    fs::write(root.join("case.agent"), "body").unwrap();
    scripted_provider(
        &fx,
        &[
            vec![
                json!({"type":"text_delta","text":"partial"}),
                json!({"type":"reasoning_delta","text":"thought"}),
                json!({"type":"error","message":"temporary","code":"busy","retryable":true}),
            ],
            success_text("final"),
        ],
    );
    let provider = root.join("record-provider");
    let script = fs::read_to_string(&provider).unwrap();
    fs::write(
        provider,
        script + "\nif [ \"$n\" = 1 ]; then echo diagnostic >&2; exit 1; fi\n",
    )
    .unwrap();
    let result = fx.run(
        fx.command_for("/bin/sh")
            .env("AGENT_TRACE_LEVEL", "events")
            .args([
                "-c",
                r#"exec "$1" -q -c history case.agent -- go 3>trace"#,
                "fixture",
                env!("CARGO_BIN_EXE_agent"),
            ]),
    );
    expect_code(&result, 0);
    assert_eq!(
        fs::read(root.join("request-1")).unwrap(),
        fs::read(root.join("request-2")).unwrap()
    );
    let trace = jsonl(&root.join("trace"));
    let errors: Vec<_> = trace
        .iter()
        .filter(|e| e["type"] == "provider_error")
        .collect();
    assert_eq!(errors.len(), 1);
    let e = errors[0];
    for (key, expected) in [
        ("code", "busy"),
        ("partial_text", "partial"),
        ("partial_reasoning", "thought"),
    ] {
        assert_eq!(e[key], expected);
    }
    assert_eq!(e["attempt"], 1);
    assert!(e["elapsed_ms"].is_number() && e["last_delta_ts"].is_string());
    assert!(
        e["provider_stderr"]
            .as_str()
            .unwrap()
            .contains("diagnostic")
    );
    assert!(
        trace
            .iter()
            .all(|e| e["type"] != "text_delta" && e["type"] != "reasoning_delta")
    );
    let wal = jsonl(&root.join("history"));
    let replies: Vec<_> = wal.iter().filter(|e| e["role"] == "assistant").collect();
    assert_eq!(replies.len(), 1);
    assert_eq!(replies[0]["text"], "final");
}
