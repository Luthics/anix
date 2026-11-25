//! RFC v1.1 §§3.1, 3.3–3.4, 8.1–8.2.
use std::fs;

use anix_core::frontmatter::parse_script_str;
use serde_json::json;

use super::{Fixture, expect_code, jsonl};

// RFC §3.3-02 — 遍历允许与禁止的字符，检查显式 name 的 ASCII 字母、数字及 . _ - 边界。
#[test]
fn agent_name_has_exactly_the_documented_ascii_alphabet() {
    for byte in 0..=127u8 {
        let name = format!("a{}z", char::from(byte));
        // JSON string quoting is also valid YAML double-quoted syntax, except
        // JSON's handling of DEL; either spelling represents a forbidden byte.
        let input = format!("---\nname: {}\n---\nbody", json!(name));
        let parsed = parse_script_str(&input, "fixture");
        let allowed = byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-');
        assert_eq!(parsed.is_ok(), allowed, "name byte {byte:#04x}");
        if allowed {
            assert_eq!(
                parsed.unwrap().frontmatter.name.as_deref(),
                Some(name.as_str())
            );
        }
    }
    for name in ["名字", "café", "🚀"] {
        assert!(parse_script_str(&format!("---\nname: {name}\n---\nbody"), "fixture").is_err());
    }
}

// RFC §3.3-03 — 错误类型的 provider 字段在解析阶段拒绝。
// RFC §3.3-04 — model 输入必须满足字符串约束。
// RFC §3.3-06 — 验证 max_tokens 的整数类型、下限及 uint32 上界。
// RFC §3.3-09 — 未知 approval 枚举值不能解析为有效 Frontmatter。
// RFC §3.3-10 — 拒绝 stderr 的未知枚举值或错误类型。
#[test]
fn declared_fields_validate_type_enums_and_uint32_boundaries() {
    for (field, valid, invalid) in [
        (
            "max_turns",
            &["1", "2", "4294967295"][..],
            &["0", "-1", "1.5", "4294967296", "'1'"][..],
        ),
        (
            "max_tokens",
            &["1024", "1025", "4294967295"],
            &["0", "1023", "-1", "4294967296"],
        ),
        (
            "max_output_tokens",
            &["0", "1", "1023"],
            &["-1", "4294967296", "false"],
        ),
        (
            "compact_threshold",
            &["0", "0.25", "0.999", "1", "4096"],
            &["-1", "1.5", "true"],
        ),
        (
            "approval",
            &["ask", "auto", "deny"],
            &["ASK", "off", "yes", "true"],
        ),
        (
            "stderr",
            &["auto", "explicit"],
            &["quiet", "Explicit", "false"],
        ),
        ("context", &["''", "'my file'"], &["false", "[]"]),
        ("model", &["m"], &["''", "{}", "true"]),
        ("provider", &["p"], &["null", "[]"]),
        ("shell", &["msh", "/bin/sh"], &["true", "[]"]),
        (
            "params",
            &["{}", "{nested: [1, true]}"],
            &["null", "[]", "'key=value'"],
        ),
    ] {
        for (values, expected) in [(valid, true), (invalid, false)] {
            for value in values {
                let source = format!("---\n{field}: {value}\n---\nbody");
                assert_eq!(
                    parse_script_str(&source, "fixture").is_ok(),
                    expected,
                    "{field}: {value}"
                );
            }
        }
    }
}

// RFC §3.1-07 — 按比例取整或使用绝对阈值，0 与达到上限的配置关闭自动压缩。
// RFC §3.3-08 — 小数阈值向下取整，整数表示绝对值，0 或超过上下文时禁用。
#[test]
fn threshold_fraction_floors_absolute_threshold_and_disable_cases() {
    for (input, limit, expected) in [
        ("0", 1024, None),
        ("0.5", 1025, Some(512)),
        ("0.8", 128000, Some(102400)),
        ("1", 1024, Some(1)),
        ("1023", 1024, Some(1023)),
        ("1024", 1024, None),
        ("1025", 1024, None),
    ] {
        let script = parse_script_str(
            &format!("---\ncompact_threshold: {input}\n---\nbody"),
            "case",
        )
        .unwrap();
        assert_eq!(
            script.frontmatter.compact_threshold.unwrap().resolve(limit),
            expected,
            "{input}"
        );
    }
}

// RFC §3.3-04 — 在五层配置的 32 种子集中检查最高优先级的 model。
// RFC §3.4-02 — 枚举五层配置开关的 32 种子集，读取 Provider 接到的模型与输出上限。
#[test]
fn every_subset_of_five_layers_selects_highest_priority_model_and_output_limit() {
    const MOCK: &str = "\n[providers.fixture]\nbin = 'record-provider'\nmode='oneshot'\n";
    let fx = Fixture::new();
    let root = &fx.dir.0;
    fs::create_dir(root.join(".anix")).unwrap();
    for mask in 0..32 {
        let mut global = String::from("[defaults]\nprovider='fixture'\nmodel='fallback'\n");
        let mut project = String::from("[defaults]\n");
        let mut front = String::new();
        let mut cmd = fx.anix();
        let mut chosen = "fallback";
        let mut limit = None;
        for (idx, label) in ["global", "project", "front", "env", "cli"]
            .iter()
            .copied()
            .enumerate()
        {
            if mask & (1 << idx) == 0 {
                continue;
            }
            chosen = label;
            let output = idx + 1;
            limit = Some(json!(output));
            match label {
                "global" => {
                    global = format!(
                        "[defaults]\nprovider='fixture'\nmodel='{label}'\nmax_output_tokens={output}\n"
                    )
                }
                "project" => {
                    project.push_str(&format!("model='{label}'\nmax_output_tokens={output}\n"))
                }
                "front" => {
                    front = format!("---\nmodel: {label}\nmax_output_tokens: {output}\n---\n")
                }
                "env" => {
                    cmd.env("ANIX_MODEL", label)
                        .env("ANIX_MAX_OUTPUT_TOKENS", output.to_string());
                }
                "cli" => {
                    cmd.args(["--model", label, "--max-output-tokens", &output.to_string()]);
                }
                _ => unreachable!(),
            }
        }
        fs::write(root.join("config/anix/config.toml"), global + MOCK).unwrap();
        fs::write(root.join(".anix/config.toml"), project).unwrap();
        fs::write(root.join("case.agent"), front + "body").unwrap();
        let result = fx.run(cmd.args(["case.agent", "--", "go"]));
        expect_code(&result, 0);
        assert_eq!(
            fs::read_to_string(root.join("model")).unwrap(),
            chosen,
            "mask {mask:05b}"
        );
        assert_eq!(
            jsonl(&fx.capture)[0].get("max_output_tokens"),
            limit.as_ref(),
            "mask {mask:05b}"
        );
    }
}

// RFC §3.1-06 — 逐一启用五层覆盖的组合，检查最后生效的 Provider 与 shell。
// RFC §3.1-07 — 从五层值的组合读取生效的轮数、Token 预算和压缩配置。
// RFC §3.1-11 — 在配置层组合中读取解析后的 idle_timeout 值。
// RFC §3.3-09 — 逐层指定 approval，检查解析出的有效策略及无覆盖的默认值。
// RFC §3.3-10 — 逐层指定 stderr，检查生效模式及无覆盖的默认值。
// RFC §3.4-02 — 对全部 32 种配置组合，核对可观测的标量和布尔选项。
#[test]
fn scalar_settings_resolve_for_every_subset_of_the_five_override_layers() {
    use anix_core::cli::Cli;
    use anix_core::config::{AgentPaths, ResolvedConfig};
    use clap::Parser;

    // Exercise the same resolver as the CLI. Launch twice to vary inherited
    // variables without touching environment shared by parallel Rust tests.
    const CHILD: &str = "ANIX_TEST_RESOLVER_CHILD";
    fn fields(layer: u32) -> Vec<(&'static str, serde_json::Value)> {
        vec![
            ("provider", json!(format!("provider-{layer}"))),
            ("shell", json!(format!("shell-{layer}"))),
            ("max_turns", json!(10 + layer)),
            ("max_tokens", json!(4000 + layer)),
            ("compact_threshold", json!(100 + layer)),
            (
                "approval",
                json!(["ask", "deny", "auto"][(layer % 3) as usize]),
            ),
            (
                "stderr",
                json!(if layer % 2 == 1 { "explicit" } else { "auto" }),
            ),
        ]
    }
    let text = |value: &serde_json::Value| {
        value
            .as_str()
            .map(String::from)
            .unwrap_or_else(|| value.to_string())
    };
    let Some(child_mode) = std::env::var_os(CHILD) else {
        for has_env in [false, true] {
            let fx = Fixture::new();
            for layer in 1..=5 {
                std::os::unix::fs::symlink("/bin/sh", fx.dir.0.join(format!("shell-{layer}")))
                    .unwrap();
            }
            let exe = std::env::current_exe().unwrap();
            let mut child = fx.command_for(exe.to_str().unwrap());
            child.env(CHILD, if has_env {"present"} else {"absent"})
                .args(["--exact", "configuration::scalar_settings_resolve_for_every_subset_of_the_five_override_layers"]);
            if has_env {
                for (key, value) in fields(4) {
                    child.env(format!("ANIX_{}", key.to_ascii_uppercase()), text(&value));
                }
                child
                    .env("MSH_TIMEOUT_SEC", "7")
                    .env("ANIX_CONTEXT", "env.wal")
                    .env("ANIX_INBOX", "env-box")
                    .env("ANIX_IDLE_TIMEOUT", "41");
            }
            let result = fx.run(&mut child);
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stdout)
            );
            assert!(String::from_utf8_lossy(&result.stdout).contains("1 passed"));
        }
        return;
    };
    let has_env = child_mode == "present";
    let fx = Fixture::new();
    let root = &fx.dir.0;
    fs::create_dir(root.join(".anix")).unwrap();
    let config = |layer: u32, enabled: bool| {
        let mut s = String::from("[defaults]\nmodel='fixture'\n");
        if enabled {
            for (key, value) in fields(layer) {
                s.push_str(&format!("{key}={value}\n"));
            }
        }
        s
    };
    let providers: String = (1..=5)
        .map(|i| format!("[providers.provider-{i}]\nbin='local'\n"))
        .collect();
    for mask in 0..32_u32 {
        let enabled = |layer: u32| mask & (1 << (layer - 1)) != 0;
        if enabled(4) != has_env {
            continue;
        }
        fs::write(
            root.join("config/anix/config.toml"),
            config(1, enabled(1)) + &providers,
        )
        .unwrap();
        fs::write(root.join(".anix/config.toml"), config(2, enabled(2))).unwrap();
        let mut source = String::from("---\n");
        if enabled(3) {
            for (key, value) in fields(3) {
                source.push_str(&format!("{key}: {value}\n"));
            }
            source.push_str("context: front.wal\n");
        }
        source.push_str("---\nbody");
        let mut args = vec![String::from("anix")];
        if enabled(5) {
            for (key, value) in fields(5) {
                args.extend([format!("--{}", key.replace('_', "-")), text(&value)]);
            }
            args.extend(
                ["-c", "cli.wal", "--inbox", "cli-box", "--idle-timeout", "0"].map(String::from),
            );
        }
        args.push(String::from("case.agent"));
        let cwd = root.canonicalize().unwrap();
        let mut paths = AgentPaths::discover(&cwd).unwrap();
        paths.global_config_dir = root.join("config/anix");
        let cfg = ResolvedConfig::resolve(
            &Cli::try_parse_from(args).unwrap(),
            &parse_script_str(&source, "case").unwrap(),
            paths,
        )
        .unwrap();
        let winner = (1..=5).rev().find(|&n| enabled(n)).unwrap_or(0);
        assert_eq!(cfg.msh_timeout_sec, if has_env { 7 } else { 120 });
        let expected = if winner == 0 {
            json!(["openai-responses", "msh", 20, 128000, 102400, "ask", "auto"])
        } else {
            serde_json::Value::Array(fields(winner).into_iter().map(|(_, v)| v).collect())
        };
        assert_eq!(
            json!([
                cfg.provider,
                cfg.shell,
                cfg.max_turns,
                cfg.max_tokens,
                cfg.effective_compact_threshold(),
                cfg.approval,
                cfg.stderr
            ]),
            expected,
            "mask {mask:05b}"
        );
        let context = if enabled(5) {
            Some("cli.wal")
        } else if has_env {
            Some("env.wal")
        } else if enabled(3) {
            Some("front.wal")
        } else {
            None
        };
        let inbox = if enabled(5) {
            Some("cli-box")
        } else if has_env {
            Some("env-box")
        } else {
            None
        };
        assert_eq!(cfg.context, context.map(|n| cwd.join(n)), "mask {mask:05b}");
        assert_eq!(cfg.inbox, inbox.map(|n| cwd.join(n)), "mask {mask:05b}");
        assert_eq!(
            cfg.idle_timeout_sec,
            if enabled(5) {
                0
            } else if has_env {
                41
            } else {
                300
            },
            "mask {mask:05b}"
        );
    }
}

// RFC §3.1-07 — 单轮输出等于上下文预算时拒绝，0 不进入 IR。
// RFC §3.3-07 — 0 不写入 header，非零上限必须严格小于有效上下文预算。
// RFC §9.1-06 — 单轮限制为 0 时 header 省略该字段，合法非零值按原值写入。
#[test]
fn output_limit_is_strictly_below_effective_context_budget_zero_omits_it() {
    let fx = Fixture::new();
    fs::write(
        fx.dir.0.join("case.agent"),
        "---\nmax_tokens: 1024\n---\nPrompt",
    )
    .unwrap();
    for (limit, code) in [
        (0, 0),
        (1, 0),
        (1023, 0),
        (1024, 2),
        (1025, 2),
        (u32::MAX, 2),
    ] {
        if fx.capture.exists() {
            fs::remove_file(&fx.capture).unwrap();
        }
        let result = fx.run(fx.anix().args([
            "--max-output-tokens",
            &limit.to_string(),
            "case.agent",
            "--",
            "go",
        ]));
        expect_code(&result, code);
        assert_eq!(fx.capture.exists(), code == 0);
        if code == 0 {
            let ir = jsonl(&fx.capture);
            assert_eq!(
                ir[0].get("max_output_tokens"),
                (limit > 0).then(|| json!(limit)).as_ref()
            );
        }
    }
}

// RFC §3.3-12 — params 的扩展键与浅合并。
// RFC §3.4-07 — CLI 中的 JSON 参数按键覆盖，未冲突的键及值类型保留。
#[test]
fn params_merge_by_key_and_cli_values_preserve_json_types() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    let global = root.join("config/anix/config.toml");
    let original = fs::read_to_string(&global).unwrap();
    fs::write(
        global,
        original + "\n[providers.fixture.params]\nglobal=1\nall='g'\ntree={old=1}\n",
    )
    .unwrap();
    fs::create_dir(root.join(".anix")).unwrap();
    fs::write(
        root.join(".anix/config.toml"),
        "[providers.fixture.params]\nproject=2\nall='p'\ntree={new=2}\n",
    )
    .unwrap();
    fs::write(
        root.join("case.agent"),
        "---\nparams: {front: 3, all: f}\n---\nbody",
    )
    .unwrap();
    let result = fx.run(fx.anix().args([
        "--param",
        "all=cli",
        "--param",
        "arr=[false,1]",
        "--param",
        "nil=null",
        "--param",
        "word=a=b",
        "--param",
        "obj={\"a\":true}",
        "case.agent",
        "--",
        "go",
    ]));
    expect_code(&result, 0);
    assert_eq!(
        jsonl(&fx.capture)[0]["params"],
        json!({"global":1,"project":2,"front":3,"all":"cli","tree":{"new":2},"arr":[false,1],"nil":null,"word":"a=b","obj":{"a":true}})
    );
}

// RFC §3.1-05 — ANIX_CONTEXT 为空串时即使脚本声明 context 也不创建 WAL。
// RFC §3.3-11 — 环境将 context 显式置空时关闭文件持久化。
// RFC §3.4-04 — 显式空串覆盖会话路径。
#[test]
fn empty_context_override_is_distinct_from_an_unset_variable() {
    for (env_path, cli_path, expected) in [
        (None, None, Some("front.wal")),
        (Some("env.wal"), None, Some("env.wal")),
        (Some(""), None, None),
        (Some("env.wal"), Some(""), None),
        (Some(""), Some("cli.wal"), Some("cli.wal")),
    ] {
        let fx = Fixture::new();
        fs::write(
            fx.dir.0.join("case.agent"),
            "---\ncontext: front.wal\n---\nbody",
        )
        .unwrap();
        let mut cmd = fx.anix();
        if let Some(path) = env_path {
            cmd.env("ANIX_CONTEXT", path);
        }
        if let Some(path) = cli_path {
            cmd.args(["-c", path]);
        }
        expect_code(&fx.run(cmd.args(["case.agent", "--", "go"])), 0);
        for name in ["front.wal", "env.wal", "cli.wal"] {
            assert_eq!(
                fx.dir.0.join(name).exists(),
                expected == Some(name),
                "env={env_path:?} cli={cli_path:?} file={name}"
            );
        }
    }
}

// RFC §8.1-04 — 按最近配置、Git 仓库及 HOME 边界分别检查向上查找结果。
#[test]
fn project_search_stops_at_nearest_configuration_git_or_home_boundary() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    let outer = root.join("outer");
    let project = outer.join("project");
    let nested = project.join("src/nested");
    fs::create_dir_all(&nested).unwrap();
    fs::create_dir(outer.join(".anix")).unwrap();
    fs::write(outer.join(".anix/config.toml"), "[defaults]\nmodel='outer'").unwrap();
    fs::write(nested.join("case.agent"), "body").unwrap();
    let run = |expected| {
        expect_code(
            &fx.run(
                fx.anix()
                    .current_dir(&nested)
                    .args(["case.agent", "--", "go"]),
            ),
            0,
        );
        assert_eq!(fs::read_to_string(root.join("model")).unwrap(), expected);
    };
    run("outer");
    fs::create_dir(project.join(".git")).unwrap();
    run("fixture-model");
    fs::create_dir(project.join(".anix")).unwrap();
    fs::write(
        project.join(".anix/config.toml"),
        "[defaults]\nmodel='project'",
    )
    .unwrap();
    run("project");
    fs::create_dir(nested.join(".anix")).unwrap();
    run("fixture-model"); // Even an empty nearest .anix/ stops the upward search.
    fs::remove_dir(nested.join(".anix")).unwrap();
    expect_code(
        &fx.run(
            fx.anix()
                .current_dir(&nested)
                .env("HOME", root.join("outer/project/src"))
                .args(["case.agent", "--", "go"]),
        ),
        0,
    );
    assert_eq!(
        fs::read_to_string(project.join("src/model")).unwrap(),
        "fixture-model"
    );
}

// RFC §3.3-03 — 模拟已注册的 Provider 读取所选配置及注入值。
// RFC §9.1-01 — oneshot Provider 从进程环境读取所选模型。
// RFC §9.2-09 — 有效 model、base URL、timeout 与密钥来源。
#[test]
fn provider_resolution_injects_model_base_url_timeout_and_env_or_fallback_key() {
    use super::executable;
    let fx = Fixture::new();
    let root = &fx.dir.0;
    let global = root.join("config/anix/config.toml");
    fs::write(
        &global,
        fs::read_to_string(&global).unwrap()
            + r#"
[providers.alternate]
bin="observe-env"
mode="oneshot"
base_url="http://127.0.0.1:1/fixture"
timeout_sec=27
api_key_env="LOCAL_KEY_FIXTURE"
api_key="configured-placeholder"
"#,
    )
    .unwrap();
    fs::write(
        root.join("case.agent"),
        "---\nprovider: fixture\nmodel: front\n---\nbody",
    )
    .unwrap();
    executable(
        &root.join("observe-env"),
        r#"#!/bin/sh
/bin/cat >/dev/null
printf '%s\n' "$ANIX_MODEL" "$ANIX_PROVIDER_BASE_URL" "$ANIX_PROVIDER_TIMEOUT_SEC" "$ANIX_PROVIDER_API_KEY" > actual
printf '%s\n' '{"type":"text_delta","text":"ok"}' '{"type":"done","finish_reason":"stop"}'
"#,
    );
    for (value, expected) in [
        (None, "configured-placeholder"),
        (Some(""), "configured-placeholder"),
        (Some("env-placeholder"), "env-placeholder"),
    ] {
        let mut cmd = fx.anix();
        cmd.env("ANIX_PROVIDER", "alternate")
            .env("ANIX_MODEL", "env-model")
            .args(["case.agent", "--", "go"]);
        if let Some(v) = value {
            cmd.env("LOCAL_KEY_FIXTURE", v);
        }
        expect_code(&fx.run(&mut cmd), 0);
        let actual = fs::read_to_string(root.join("actual")).unwrap();
        assert_eq!(
            actual.lines().collect::<Vec<_>>(),
            ["env-model", "http://127.0.0.1:1/fixture", "27", expected]
        );
    }
}

// RFC §3.1-08 — 对照 ANIX_YES、ANIX_APPROVAL 与 CLI，检查命令的磁盘副作用及拒绝结果。
// RFC §3.4-03 — 识别 ANIX_YES 的 1 与 true，并检查与 CLI 审批覆盖的组合。
// RFC §5.1-01 — 用环境别名与 CLI 切换审批策略，检查是否产生预期的命令副作用。
#[test]
fn environment_approval_aliases_and_cli_override_produce_the_expected_tool_effects() {
    use super::{scripted_provider, success_text, tool, with_tools};
    for (yes, approval, opts, allowed) in [
        (None, "deny", &[][..], false),
        (None, "auto", &[][..], true),
        (Some("1"), "", &[][..], true),
        (Some("true"), "", &[][..], true),
        (Some("true"), "", &["--approval", "deny"][..], false),
    ] {
        let fx = Fixture::new();
        let root = &fx.dir.0;
        fs::write(
            root.join("case.agent"),
            "---\napproval: deny\nstderr: auto\n---\nbody",
        )
        .unwrap();
        scripted_provider(
            &fx,
            &[
                with_tools([tool("x", "printf ran > marker")]),
                success_text("human prose"),
            ],
        );
        let mut cmd = fx.anix();
        cmd.env("ANIX_SHELL", "/bin/sh")
            .env("ANIX_STDERR", "explicit")
            .env("ANIX_OUTPUT", "none");
        if let Some(y) = yes {
            cmd.env("ANIX_YES", y);
        }
        if !approval.is_empty() {
            cmd.env("ANIX_APPROVAL", approval);
        }
        let out = fx.run(cmd.args(opts).args(["case.agent", "--", "go"]));
        expect_code(&out, 0);
        assert!(out.stderr.is_empty() && out.stdout.is_empty());
        assert_eq!(root.join("marker").exists(), allowed);
        let req = jsonl(&root.join("request-2"));
        assert_eq!(req[0]["meta"]["stderr_mode"], "explicit");
        assert_eq!(
            req.last().unwrap()["is_error"].as_bool().unwrap_or(false),
            !allowed
        );
    }
}

// RFC §3.4-03 — 非法环境配置以 2 退出且不启动 Provider。
#[test]
fn invalid_resolved_environment_values_exit_two_before_contacting_the_provider() {
    for (key, value) in [
        ("ANIX_PROVIDER", "not-registered"),
        ("ANIX_MAX_TURNS", "0"),
        ("ANIX_MAX_TURNS", "4294967296"),
        ("ANIX_MAX_TOKENS", "1023"),
        ("ANIX_MAX_OUTPUT_TOKENS", "4294967296"),
        ("ANIX_COMPACT_THRESHOLD", "-1"),
        ("ANIX_APPROVAL", "unknown"),
        ("ANIX_STDERR", "unknown"),
        ("ANIX_VISION", "automatic"),
        ("ANIX_IDLE_TIMEOUT", "-1"),
    ] {
        let fx = Fixture::new();
        fs::write(fx.dir.0.join("case.agent"), "body").unwrap();
        let out = fx.run(fx.anix().env(key, value).args(["case.agent", "--", "go"]));
        assert_eq!(
            out.status.code(),
            Some(2),
            "{key}={value}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(!out.stderr.is_empty() && out.stdout.is_empty() && !fx.capture.exists());
    }
}

// RFC §6.5-05 — 全局或项目 TOML 出现未知键时以 2 退出且未推理。
#[test]
fn global_and_project_config_reject_unknown_keys_before_any_inference() {
    for where_ in ["config/anix", ".anix"] {
        for text in [
            "unrecognized=true",
            "[defaults]\nunknown_limit=3",
            "[providers.fixture]\nunknown_bin='other'",
            "[providers.fixture]\nmode='daemon'",
            "[vision]\nunrecognized=1",
            "[approval]\nunrecognized=1",
        ] {
            let fx = Fixture::new();
            let root = &fx.dir.0;
            let dir = root.join(where_);
            fs::create_dir_all(&dir).unwrap();
            let config = dir.join("config.toml");
            fs::write(config, text).unwrap();
            fs::write(
                root.join("case.agent"),
                "---\nmodel: fixture-model\n---\nbody",
            )
            .unwrap();
            // The default provider uses coprocess mode. Record unexpected startup
            // immediately so a config-parsing regression cannot stall on stdin.
            super::executable(
                &root.join("anix-provider-openai-responses"),
                "#!/bin/sh\n: > \"$HOME/request.jsonl\"\nexit 1\n",
            );
            let out = fx.run(fx.anix().args([
                "--provider",
                "openai-responses",
                "case.agent",
                "--",
                "go",
            ]));
            assert_eq!(
                out.status.code(),
                Some(2),
                "{where_}: {text:?}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            assert!(!out.stderr.is_empty() && !fx.capture.exists());
        }
    }
}

// RFC §3.3-08 — 在 uint32 最大值两侧探测绝对阈值，溢出应先被拒绝。
#[test]
fn absolute_frontmatter_threshold_must_fit_uint32_before_the_disable_rule_applies() {
    let mut unexpected = Vec::new();
    for (value, expected) in [
        (4294967294u64, 0),
        (4294967295, 0),
        (4294967296, 2),
        (4294967297, 2),
        (u64::MAX, 2),
    ] {
        let fx = Fixture::new();
        fs::write(
            fx.dir.0.join("case.agent"),
            format!("---\nmax_tokens: 1024\ncompact_threshold: {value}\n---\nbody"),
        )
        .unwrap();
        let out = fx.run(
            fx.anix()
                .args(["-q", "--output", "none", "case.agent", "--", "go"]),
        );
        if out.status.code() != Some(expected) || fx.capture.exists() != (expected == 0) {
            unexpected.push((value, out.status.code(), fx.capture.exists()));
        } else if expected == 2 {
            assert!(!out.stderr.is_empty());
        }
        assert!(out.stdout.is_empty());
    }
    assert!(
        unexpected.is_empty(),
        "threshold, exit, provider reached: {unexpected:?}"
    );
}

// RFC §3.1-09 — --param 的 JSON 转为对应类型，无法解析时保留原字符串。
#[test]
fn cli_params_preserve_json_literals_and_the_exact_fallback_string() {
    let fx = Fixture::new();
    fs::write(fx.dir.0.join("case.agent"), "body").unwrap();
    let cases = [
        ("flag", " true ", json!(true)),
        ("empty", "", json!("")),
        ("spaces", " \t ", json!(" \t ")),
        ("quoted", r#""null""#, json!("null")),
        ("nothing", "null", json!(null)),
        ("equals", "x==y ", json!("x==y ")),
        (
            "number",
            "9007199254740993",
            json!(9_007_199_254_740_993u64),
        ),
        ("fraction", "-1.25e2", json!(-125.0)),
        ("leading_zero", "01", json!("01")),
        ("array", "[1,]", json!("[1,]")),
        ("unfinished", "{\"a\":", json!("{\"a\":")),
        (
            "key.with.dots",
            r#"{"b":["\u03a9",false]}"#,
            json!({"b":["Ω",false]}),
        ),
    ];
    let mut cmd = fx.anix();
    let mut params = json!({});
    for (key, raw, expected) in cases {
        cmd.arg("--param").arg(format!("{key}={raw}"));
        params[key] = expected;
    }
    expect_code(&fx.run(cmd.args(["-q", "case.agent", "--", "go"])), 0);
    assert_eq!(jsonl(&fx.capture)[0]["params"], params);
}

// RFC §3.1-09 — 缺少有效 KEY=VALUE 时使用错误码 2，WAL 与 Provider 请求均不存在。
#[test]
fn malformed_cli_param_stops_before_provider_or_wal_creation() {
    for param in ["without-separator", "=value", ""] {
        let fx = Fixture::new();
        fs::write(fx.dir.0.join("case.agent"), "body").unwrap();
        let out = fx.run(fx.anix().args([
            "-c",
            "history",
            "--param",
            "valid=true",
            "--param",
            param,
            "case.agent",
            "--",
            "go",
        ]));
        expect_code(&out, 2);
        assert!(!out.stderr.is_empty());
        assert!(out.stdout.is_empty());
        assert!(!fx.capture.exists());
        assert!(!fx.dir.0.join("history").exists(), "param={param:?}");
    }
}

// RFC §3.1-05 — 不同工作目录下运行同一脚本，使用各自的相对 WAL 路径。
// RFC §3.3-11 — 同一脚本从两个 cwd 启动时，各自生成对应的相对 WAL。
// RFC §8.1-04 — 从两个 cwd 调用同一脚本时使用各自项目配置。
// RFC §9.1-08 — 从不同 cwd 启动时，实际 Provider header.meta.cwd 记录各自目录。
#[test]
fn a_shared_script_resolves_its_context_and_project_configuration_from_each_cwd() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    let scripts = root.join("scripts");
    fs::create_dir_all(scripts.join(".anix")).unwrap();
    fs::write(
        scripts.join(".anix/config.toml"),
        "[defaults]\nmodel='beside-script'",
    )
    .unwrap();
    let script = scripts.join("shared.agent");
    fs::write(&script, "---\ncontext: session.jsonl\n---\nshared prompt").unwrap();
    for project in ["one", "two"] {
        let dir = root.join(project);
        fs::create_dir_all(dir.join(".anix")).unwrap();
        fs::create_dir(dir.join("nested")).unwrap();
        fs::write(
            dir.join(".anix/config.toml"),
            format!("[defaults]\nmodel='model-{project}'"),
        )
        .unwrap();
    }
    for (project, input, prior) in [
        ("one", "a", None),
        ("two", "b", None),
        ("one", "c", Some("a")),
    ] {
        let cwd = root.join(project).join("nested");
        let out = fx.run(
            fx.anix()
                .current_dir(&cwd)
                .arg("-q")
                .arg(&script)
                .args(["--", input]),
        );
        expect_code(&out, 0);
        let ir = jsonl(&fx.capture);
        assert_eq!(
            ir[0]["meta"]["cwd"],
            cwd.canonicalize().unwrap().to_str().unwrap()
        );
        assert_eq!(
            fs::read_to_string(root.join("model")).unwrap(),
            format!("model-{project}")
        );
        let expected: Vec<_> = prior
            .into_iter()
            .chain([input])
            .map(|x| format!("<args>{x}</args>"))
            .collect();
        for rows in [&ir, &jsonl(&cwd.join("session.jsonl"))] {
            let users: Vec<_> = rows
                .iter()
                .filter(|r| r["role"] == "user")
                .map(|r| r["text"].as_str().unwrap())
                .collect();
            assert_eq!(users, expected);
        }
        assert!(!root.join(project).join("session.jsonl").exists());
    }
    assert!(!scripts.join("session.jsonl").exists());
}

// RFC §3.3-12 — params 的扩展键与浅合并。
// RFC §3.4-07 — 只合并被选中 Provider 的 params，同名对象整体替换。
// RFC §8.2-01 — 所选 Provider 的 params 浅合并。
#[test]
fn only_the_selected_providers_defaults_feed_the_shallow_params_merge() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    let cfg = root.join("config/anix/config.toml");
    let global = fs::read_to_string(&cfg).unwrap();
    fs::write(
        &cfg,
        global
            + r#"
[providers.fixture.params]
only_a = 1
tree = {from_global_a = 1}
[providers.alternate]
bin = "record-provider"
mode = "oneshot"
[providers.alternate.params]
only_b = 2
tree = {from_global_b = 2}
"#,
    )
    .unwrap();
    fs::create_dir(root.join(".anix")).unwrap();
    fs::write(
        root.join(".anix/config.toml"),
        r#"
[providers.fixture.params]
layer = "project-a"
tree = {from_project_a = 3}
[providers.alternate.params]
layer = "project-b"
tree = {from_project_b = 4}
"#,
    )
    .unwrap();
    fs::write(
        root.join("case.agent"),
        "---\nprovider: alternate\nparams: {common: 'script', overridable: [1]}\n---\nbody",
    )
    .unwrap();
    for (env, cli, provider) in [
        (None, None, "alternate"),
        (Some("fixture"), None, "fixture"),
        (Some("alternate"), Some("fixture"), "fixture"),
        (Some("fixture"), Some("alternate"), "alternate"),
    ] {
        let mut cmd = fx.anix();
        if let Some(v) = env {
            cmd.env("ANIX_PROVIDER", v);
        }
        if let Some(v) = cli {
            cmd.args(["--provider", v]);
        }
        expect_code(
            &fx.run(cmd.args([
                "--param",
                "overridable=null",
                "-q",
                "case.agent",
                "--",
                "go",
            ])),
            0,
        );
        let expected = if provider == "fixture" {
            json!({"only_a":1,"layer":"project-a","tree":{"from_project_a":3},"common":"script","overridable":null})
        } else {
            json!({"only_b":2,"layer":"project-b","tree":{"from_project_b":4},"common":"script","overridable":null})
        };
        assert_eq!(
            jsonl(&fx.capture)[0]["params"],
            expected,
            "env={env:?} cli={cli:?}"
        );
    }
}

// RFC §2-02 — Provider 的命令名依次查找 PATH 和私有目录，显式路径直接执行。
// RFC §3.1-06 — 配置为名称时按 PATH、libexec 查找 Provider，带斜杠时直接使用该路径。
#[test]
fn a_named_provider_searches_path_then_libexec_while_a_path_executes_directly() {
    use super::executable;
    use std::os::unix::fs::PermissionsExt;
    let fx = Fixture::new();
    let root = &fx.dir.0;
    let config = root.join("config/anix/config.toml");
    let original = fs::read_to_string(&config).unwrap();
    let recorder = fs::read_to_string(root.join("record-provider")).unwrap();
    let first = root.join("first");
    let second = root.join("second");
    let fallback = root.join(".local/libexec/anix");
    let direct = root.join("elsewhere");
    for (dir, label) in [
        (&first, "first"),
        (&second, "second"),
        (&fallback, "libexec"),
        (&direct, "direct"),
    ] {
        executable(
            &dir.join("driver"),
            &format!("{recorder}\nprintf '%s' {label} > selected\n"),
        );
    }
    fs::write(root.join("case.agent"), "body").unwrap();
    let search = std::env::join_paths([&first, &second]).unwrap();
    let run = |spec: &str, label: Option<&str>| {
        fs::write(&config, original.replace("record-provider", spec)).unwrap();
        let marker = root.join("selected");
        if marker.exists() {
            fs::remove_file(&marker).unwrap();
        }
        if fx.capture.exists() {
            fs::remove_file(&fx.capture).unwrap();
        }
        let out = fx.run(
            fx.anix()
                .env("PATH", &search)
                .args(["-q", "case.agent", "--", "go"]),
        );
        if let Some(label) = label {
            assert_eq!(
                out.status.code(),
                Some(0),
                "{spec} -> {label}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            assert_eq!(fs::read_to_string(marker).unwrap(), label);
            assert_eq!(jsonl(&fx.capture)[0]["type"], "header");
        } else {
            expect_code(&out, 1);
            assert!(!out.stderr.is_empty());
            assert!(!marker.exists() && !fx.capture.exists());
        }
    };
    run("driver", Some("first"));
    run(direct.join("driver").to_str().unwrap(), Some("direct"));
    run("~/elsewhere/driver", Some("direct"));
    run("./missing/driver", None);
    fs::set_permissions(first.join("driver"), fs::Permissions::from_mode(0o644)).unwrap();
    run("driver", Some("second"));
    fs::remove_file(second.join("driver")).unwrap();
    run("driver", Some("libexec"));
}

// RFC §8.1-01 — XDG_CONFIG_HOME 与 HOME 回退。
// RFC §8.1-02 — XDG_STATE_HOME 与 HOME 回退。
// RFC §8.1-03 — 分别设置 XDG 变量，检查 runtime 目录及其他目录独立回退。
#[test]
fn each_xdg_directory_resolves_independently_when_other_overrides_are_absent() {
    use anix_core::config::AgentPaths;
    use std::path::PathBuf;

    // Resolve the paths in isolated processes. In particular, never create
    // or chmod the actual per-UID fallback directory on a developer machine.
    const MASK: &str = "RFC_XDG_MASK";
    let vars = ["XDG_CONFIG_HOME", "XDG_STATE_HOME", "XDG_RUNTIME_DIR"];
    let Some(mask) = std::env::var_os(MASK) else {
        let fx = Fixture::new();
        let exe = std::env::current_exe().unwrap();
        for mask in 0..8 {
            let mut child = fx.command_for(exe.to_str().unwrap());
            for (bit, key) in vars.iter().enumerate() {
                child.env_remove(key);
                if mask & (1 << bit) != 0 {
                    child.env(key, fx.dir.0.join(key));
                }
            }
            child.env(MASK, mask.to_string()).args(["--exact",
                "configuration::each_xdg_directory_resolves_independently_when_other_overrides_are_absent"]);
            let out = fx.run(&mut child);
            assert!(
                out.status.success(),
                "mask {mask:03b}: {}",
                String::from_utf8_lossy(&out.stdout)
            );
            assert!(String::from_utf8_lossy(&out.stdout).contains("1 passed"));
        }
        return;
    };
    let mask: u8 = mask.to_str().unwrap().parse().unwrap();
    let home = PathBuf::from(std::env::var_os("HOME").unwrap());
    let uid = std::process::Command::new("/usr/bin/id")
        .arg("-u")
        .output()
        .unwrap();
    assert!(uid.status.success());
    let uid = String::from_utf8(uid.stdout).unwrap();
    let fallback = [
        home.join(".config/anix"),
        home.join(".local/state/anix"),
        PathBuf::from(format!("/tmp/anix-{}", uid.trim())),
    ];
    let [config, state, runtime] = std::array::from_fn(|bit| {
        if mask & (1 << bit) == 0 {
            fallback[bit].clone()
        } else {
            home.join(vars[bit]).join("anix")
        }
    });
    let paths = AgentPaths::discover(&home).unwrap();
    assert_eq!(paths.global_config_file(), config.join("config.toml"));
    assert_eq!(paths.trusted_registry_file(), state.join("trusted.json"));
    assert_eq!(paths.journal_socket_path(), runtime.join("journal.sock"));
}

// RFC §9.1-01 — 更换模型时只改变 Provider 环境，Canonical IR 不含 model。
#[test]
fn selected_model_changes_the_provider_environment_without_entering_the_ir() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    fs::write(root.join("case.agent"), "portable prompt").unwrap();
    for model in ["first-model-marker", "second-model-marker"] {
        expect_code(
            &fx.run(
                fx.anix()
                    .args(["--model", model, "case.agent", "--", "input"]),
            ),
            0,
        );
        assert_eq!(fs::read_to_string(root.join("model")).unwrap(), model);
        let bytes = fs::read_to_string(&fx.capture).unwrap();
        assert!(!bytes.contains(model));
        let rows = jsonl(&fx.capture);
        assert_eq!(rows[0]["type"], "header");
        assert!(rows.iter().all(|row| row.get("model").is_none()));
        assert!(rows[0]["meta"].get("model").is_none());
        assert_eq!(rows[1]["text"], "portable prompt");
        assert_eq!(rows[2]["text"], "<args>input</args>");
    }
}
