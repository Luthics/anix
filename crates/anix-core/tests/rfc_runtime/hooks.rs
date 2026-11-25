//! §§4.4, 8.3 and 9.3–9.6. Tests use separate HOME and project roots.
use super::{
    Fixture, executable, expect_code, jsonl, scripted_provider, success_text, tool, with_tools,
};
use std::fs;
use std::time::{Duration, Instant};

// RFC §9.1-09 — pre-context 输出中的改写及追加记录可进入被冻结的上下文。
// RFC §9.3-01 — 串联 pre-context 过滤器后只冻结最终改写的消息序列。
#[test]
fn pre_context_filters_can_rewrite_and_append_conversation_records_before_freezing() {
    use serde_json::json;
    let fx = Fixture::new();
    let root = &fx.dir.0;
    fs::write(root.join("case.agent"), "original prompt").unwrap();
    let hooks = root.join("config/anix/hooks/pre-context.d");
    let system = json!({"role":"system","turn_id":0,"text":" revised\n\t中  "});
    let user =
        json!({"role":"user","turn_id":1,"text":"from hook","x_annotation":{"list":[true,3]}});
    let assistant = json!({"role":"assistant","turn_id":2,"text":"earlier reply"});
    let mut revised = user.clone();
    revised["text"] = json!("revised by second hook");
    fs::write(
        root.join("hook-rows"),
        format!("{system}\n{user}\n{assistant}\n"),
    )
    .unwrap();
    fs::write(root.join("edited-user"), format!("{revised}\n")).unwrap();
    executable(
        &hooks.join("40-seed"),
        r#"#!/bin/sh
set -eu
/bin/cat > initial.ir
IFS= read -r header < initial.ir
printf '%s\n' "$header"
/bin/cat hook-rows
"#,
    );
    executable(
        &hooks.join("41-edit"),
        r#"#!/bin/sh
set -eu
/bin/cat > between.ir
{
    IFS= read -r header
    IFS= read -r frozen
    IFS= read -r original_user
    printf '%s\n' "$header" "$frozen"
    /bin/cat edited-user
    /bin/cat
} < between.ir
"#,
    );
    scripted_provider(&fx, &[success_text("first"), success_text("second")]);
    let args = ["-q", "-c", "history", "case.agent", "--", "go"];
    expect_code(&fx.run(fx.anix().args(args)), 0);
    let seed = jsonl(&root.join("initial.ir"));
    assert_eq!(seed[1]["text"], "original prompt");
    let between = jsonl(&root.join("between.ir"));
    assert_eq!(
        between,
        [seed[0].clone(), system.clone(), user, assistant.clone()]
    );
    let first = jsonl(&root.join("request-1"));
    assert_eq!(first[1..4], [system, revised, assistant]);
    assert_eq!(first[4]["text"], "<args>go</args>");
    assert!(first[4]["turn_id"].as_u64().unwrap() > 2);
    // A resumed session must not call the filters again; its recorded
    // conversation still contains the edited text and the original prefix.
    executable(&hooks.join("40-seed"), "#!/bin/sh\nexit 19\n");
    expect_code(&fx.run(fx.anix().args(args)), 0);
    let resumed = jsonl(&root.join("request-2"));
    for (original, restored) in first[1..5].iter().zip(&resumed[1..5]) {
        for field in ["role", "text", "turn_id"] {
            assert_eq!(original[field], restored[field], "{field}");
        }
    }
}

// RFC §9.3-01 — pre-context 失败时 stderr 可见，后续推理不启动。
#[test]
fn pre_context_failure_forwards_stderr_and_stops_before_inference() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    fs::write(root.join("case.agent"), "body").unwrap();
    let dir = root.join("config/anix/hooks/pre-context.d");
    executable(
        &dir.join("51-fail"),
        "#!/bin/sh\n/bin/cat > pre-input\nprintf specific-pre-failure >&2\nexit 7\n",
    );
    executable(
        &dir.join("52-next"),
        "#!/bin/sh\n: > should-not-run\n/bin/cat\n",
    );
    let out = fx.run(fx.anix().args(["case.agent", "--", "start"]));
    expect_code(&out, 1);
    assert!(
        String::from_utf8(out.stderr)
            .unwrap()
            .contains("specific-pre-failure")
    );
    assert_eq!(jsonl(&root.join("pre-input"))[0]["type"], "header");
    assert!(!root.join("should-not-run").exists() && !fx.capture.exists());
}

// RFC §9.3-02 — 首个失败的 post-tool 阻止后续过滤器、工具和推理。
#[test]
fn post_tool_failure_stops_before_later_hooks_tools_and_provider_turns() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    fs::write(root.join("case.agent"), "body").unwrap();
    let dir = root.join("config/anix/hooks/post-tool.d");
    executable(
        &dir.join("51-fail"),
        "#!/bin/sh\n/bin/cat > post-input\nexit 6\n",
    );
    executable(
        &dir.join("52-next"),
        "#!/bin/sh\n: > should-not-run\n/bin/cat\n",
    );
    scripted_provider(
        &fx,
        &[
            with_tools([
                tool("first", "printf one"),
                tool("second", "printf two > second-tool"),
            ]),
            success_text("unexpected"),
        ],
    );
    let out = fx.run(
        fx.command_for("/bin/sh")
            .env("ANIX_TRACE_LEVEL", "events")
            .args([
                "-c",
                r#"exec "$1" --trace-fd=3 --shell /bin/sh -y case.agent -- start 3>trace"#,
                "fixture",
                env!("CARGO_BIN_EXE_anix"),
            ]),
    );
    assert!(!out.status.success());
    let events = jsonl(&root.join("trace"));
    let end = events.last().unwrap();
    assert_eq!(end["type"], "run_end");
    assert_eq!(end["exit_code"], out.status.code().unwrap());
    let post: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("post-input")).unwrap()).unwrap();
    assert_eq!(post["tool_call_id"], "first");
    for path in ["second-tool", "should-not-run", "request-2"] {
        assert!(!root.join(path).exists(), "{path}");
    }
}

// RFC §6.3-02 — 一个 on-exit 钩子超时后其他钩子仍执行，保留主进程退出码。
// RFC §9.3-03 — 一个 on-exit 钩子超时只影响该钩子，主退出码保持不变。
// RFC §10.2-02 — 在无 pidfd 的 Linux 配置里，退出钩子仍有独立超时。
#[test]
fn on_exit_timeout_is_local_to_one_hook_and_preserves_the_primary_exit_code() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    fs::write(root.join("case.agent"), "body").unwrap();
    let dir = root.join("config/anix/hooks/on-exit.d");
    executable(
        &dir.join("10-slow"),
        "#!/bin/sh\n/bin/cat > slow-input\nexec /bin/sleep 20\n",
    );
    executable(&dir.join("20-next"), "#!/bin/sh\n/bin/cat > next-input\n");
    fs::write(
        root.join("record-provider"),
        "#!/bin/sh\n/bin/cat >/dev/null\nprintf bad-provider >&2\nexit 4\n",
    )
    .unwrap();
    let start = Instant::now();
    let out = fx.run(fx.command_for("/bin/sh").args([
        "-c",
        r#"exec "$1" --trace-fd=3 -q case.agent -- go 3>trace"#,
        "fixture",
        env!("CARGO_BIN_EXE_anix"),
    ]));
    expect_code(&out, 1);
    assert!(start.elapsed() >= Duration::from_secs(5));
    let payload = |name: &str| {
        serde_json::from_slice::<serde_json::Value>(&fs::read(root.join(name)).unwrap()).unwrap()
    };
    let slow = payload("slow-input");
    assert_eq!(slow["exit_code"], 1);
    assert_eq!(slow, payload("next-input"));
    assert!(
        jsonl(&root.join("trace"))
            .iter()
            .any(|r| r.to_string().contains("10-slow"))
    );
    assert!(!String::from_utf8(out.stderr).unwrap().contains("10-slow"));
}

// RFC §9.4-03 — 六层搜索顺序和同名覆盖。
// RFC §9.4-04 — 逐层比较同名技能的获胜路径，首轮索引中不出现正文。
#[test]
fn each_skill_search_layer_wins_once_and_the_body_stays_out_of_the_index() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    let project = root.join("project");
    fs::create_dir(&project).unwrap();
    fs::write(project.join("case.agent"), "body").unwrap();
    fs::remove_file(root.join("config/anix/hooks/pre-context.d/20-skills")).unwrap();
    let layers = [
        project.join(".anix/skills"),
        project.join(".skills"),
        project.join(".claude/skills"),
        root.join("config/anix/skills"),
        root.join(".skills"),
        root.join(".claude/skills"),
    ];
    for (i, dir) in layers.iter().enumerate() {
        let path = dir.join("tool");
        fs::create_dir_all(&path).unwrap();
        fs::write(
            path.join("SKILL.md"),
            format!("---\nname: sample\ndescription: 'level-{i} & details'\n---\nBODY-ONLY"),
        )
        .unwrap();
    }
    for (i, dir) in layers.iter().enumerate() {
        expect_code(
            &fx.run(
                fx.anix()
                    .current_dir(&project)
                    .args(["case.agent", "--", "go"]),
            ),
            0,
        );
        let ir = jsonl(&fx.capture);
        let text = ir[1]["text"].as_str().unwrap();
        assert_eq!(text.matches("<skill ").count(), 1, "{text}");
        let path = fs::canonicalize(dir.join("tool/SKILL.md")).unwrap();
        assert!(
            text.contains(path.to_str().unwrap())
                && text.contains(&format!("level-{i} &amp; details")),
            "{text}"
        );
        assert!(!text.contains("BODY-ONLY"));
        fs::remove_file(path).unwrap();
    }
    expect_code(
        &fx.run(
            fx.anix()
                .current_dir(&project)
                .args(["case.agent", "--", "go"]),
        ),
        0,
    );
    assert!(
        !jsonl(&fx.capture)[1]["text"]
            .as_str()
            .unwrap()
            .contains("<skill ")
    );
}

// RFC §9.6-02 — 版本漂移后恢复内置指南；状态目录只读时使用运行时目录。
#[test]
fn tips_are_restored_after_version_drift_and_fall_back_to_runtime_when_state_is_unwritable() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    fs::write(root.join("case.agent"), "body").unwrap();
    let mask = root.join("config/anix/hooks/pre-context.d/30-tips");
    let state = root.join("state");
    let builtin = state.join("anix/builtin-tips");
    let invoke = || {
        expect_code(
            &fx.run(
                fx.anix()
                    .env("XDG_STATE_HOME", &state)
                    .args(["case.agent", "--", "go"]),
            ),
            0,
        );
        jsonl(&fx.capture)[1]["text"].as_str().unwrap().to_owned()
    };
    assert!(!invoke().contains("<runtime_tips>") && !builtin.exists());
    fs::remove_file(mask).unwrap();
    let text = invoke();
    let file = builtin.join("how-to-loop.md");
    let original = fs::read(&file).unwrap();
    assert!(!original.is_empty());
    assert!(text.contains(fs::canonicalize(&file).unwrap().to_str().unwrap()));
    fs::write(&file, "stale text").unwrap();
    invoke();
    assert_eq!(fs::read(file).unwrap(), original);
    // A regular path component reliably prevents writes, including under uid 0.
    fs::rename(&builtin, state.join("previous")).unwrap();
    fs::write(&builtin, "blocked path").unwrap();
    let fallback = invoke();
    let file = root.join("run/anix/builtin-tips/how-to-loop.md");
    assert_eq!(fs::read(&file).unwrap(), original);
    assert!(fallback.contains(fs::canonicalize(file).unwrap().to_str().unwrap()));
}

// RFC §9.6-02 — 屏蔽 30-tips 时首次运行不会释放内置指南文件。
// RFC §9.6-03 — 空文件或 /dev/null 屏蔽 30-tips 后索引和磁盘副作用均消失。
#[test]
fn masking_the_whole_tips_hook_suppresses_both_its_index_and_first_run_files() {
    use std::os::unix::fs::symlink;
    for project in [false, true] {
        for dev_null in [false, true] {
            let fx = Fixture::new();
            let root = &fx.dir.0;
            fs::write(root.join("case.agent"), "body").unwrap();
            let global = root.join("config/anix/hooks/pre-context.d/30-tips");
            fs::remove_file(&global).unwrap();
            let mask = if project {
                root.join(".anix/hooks/pre-context.d/30-tips")
            } else {
                global
            };
            fs::create_dir_all(mask.parent().unwrap()).unwrap();
            if dev_null {
                symlink("/dev/null", &mask).unwrap();
            } else {
                fs::write(&mask, "").unwrap();
            }
            if project {
                expect_code(&fx.run(fx.anix().args(["trust", "."])), 0);
            }
            for disabled in [true, false] {
                if !disabled {
                    fs::remove_file(&mask).unwrap();
                }
                expect_code(&fx.run(fx.anix().args(["-q", "case.agent", "--", "go"])), 0);
                let rows = jsonl(&fx.capture);
                let text = rows[1]["text"].as_str().unwrap();
                if disabled {
                    assert_eq!(text, "body");
                } else {
                    assert!(text.contains("<runtime_tips>"));
                }
                assert_eq!(
                    root.join(".local/state/anix/builtin-tips/how-to-goal.md")
                        .exists(),
                    !disabled
                );
                assert!(!root.join("run/anix/builtin-tips/how-to-goal.md").exists());
            }
        }
    }
}

// RFC §9.6-04 — 屏蔽所有指南后不向系统提示词注入 tips 索引。
#[test]
fn an_empty_merged_tips_index_costs_no_prompt_bytes() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    fs::write(root.join("case.agent"), "body").unwrap();
    fs::remove_file(root.join("config/anix/hooks/pre-context.d/30-tips")).unwrap();
    let run = || {
        expect_code(&fx.run(fx.anix().args(["-q", "case.agent", "--", "go"])), 0);
        jsonl(&fx.capture)[1]["text"].as_str().unwrap().to_owned()
    };
    assert!(run().contains("<runtime_tips>"));
    let tips = root.join(".anix/tips");
    fs::create_dir_all(tips.join("nested")).unwrap();
    for entry in fs::read_dir(root.join(".local/state/anix/builtin-tips")).unwrap() {
        let p = entry.unwrap().path();
        if p.extension().is_some_and(|ext| ext == "md") {
            fs::write(tips.join(p.file_name().unwrap()), "").unwrap();
        }
    }
    for ignored in [".hidden.md", "nested/guide.md", "guide.txt"] {
        fs::write(tips.join(ignored), "ignored").unwrap();
    }
    assert_eq!(run(), "body");
    fs::write(tips.join("visible.md"), "contents stay on disk").unwrap();
    let displayed = run();
    let path = fs::canonicalize(tips.join("visible.md")).unwrap();
    assert_eq!(displayed.matches(path.to_str().unwrap()).count(), 1);
    assert!(!displayed.contains("contents stay on disk"));
}

// RFC §9.6-01 — MCP 指南按 anix 状态目录、全局和项目路径披露。
#[test]
fn mcp_guide_follows_anix_xdg_and_project_precedence_without_disclosing_tool_schemas() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    fs::write(root.join("case.agent"), "body").unwrap();
    fs::remove_file(root.join("config/anix/hooks/pre-context.d/30-tips")).unwrap();
    let name = "how-to-mcp.md";
    let builtin = root.join("state/anix/builtin-tips").join(name);
    let global = root.join("config/anix/tips").join(name);
    let project = root.join(".anix/tips").join(name);
    fs::create_dir_all(project.parent().unwrap()).unwrap();
    fs::create_dir_all(global.parent().unwrap()).unwrap();
    fs::write(
        root.join(".mcp.json"),
        r#"{"mcpServers":{"fixture-server":{"command":"fixture-mcp"}}}"#,
    )
    .unwrap();
    executable(&root.join("fixture-mcp"), "#!/bin/sh\n: > server-started\n");
    let names = [
        "how-to-background.md",
        "how-to-goal.md",
        "how-to-loop.md",
        name,
        "how-to-sub-agent.md",
    ];
    for step in 0..4 {
        match step {
            1 => fs::write(&global, "global private body").unwrap(),
            2 => fs::write(&project, "project private body").unwrap(),
            3 => fs::write(&project, "").unwrap(),
            _ => {}
        }
        expect_code(
            &fx.run(fx.anix().env("XDG_STATE_HOME", root.join("state")).args([
                "-q",
                "case.agent",
                "--",
                "go",
            ])),
            0,
        );
        let ir = jsonl(&fx.capture);
        let tools: Vec<_> = ir[0]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert_eq!(tools, ["msh"]);
        let text = ir[1]["text"].as_str().unwrap();
        let block = text
            .split_once("<runtime_tips>")
            .unwrap()
            .1
            .split_once("</runtime_tips>")
            .unwrap()
            .0;
        let paths: Vec<_> = block
            .lines()
            .filter(|l| l.starts_with('/'))
            .map(std::path::Path::new)
            .collect();
        let indexed: Vec<_> = paths
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap())
            .collect();
        assert!(indexed.windows(2).all(|pair| pair[0] < pair[1]));
        for expected in names {
            assert_eq!(
                indexed.contains(&expected),
                step != 3 || expected != name,
                "{indexed:?}"
            );
        }
        if step < 3 {
            let path = paths
                .iter()
                .find(|p| p.file_name().unwrap() == name)
                .unwrap();
            assert_eq!(
                **path,
                fs::canonicalize([&builtin, &global, &project][step]).unwrap()
            );
        }
        assert!(fs::metadata(&builtin).unwrap().len() > 0);
        assert!(!text.contains("private body") && !text.contains("fixture-server"));
        assert!(!root.join("server-started").exists());
    }
}

// RFC §3.1-10 — 启动与恢复时对照 stderr=auto/explicit 的提示词、输出与 WAL 通知。
// RFC §4.4-02 — 首次生成 stderr=auto 的运行提示，诊断流可见模型文本。
// RFC §4.4-03 — 在旧 WAL 上切换至 explicit，只在末尾追加通知并保留冻结首条提示。
#[test]
fn runtime_contract_depends_on_stderr_and_only_changes_at_the_tail_on_resume() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    fs::remove_file(root.join("config/anix/hooks/pre-context.d/00-posix-runtime")).unwrap();
    fs::write(root.join("case.agent"), "keep this prompt").unwrap();
    let request = |mode: &str, context: &str| {
        let out = fx.run(fx.anix().args([
            "--output",
            "explicit",
            "--stderr",
            mode,
            "-c",
            context,
            "case.agent",
            "--",
            "input",
        ]));
        expect_code(&out, 0);
        assert!(out.stdout.is_empty());
        if mode == "explicit" {
            assert!(out.stderr.is_empty());
        } else {
            assert!(String::from_utf8(out.stderr).unwrap().contains("ok"));
        }
        let ir = jsonl(&fx.capture);
        assert_eq!(ir[0]["meta"]["stderr_mode"], mode);
        ir.into_iter()
            .filter(|row| row["role"] == "system")
            .map(|row| row["text"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>()
    };
    let auto = request("auto", "");
    let explicit = request("explicit", "");
    assert_ne!(auto, explicit);
    for prefix in [&auto, &explicit] {
        assert_eq!(prefix.len(), 1);
        for token in ["keep this prompt", "$ANIX_OUT", "$ANIX_ERR", "msh"] {
            assert!(
                prefix[0].contains(token),
                "missing {token} in {}",
                prefix[0]
            );
        }
    }
    for tool in ["stderr", "aview", "aclr"] {
        assert!(auto[0].contains(tool), "missing {tool} in {}", auto[0]);
    }
    for (first, second, frozen) in [("auto", "explicit", auto), ("explicit", "auto", explicit)] {
        let wal = format!("{first}.wal");
        assert_eq!(request(first, &wal), frozen);
        let drift = request(second, &wal);
        assert_eq!(drift.len(), 2);
        assert_eq!(drift[0], frozen[0]);
        assert!(
            drift[1].starts_with("<runtime_update>") && drift[1].ends_with("</runtime_update>")
        );
        assert!(drift[1].contains(second));
        assert_eq!(request(second, &wal), drift);
    }
}

// RFC §4.4-01 — 同名全局或已信任项目钩子可覆写和屏蔽。
// RFC §8.3-01 — 三个内置 pre-context 钩子依次由全局及项目同名文件覆写，也可被空文件或空设备软链屏蔽。
// RFC §9.4-02 — 20-skills 内置、同名覆写与禁用。
// RFC §9.6-03 — 用全局或已信任项目中的同名钩子覆写或屏蔽 30-tips。
#[test]
fn each_builtin_can_be_replaced_globally_then_overridden_or_masked_by_the_project() {
    use std::os::unix::fs::symlink;

    let fx = Fixture::new();
    let root = &fx.dir.0;
    let global = root.join("config/anix/hooks/pre-context.d");
    let project = root.join(".anix/hooks/pre-context.d");
    let skill = root.join(".skills/sample");
    fs::create_dir_all(&skill).unwrap();
    fs::write(
        skill.join("SKILL.md"),
        "---\nname: sample\ndescription: Example\n---\n",
    )
    .unwrap();
    fs::write(root.join("case.agent"), "SCRIPT_MARKER").unwrap();
    let capture = || {
        expect_code(
            &fx.run(fx.anix().args(["-q", "case.agent", "--", "input"])),
            0,
        );
        jsonl(&fx.capture)[1]["text"].as_str().unwrap().to_owned()
    };
    // The other two built-ins stay masked by the fixture in each iteration.
    for (name, tag) in [
        ("00-posix-runtime", "$ANIX_OUT"),
        ("20-skills", "<available_skills>"),
        ("30-tips", "<runtime_tips>"),
    ] {
        let lower = global.join(name);
        let upper = project.join(name);
        fs::remove_file(&lower).unwrap();
        assert!(capture().contains(tag), "{name} absent on first run");
        executable(
            &lower,
            "#!/bin/sh\nexec /usr/bin/sed 's/SCRIPT_MARKER/GLOBAL_MARKER/g'\n",
        );
        assert_eq!(capture(), "GLOBAL_MARKER", "{name}");
        executable(
            &upper,
            "#!/bin/sh\nexec /usr/bin/sed 's/SCRIPT_MARKER/PROJECT_MARKER/g'\n",
        );
        expect_code(&fx.run(fx.anix().args(["trust", "."])), 0);
        assert_eq!(capture(), "PROJECT_MARKER", "{name}");
        for null_link in [false, true] {
            fs::remove_file(&upper).unwrap();
            if null_link {
                symlink("/dev/null", &upper).unwrap();
            } else {
                fs::write(&upper, "").unwrap();
            }
            expect_code(&fx.run(fx.anix().args(["trust", "."])), 0);
            assert_eq!(capture(), "SCRIPT_MARKER", "{name} null_link={null_link}");
        }
        fs::remove_file(&upper).unwrap();
        assert_eq!(capture(), "GLOBAL_MARKER");
        fs::write(&lower, "").unwrap();
    }
}

// RFC §6.1-13 — 工具退出 0、7、124 时，当前 IR、落盘 WAL 和重新加载的 IR 都含正确的轮次、退出码与过滤后文本。
// RFC §9.3-02 — 两个 post-tool 过滤器依次改写文本，只有最终结果到达 WAL 和 Provider。
#[test]
fn post_tool_filters_chain_in_order_and_only_the_final_text_reaches_wal_and_provider() {
    for status in [0, 7, 124] {
        let fx = Fixture::new();
        let root = &fx.dir.0;
        fs::write(root.join("case.agent"), "fixture").unwrap();
        fs::write(root.join("sample"), "RAW fixture 文").unwrap();
        let hooks = root.join("config/anix/hooks/post-tool.d");
        for (name, capture, substitution) in [
            ("40-first", "first.json", "s/RAW/INTERMEDIATE/g"),
            ("50-last", "last.json", "s/INTERMEDIATE/FINAL/g"),
        ] {
            executable(
                &hooks.join(name),
                &format!(
                    "#!/bin/sh\nset -eu\n/bin/cat > {capture}\n/usr/bin/sed '{substitution}' {capture}\n"
                ),
            );
        }
        let cmd = format!("printf public > \"$ANIX_OUT\"; /bin/cat sample; exit {status}");
        scripted_provider(
            &fx,
            &[
                with_tools([tool("filtered-call", &cmd)]),
                success_text("done"),
                success_text("resumed"),
            ],
        );
        let args = [
            "--output",
            "explicit",
            "-q",
            "-y",
            "--shell",
            "/bin/sh",
            "-c",
            "history",
            "case.agent",
            "--",
            "go",
        ];
        let run = fx.run(fx.anix().args(args));
        expect_code(&run, 0);
        assert_eq!(run.stdout, b"public");
        let read_hook_input = |name: &str| {
            serde_json::from_slice::<serde_json::Value>(&fs::read(root.join(name)).unwrap())
                .unwrap()
        };
        let first = read_hook_input("first.json");
        let last = read_hook_input("last.json");
        assert_eq!(first["cmd"], cmd);
        assert_eq!(first["exit_code"], status);
        assert_eq!(first["tool_call_id"], "filtered-call");
        assert!(first["turn_id"].is_u64());
        assert_eq!(first["output"], "RAW fixture 文");
        let mut between = first.clone();
        between["output"] = "INTERMEDIATE fixture 文".into();
        assert_eq!(last, between);

        let resume = fx.run(fx.anix().args(args));
        expect_code(&resume, 0);
        assert!(resume.stdout.is_empty());
        for name in ["request-2", "history", "request-3"] {
            let records = jsonl(&root.join(name));
            let tools: Vec<_> = records.iter().filter(|r| r["role"] == "tool").collect();
            assert_eq!(tools.len(), 1, "{name}");
            let item = tools[0];
            assert_eq!(item["tool_call_id"], first["tool_call_id"]);
            assert_eq!(item["turn_id"], first["turn_id"]);
            // §7.1 specifies the timeout status explicitly. Ordinary nonzero
            // statuses still pass through the hook, with no mandated error flag.
            if status == 124 {
                assert_eq!(item["is_error"], true, "{name}");
            }
            if status == 0 {
                assert!(!item["is_error"].as_bool().unwrap_or(false), "{name}");
            }
            let turn_id = first["turn_id"].as_u64().unwrap();
            assert_eq!(
                item["text"],
                format!("[t{turn_id}] exit={status}\nFINAL fixture 文"),
                "{name}"
            );
        }
    }
}
