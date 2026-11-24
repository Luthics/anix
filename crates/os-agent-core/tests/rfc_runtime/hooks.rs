//! §§4.4, 8.3 and 9.3–9.6. Tests use separate HOME and project roots.
use super::{
    Fixture, executable, expect_code, jsonl, scripted_provider, success_text, tool, with_tools,
};
use std::fs;
use std::time::{Duration, Instant};

// RFC §9.3, §9.1, §6.2
#[test]
fn pre_context_filters_can_rewrite_and_append_conversation_records_before_freezing() {
    use serde_json::json;
    let fx = Fixture::new();
    let root = &fx.dir.0;
    fs::write(root.join("case.agent"), "original prompt").unwrap();
    let hooks = root.join("config/agent/hooks/pre-context.d");
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
    expect_code(&fx.run(fx.agent().args(args)), 0);
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
    expect_code(&fx.run(fx.agent().args(args)), 0);
    let resumed = jsonl(&root.join("request-2"));
    for (original, restored) in first[1..5].iter().zip(&resumed[1..5]) {
        for field in ["role", "text", "turn_id"] {
            assert_eq!(original[field], restored[field], "{field}");
        }
    }
}

// RFC §9.3
#[test]
fn pre_context_failure_forwards_stderr_and_stops_before_inference() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    fs::write(root.join("case.agent"), "body").unwrap();
    let dir = root.join("config/agent/hooks/pre-context.d");
    executable(
        &dir.join("51-fail"),
        "#!/bin/sh\n/bin/cat > pre-input\nprintf specific-pre-failure >&2\nexit 7\n",
    );
    executable(
        &dir.join("52-next"),
        "#!/bin/sh\n: > should-not-run\n/bin/cat\n",
    );
    let out = fx.run(fx.agent().args(["case.agent", "--", "start"]));
    expect_code(&out, 1);
    assert!(
        String::from_utf8(out.stderr)
            .unwrap()
            .contains("specific-pre-failure")
    );
    assert_eq!(jsonl(&root.join("pre-input"))[0]["type"], "header");
    assert!(!root.join("should-not-run").exists() && !fx.capture.exists());
}

// RFC §9.3
#[test]
fn post_tool_failure_stops_before_later_hooks_tools_and_provider_turns() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    fs::write(root.join("case.agent"), "body").unwrap();
    let dir = root.join("config/agent/hooks/post-tool.d");
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
        fx.agent()
            .args(["--shell", "/bin/sh", "-y", "case.agent", "--", "start"]),
    );
    assert!(!out.status.success());
    assert_eq!(jsonl(&root.join("post-input"))[0]["tool_call_id"], "first");
    for path in ["second-tool", "should-not-run", "request-2"] {
        assert!(!root.join(path).exists(), "{path}");
    }
}

// RFC §9.3, §9.5
#[test]
fn on_exit_timeout_is_local_to_one_hook_and_preserves_the_primary_exit_code() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    fs::write(root.join("case.agent"), "body").unwrap();
    let dir = root.join("config/agent/hooks/on-exit.d");
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
        r#"exec "$1" -q case.agent -- go 3>trace"#,
        "fixture",
        env!("CARGO_BIN_EXE_agent"),
    ]));
    expect_code(&out, 1);
    assert!(start.elapsed() >= Duration::from_secs(5));
    let slow = jsonl(&root.join("slow-input"));
    assert_eq!(slow[0]["exit_code"], 1);
    assert_eq!(slow, jsonl(&root.join("next-input")));
    assert!(
        jsonl(&root.join("trace"))
            .iter()
            .any(|r| r.to_string().contains("10-slow"))
    );
    assert!(!String::from_utf8(out.stderr).unwrap().contains("10-slow"));
}

// RFC §9.4
#[test]
fn each_skill_search_layer_wins_once_and_the_body_stays_out_of_the_index() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    let project = root.join("project");
    fs::create_dir(&project).unwrap();
    fs::write(project.join("case.agent"), "body").unwrap();
    fs::remove_file(root.join("config/agent/hooks/pre-context.d/20-skills")).unwrap();
    let layers = [
        project.join(".agent/skills"),
        project.join(".skills"),
        project.join(".claude/skills"),
        root.join("config/agent/skills"),
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
                fx.agent()
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
            fx.agent()
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

// RFC §9.6
#[test]
fn tips_are_restored_after_version_drift_and_fall_back_to_runtime_when_state_is_unwritable() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    fs::write(root.join("case.agent"), "body").unwrap();
    let mask = root.join("config/agent/hooks/pre-context.d/30-tips");
    let state = root.join("state");
    let builtin = state.join("agent/builtin-tips");
    let invoke = || {
        expect_code(
            &fx.run(
                fx.agent()
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
    let file = root.join("run/agent/builtin-tips/how-to-loop.md");
    assert_eq!(fs::read(&file).unwrap(), original);
    assert!(fallback.contains(fs::canonicalize(file).unwrap().to_str().unwrap()));
}

// RFC §9.6
#[test]
fn masking_the_whole_tips_hook_suppresses_both_its_index_and_first_run_files() {
    use std::os::unix::fs::symlink;
    for project in [false, true] {
        for dev_null in [false, true] {
            let fx = Fixture::new();
            let root = &fx.dir.0;
            fs::write(root.join("case.agent"), "body").unwrap();
            let global = root.join("config/agent/hooks/pre-context.d/30-tips");
            fs::remove_file(&global).unwrap();
            let mask = if project {
                root.join(".agent/hooks/pre-context.d/30-tips")
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
                expect_code(&fx.run(fx.agent().args(["trust", "."])), 0);
            }
            for disabled in [true, false] {
                if !disabled {
                    fs::remove_file(&mask).unwrap();
                }
                expect_code(
                    &fx.run(fx.agent().args(["-q", "case.agent", "--", "go"])),
                    0,
                );
                let rows = jsonl(&fx.capture);
                let text = rows[1]["text"].as_str().unwrap();
                if disabled {
                    assert_eq!(text, "body");
                } else {
                    assert!(text.contains("<runtime_tips>"));
                }
                assert_eq!(
                    root.join(".local/state/agent/builtin-tips/how-to-goal.md")
                        .exists(),
                    !disabled
                );
                assert!(!root.join("run/agent/builtin-tips/how-to-goal.md").exists());
            }
        }
    }
}

// RFC §9.6
#[test]
fn an_empty_merged_tips_index_costs_no_prompt_bytes() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    fs::write(root.join("case.agent"), "body").unwrap();
    fs::remove_file(root.join("config/agent/hooks/pre-context.d/30-tips")).unwrap();
    let run = || {
        expect_code(
            &fx.run(fx.agent().args(["-q", "case.agent", "--", "go"])),
            0,
        );
        jsonl(&fx.capture)[1]["text"].as_str().unwrap().to_owned()
    };
    assert!(run().contains("<runtime_tips>"));
    let tips = root.join(".agent/tips");
    fs::create_dir_all(tips.join("nested")).unwrap();
    for entry in fs::read_dir(root.join(".local/state/agent/builtin-tips")).unwrap() {
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

// RFC §9.6
#[test]
fn background_tip_is_indexed_in_ascii_order_and_can_be_overridden_or_masked_individually() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    fs::write(root.join("case.agent"), "body").unwrap();
    fs::remove_file(root.join("config/agent/hooks/pre-context.d/30-tips")).unwrap();
    let filename = "how-to-background.md";
    let builtin = root.join(".local/state/agent/builtin-tips").join(filename);
    let project = root.join(".agent/tips").join(filename);
    fs::create_dir_all(project.parent().unwrap()).unwrap();
    for step in 0..3 {
        if step == 1 {
            fs::write(&project, "local background guide").unwrap();
        }
        if step == 2 {
            fs::write(&project, "").unwrap();
        }
        expect_code(
            &fx.run(fx.agent().args(["-q", "case.agent", "--", "go"])),
            0,
        );
        let rows = jsonl(&fx.capture);
        let text = rows[1]["text"].as_str().unwrap();
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
        let names: Vec<_> = paths.iter().map(|p| p.file_name().unwrap()).collect();
        assert!(names.windows(2).all(|s| s[0] < s[1]), "{names:?}");
        assert!(names.iter().any(|n| *n == "how-to-loop.md"));
        let background: Vec<_> = paths
            .into_iter()
            .filter(|p| p.file_name().unwrap() == filename)
            .collect();
        match step {
            0 => {
                assert_eq!(background, [fs::canonicalize(&builtin).unwrap()]);
                assert!(fs::metadata(&builtin).unwrap().len() > 0);
            }
            1 => assert_eq!(background, [fs::canonicalize(&project).unwrap()]),
            _ => assert!(background.is_empty()),
        }
        assert!(!text.contains("local background guide"));
    }
}
