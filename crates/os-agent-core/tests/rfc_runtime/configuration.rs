//! RFC §§3.1, 3.3–3.4, 8.1–8.2.
use std::fs;

use os_agent_core::frontmatter::parse_script_str;
use serde_json::json;

use super::{Fixture, expect_code, jsonl};

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

#[test]
fn every_subset_of_five_layers_selects_highest_priority_model_and_output_limit() {
    const MOCK: &str = "\n[providers.fixture]\nbin = 'record-provider'\n";
    let fx = Fixture::new();
    let root = &fx.dir.0;
    fs::create_dir(root.join(".agent")).unwrap();
    for mask in 0..32 {
        let mut global = String::from("[defaults]\nprovider='fixture'\nmodel='fallback'\n");
        let mut project = String::from("[defaults]\n");
        let mut front = String::new();
        let mut cmd = fx.agent();
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
                    cmd.env("AGENT_MODEL", label)
                        .env("AGENT_MAX_OUTPUT_TOKENS", output.to_string());
                }
                "cli" => {
                    cmd.args(["--model", label, "--max-output-tokens", &output.to_string()]);
                }
                _ => unreachable!(),
            }
        }
        fs::write(root.join("config/agent/config.toml"), global + MOCK).unwrap();
        fs::write(root.join(".agent/config.toml"), project).unwrap();
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
        let result = fx.run(fx.agent().args([
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

#[test]
fn params_merge_by_key_and_cli_values_preserve_json_types() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    let global = root.join("config/agent/config.toml");
    let original = fs::read_to_string(&global).unwrap();
    fs::write(
        global,
        original + "\n[providers.fixture.params]\nglobal=1\nall='g'\ntree={old=1}\n",
    )
    .unwrap();
    fs::create_dir(root.join(".agent")).unwrap();
    fs::write(
        root.join(".agent/config.toml"),
        "[providers.fixture.params]\nproject=2\nall='p'\ntree={new=2}\n",
    )
    .unwrap();
    fs::write(
        root.join("case.agent"),
        "---\nparams: {front: 3, all: f}\n---\nbody",
    )
    .unwrap();
    let result = fx.run(fx.agent().args([
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
        let mut cmd = fx.agent();
        if let Some(path) = env_path {
            cmd.env("AGENT_CONTEXT", path);
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

#[test]
fn project_search_stops_at_nearest_configuration_git_or_home_boundary() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    let outer = root.join("outer");
    let project = outer.join("project");
    let nested = project.join("src/nested");
    fs::create_dir_all(&nested).unwrap();
    fs::create_dir(outer.join(".agent")).unwrap();
    fs::write(
        outer.join(".agent/config.toml"),
        "[defaults]\nmodel='outer'",
    )
    .unwrap();
    fs::write(nested.join("case.agent"), "body").unwrap();
    let run = |expected| {
        expect_code(
            &fx.run(
                fx.agent()
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
    fs::create_dir(project.join(".agent")).unwrap();
    fs::write(
        project.join(".agent/config.toml"),
        "[defaults]\nmodel='project'",
    )
    .unwrap();
    run("project");
    fs::create_dir(nested.join(".agent")).unwrap();
    run("fixture-model"); // Even an empty nearest .agent/ stops the upward search.
    fs::remove_dir(nested.join(".agent")).unwrap();
    expect_code(
        &fx.run(
            fx.agent()
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

#[test]
fn provider_resolution_injects_model_base_url_timeout_and_env_or_fallback_key() {
    use super::executable;
    let fx = Fixture::new();
    let root = &fx.dir.0;
    let global = root.join("config/agent/config.toml");
    fs::write(
        &global,
        fs::read_to_string(&global).unwrap()
            + r#"
[providers.alternate]
bin="observe-env"
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
printf '%s\n' "$AGENT_MODEL" "$AGENT_PROVIDER_BASE_URL" "$AGENT_PROVIDER_TIMEOUT_SEC" "$AGENT_PROVIDER_API_KEY" > actual
printf '%s\n' '{"type":"text_delta","text":"ok"}' '{"type":"done","finish_reason":"stop"}'
"#,
    );
    for (value, expected) in [
        (None, "configured-placeholder"),
        (Some(""), "configured-placeholder"),
        (Some("env-placeholder"), "env-placeholder"),
    ] {
        let mut cmd = fx.agent();
        cmd.env("AGENT_PROVIDER", "alternate")
            .env("AGENT_MODEL", "env-model")
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
        let mut cmd = fx.agent();
        cmd.env("AGENT_SHELL", "/bin/sh")
            .env("AGENT_STDERR", "explicit");
        if let Some(y) = yes {
            cmd.env("AGENT_YES", y);
        }
        if !approval.is_empty() {
            cmd.env("AGENT_APPROVAL", approval);
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

#[test]
fn invalid_resolved_environment_values_exit_two_before_contacting_the_provider() {
    for (key, value) in [
        ("AGENT_PROVIDER", "not-registered"),
        ("AGENT_MAX_TURNS", "0"),
        ("AGENT_MAX_TURNS", "4294967296"),
        ("AGENT_MAX_TOKENS", "1023"),
        ("AGENT_MAX_OUTPUT_TOKENS", "4294967296"),
        ("AGENT_COMPACT_THRESHOLD", "-1"),
        ("AGENT_APPROVAL", "unknown"),
        ("AGENT_STDERR", "unknown"),
        ("AGENT_IDLE_TIMEOUT", "-1"),
    ] {
        let fx = Fixture::new();
        fs::write(fx.dir.0.join("case.agent"), "body").unwrap();
        let out = fx.run(fx.agent().env(key, value).args(["case.agent", "--", "go"]));
        assert_eq!(
            out.status.code(),
            Some(2),
            "{key}={value}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(!out.stderr.is_empty() && out.stdout.is_empty() && !fx.capture.exists());
    }
}

#[test]
fn global_and_project_config_reject_unknown_keys_before_any_inference() {
    for where_ in ["config/agent", ".agent"] {
        for text in [
            "unrecognized=true",
            "[defaults]\nunknown_limit=3",
            "[providers.fixture]\nunknown_bin='other'",
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
            // Use an existing built-in provider even if the deliberately replaced global
            // file no longer defines "fixture". Its bin is a local recorder on PATH.
            fs::copy(
                root.join("record-provider"),
                root.join("agent-provider-openai-responses"),
            )
            .unwrap();
            let out = fx.run(fx.agent().args([
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
