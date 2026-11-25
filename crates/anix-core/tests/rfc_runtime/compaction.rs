//! RFC §§6.1–6.2 and 9.2–9.5. Use independently chosen totals and component
//! counts to detect double counting across tools, compaction and resumed processes.
use super::{Fixture, done, expect_code, jsonl, scripted_provider, success_text, tool, with_tools};
use serde_json::{Value, json};
use std::fs;
use std::os::unix::fs::PermissionsExt;

fn counts(input: u64, output: u64, cached: u64, write: u64, reasoning: u64) -> Value {
    json!({"input_tokens":input,"output_tokens":output,"cached_tokens":cached,"cache_write_tokens":write,"reasoning_tokens":reasoning})
}
fn with_usage(mut events: Vec<Value>, mut usage: Value) -> Vec<Value> {
    usage["type"] = json!("usage");
    events.insert(0, usage);
    events
}
fn hook(fx: &Fixture) {
    let dir = fx.dir.0.join("config/anix/hooks/on-exit.d");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("report");
    fs::write(&path, "#!/bin/sh\n/bin/cat > exit.json\n").unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}

fn exit_report(fx: &Fixture) -> Value {
    serde_json::from_slice(&fs::read(fx.dir.0.join("exit.json")).unwrap()).unwrap()
}

// RFC v1.1 §6.1, §6.2, §9.2, §9.3, §9.5
#[test]
fn each_subset_of_optional_usage_details_preserves_inclusive_totals_through_exit() {
    // Odd counts above the exact-integer range of binary64 and above u32.
    // Keep input + output representable so the budget and all five counters
    // have unambiguous expected values.
    let input = 9_007_199_254_740_993;
    let output = 4_294_967_297;
    let details: [(&str, u64); 3] = [
        ("cached_tokens", 4_294_967_315),
        ("cache_write_tokens", 13),
        ("reasoning_tokens", 19),
    ];
    for mask in 0..8 {
        let fx = Fixture::new();
        let root = &fx.dir.0;
        hook(&fx);
        fs::write(root.join("case.agent"), "usage fixture").unwrap();
        let mut event = json!({"type":"usage","input_tokens":input,"output_tokens":output});
        let mut expected = counts(input, output, 0, 0, 0);
        for (bit, (field, n)) in details.iter().copied().enumerate() {
            if mask & (1 << bit) != 0 {
                event[field] = json!(n);
                expected[field] = json!(n);
            }
        }
        scripted_provider(
            &fx,
            &[vec![
                json!({"type":"text_delta","text":"measured"}),
                event,
                done(),
            ]],
        );
        let out = fx.run(fx.command_for("/bin/sh").args([
            "-c",
            r#"exec "$1" --trace-fd=3 -q -c history case.agent -- go 3>trace"#,
            "fixture",
            env!("CARGO_BIN_EXE_anix"),
        ]));
        expect_code(&out, 0);
        let wal = jsonl(&root.join("history"));
        let assistant = wal.last().unwrap();
        assert_eq!(assistant["role"], "assistant");
        let saved = &assistant["usage"];
        assert_eq!(saved["input_tokens"], input);
        assert_eq!(saved["output_tokens"], output);
        for (field, _) in details {
            // §9.2 permits omission of an optional zero detail in the WAL.
            let stored = saved.get(field).map(|v| v.as_u64().unwrap()).unwrap_or(0);
            assert_eq!(
                stored,
                expected[field].as_u64().unwrap(),
                "WAL mask={mask}, {field}"
            );
        }
        let trace = jsonl(&root.join("trace"));
        let response = trace.iter().find(|r| r["type"] == "llm_response").unwrap();
        assert_eq!(response["usage"], expected, "llm_response mask={mask}");
        let end = trace.iter().find(|r| r["type"] == "run_end").unwrap();
        let hook = exit_report(&fx);
        for report in [end, &hook] {
            assert_eq!(report["usage"], expected, "mask={mask}");
            assert_eq!(report["session_usage"], expected, "mask={mask}");
            assert_eq!(report["context"]["tokens"], input + output, "mask={mask}");
        }
    }
}

// RFC v1.1 §6.1, §6.2, §9.2, §9.3
#[test]
fn a_reported_zero_watermark_and_an_unreported_usage_use_different_budget_baselines() {
    for reported in [false, true] {
        let fx = Fixture::new();
        let root = &fx.dir.0;
        fs::write(root.join("case.agent"), "frozen prefix").unwrap();
        hook(&fx);
        let mut reply = success_text("finished");
        if reported {
            reply.insert(
                0,
                json!({"type":"usage","input_tokens":0,"output_tokens":0}),
            );
        }
        scripted_provider(&fx, &[reply]);
        expect_code(
            &fx.run(
                fx.anix()
                    .args(["-q", "-c", "history", "case.agent", "--", "hello"]),
            ),
            0,
        );
        let exit = exit_report(&fx);
        assert_eq!(exit["usage"], counts(0, 0, 0, 0, 0));
        assert_eq!(exit["session_usage"], exit["usage"]);
        let watermark = exit["context"]["tokens"].as_u64().unwrap();
        let wal = jsonl(&root.join("history"));
        let last = wal.last().unwrap();
        assert_eq!(last["role"], "assistant");
        if reported {
            assert_eq!(last["usage"]["input_tokens"], 0);
            assert_eq!(last["usage"]["output_tokens"], 0);
            assert_eq!(watermark, 0);
        } else {
            assert!(watermark > 0, "the nonempty window must be estimated");
        }
    }
}

// RFC v1.1 §9.2, §6.1, §9.3, §9.5
#[test]
fn retry_usage_counts_every_charged_attempt_without_inflating_the_final_watermark() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    fs::write(root.join("case.agent"), "body").unwrap();
    hook(&fx);
    let mut partial = counts(17, 7, 9, 3, 5);
    partial["type"] = json!("usage");
    let failure = vec![
        partial,
        json!({"type":"error","code":"temporary","message":"retry fixture","retryable":true}),
    ];
    let success = with_usage(success_text("finished"), counts(23, 11, 13, 2, 4));
    scripted_provider(&fx, &[failure, success]);
    let provider = root.join("record-provider");
    fs::write(
        &provider,
        fs::read_to_string(&provider).unwrap() + "\n[ \"$n\" -gt 1 ]\n",
    )
    .unwrap();
    let cmd = r#"exec "$1" --trace-fd=3 -q -c history case.agent -- go 3>trace"#;
    let output =
        fx.run(
            fx.command_for("/bin/sh")
                .args(["-c", cmd, "fixture", env!("CARGO_BIN_EXE_anix")]),
        );
    expect_code(&output, 0);
    assert_eq!(
        fs::read(root.join("request-1")).unwrap(),
        fs::read(root.join("request-2")).unwrap()
    );
    let trace = jsonl(&root.join("trace"));
    assert_eq!(
        trace
            .iter()
            .filter(|r| r["type"] == "provider_error")
            .count(),
        1
    );
    let exit = exit_report(&fx);
    let end = trace.iter().find(|r| r["type"] == "run_end").unwrap();
    for row in [end, &exit] {
        assert_eq!(
            row["context"]["tokens"], 34,
            "last successful model input + output"
        );
        assert_eq!(
            row["usage"],
            counts(40, 18, 22, 5, 9),
            "all billed attempts"
        );
        assert_eq!(
            row["session_usage"], row["usage"],
            "fresh persistent session"
        );
    }
}

// RFC v1.1 §6.1, §6.2, §9.2, §9.3, §9.5
#[test]
fn full_compaction_and_resume_account_for_every_call_without_inflating_watermark() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    hook(&fx);
    fs::write(root.join("case.agent"), "frozen prompt").unwrap();
    let first = counts(50, 20, 10, 5, 7);
    let summarize = counts(1230, 6000, 111, 22, 5000);
    let third = json!({"input_tokens":7,"output_tokens":3});
    let fourth = counts(10, 4, 1, 2, 3);
    scripted_provider(
        &fx,
        &[
            with_usage(
                with_tools([tool(
                    "c",
                    r#"printf '%s\n' '{"op":"compact"}' > "$ANIX_CTL""#,
                )]),
                first.clone(),
            ),
            with_usage(success_text("summary text"), summarize.clone()),
            with_usage(success_text("first done"), third),
            with_usage(success_text("resume done"), fourth.clone()),
        ],
    );
    let run = |quiet: bool| {
        let mut cmd = fx.command_for("/bin/sh");
        let mode = if quiet { "explicit" } else { "auto" };
        cmd.args([
            "-c",
            r#"exec "$@" 3>trace"#,
            "fixture",
            env!("CARGO_BIN_EXE_anix"),
            "--trace-fd=3",
            "-y",
            "--no-compact",
            "--max-tokens",
            "1024",
            "--shell",
            "/bin/sh",
            "-c",
            "history",
            "--stderr",
            mode,
            "case.agent",
            "--",
            "input",
        ]);
        let out = fx.run(&mut cmd);
        expect_code(&out, 0);
        String::from_utf8(out.stderr).unwrap()
    };
    let stderr = run(false);
    let compact_events = || {
        jsonl(&root.join("trace"))
            .iter()
            .filter(|r| r["type"] == "compact")
            .count()
    };
    assert_eq!(compact_events(), 1);
    let first_total = counts(1287, 6023, 121, 27, 5007);
    let wal = jsonl(&root.join("history"));
    let checkpoint = wal.iter().find(|r| r["type"] == "checkpoint").unwrap();
    assert_eq!(checkpoint["compact_usage"], summarize);
    assert_eq!(
        checkpoint["cumulative_usage"],
        counts(1280, 6020, 121, 27, 5007)
    );
    let assistant: Vec<_> = wal.iter().filter(|r| r["role"] == "assistant").collect();
    assert_eq!(assistant.len(), 2);
    assert_eq!(assistant[0]["usage"], first);
    assert_eq!(assistant[1]["usage"]["input_tokens"], 7);
    let summary_ir = jsonl(&root.join("request-2"));
    assert_eq!(summary_ir[0]["tools"], json!([]));
    let fresh = jsonl(&root.join("request-3"));
    assert_eq!(fresh.len(), 3);
    assert_eq!(fresh[1]["role"], "system");
    assert_eq!(
        fresh[2]["text"],
        "<checkpoint>\nsummary text\n</checkpoint>"
    );
    for i in 1..=3 {
        for r in jsonl(&root.join(format!("request-{i}"))) {
            for key in [
                "usage",
                "session_usage",
                "cumulative_usage",
                "compact_usage",
            ] {
                assert!(r.get(key).is_none(), "{r}");
            }
        }
    }
    let totals = |run: &Value, session: &Value, watermark: u64| {
        let trace = jsonl(&root.join("trace"));
        let hook = &exit_report(&fx);
        let end = trace.iter().find(|r| r["type"] == "run_end").unwrap();
        for row in [hook, end] {
            assert_eq!(&row["usage"], run);
            assert_eq!(&row["session_usage"], session);
            assert_eq!(
                row["context"],
                json!({"tokens":watermark,"max_tokens":1024})
            );
        }
    };
    totals(&first_total, &first_total, 10);
    let summaries: Vec<_> = stderr
        .lines()
        .filter(|l| l.contains("ctx 10/1024"))
        .collect();
    assert_eq!(summaries.len(), 1);
    assert!(summaries[0].contains("run") && summaries[0].contains("total"));
    let quiet = run(true);
    assert_eq!(compact_events(), 0);
    assert!(quiet.is_empty(), "{quiet}");
    totals(&fourth, &counts(1297, 6027, 122, 29, 5010), 14);
    let restored = jsonl(&root.join("request-4"));
    assert!(restored.iter().any(|r| r["text"] == "first done"));
    assert!(!restored.iter().any(|r| r.get("usage").is_some()));
}

// RFC v1.1 §6.1, §6.2, §9.1
#[test]
fn tail_fold_aligns_to_assistant_and_removes_the_triggering_tool_call() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    fs::write(root.join("case.agent"), "frozen").unwrap();
    scripted_provider(
        &fx,
        &[
            with_tools([tool("a", "printf alpha"), tool("b", "printf beta")]),
            with_tools([tool(
                "fold",
                r#"printf '%s\n' '{"op":"fold","from_turn":4,"summary":"recap"}' > "$ANIX_CTL""#,
            )]),
            success_text("finished"),
            success_text("resumed"),
        ],
    );
    let args = [
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
    expect_code(&fx.run(fx.anix().args(args)), 0);
    let first = jsonl(&root.join("request-1"));
    let folded = jsonl(&root.join("request-3"));
    assert_eq!(folded.len(), 4);
    assert_eq!(first[1], folded[1]);
    assert_eq!(first[2], folded[2]);
    let record = jsonl(&root.join("history"))
        .into_iter()
        .find(|r| r["type"] == "fold")
        .unwrap();
    assert_eq!(record["from_turn"], 2);
    let to = record["to_turn"].as_u64().unwrap();
    assert!(to >= 5);
    assert_eq!(folded[3]["role"], "user");
    assert_eq!(
        folded[3]["text"],
        format!("<folded turns=\"2..{to}\">recap</folded>")
    );
    expect_code(&fx.run(fx.anix().args(args)), 0);
    let restored = jsonl(&root.join("request-4"));
    assert_eq!(restored[1..4], folded[1..4]);
    assert!(restored[4]["turn_id"].as_u64().unwrap() > to);
}

// RFC v1.1 §6.1, §9.1
#[test]
fn external_compactor_receives_tool_free_ir_and_empty_or_failed_summary_is_fatal() {
    for (script, code) in [
        ("printf summary", 0),
        (r"printf 'valid prefix\377tail'", 1),
        (":", 1),
        ("printf discarded; exit 9", 1),
    ] {
        let fx = Fixture::new();
        let root = &fx.dir.0;
        fs::write(root.join("case.agent"), "prompt").unwrap();
        let conf = root.join("config/anix/config.toml");
        let original = fs::read_to_string(&conf).unwrap();
        fs::write(&conf, original + "\n[compact]\nbin='fixture-compact'\n").unwrap();
        let compact = root.join("fixture-compact");
        fs::write(
            &compact,
            format!("#!/bin/sh\n/bin/cat > compact.ir\n{script}\n"),
        )
        .unwrap();
        fs::set_permissions(compact, fs::Permissions::from_mode(0o700)).unwrap();
        scripted_provider(
            &fx,
            &[
                with_tools([tool(
                    "c",
                    r#"printf '%s\n' '{"op":"compact"}' > "$ANIX_CTL""#,
                )]),
                vec![json!({"type":"text_delta","text":"done"}), done()],
            ],
        );
        let out = fx.run(fx.anix().args([
            "-q",
            "-y",
            "--shell",
            "/bin/sh",
            "-c",
            "history",
            "case.agent",
            "--",
            "go",
        ]));
        expect_code(&out, code);
        assert_eq!(jsonl(&root.join("compact.ir"))[0]["tools"], json!([]));
        let wal = jsonl(&root.join("history"));
        assert_eq!(wal.iter().any(|r| r["type"] == "checkpoint"), code == 0);
        assert_eq!(root.join("request-2").exists(), code == 0);
    }
}

/// Measure the first request with a deliberately failing provider so no assistant
/// reply changes the window before the on-exit observation.
fn initial_watermark(input: &str) -> u64 {
    let fx = Fixture::new();
    hook(&fx);
    fs::write(fx.dir.0.join("case.agent"), "body").unwrap();
    fs::write(
        fx.dir.0.join("record-provider"),
        "#!/bin/sh\n/bin/cat >/dev/null\nexit 1\n",
    )
    .unwrap();
    expect_code(
        &fx.run(fx.anix().args(["--no-compact", "case.agent", "--", input])),
        1,
    );
    exit_report(&fx)["context"]["tokens"].as_u64().unwrap()
}

// RFC v1.1 §6.1
#[test]
fn fresh_window_counts_utf8_bytes_in_groups_of_three() {
    let readings: Vec<_> = (0..7)
        .map(|n| initial_watermark(&format!("x{}", "a".repeat(n))))
        .collect();
    for pair in readings.windows(2) {
        assert!(pair[1] == pair[0] || pair[1] == pair[0] + 1, "{readings:?}");
    }
    for i in 0..4 {
        assert_eq!(readings[i + 3], readings[i] + 1, "{readings:?}");
    }
    assert_eq!(initial_watermark("x水"), readings[3]);
    assert_eq!(initial_watermark("x🦀"), readings[4]);
}

// RFC v1.1 §6.1, §6.5
#[test]
fn hard_limit_allows_equality_and_blocks_the_first_provider_request_above_it() {
    let input = "z".repeat(3600);
    let watermark = initial_watermark(&input);
    assert!(watermark > 1024);
    for (limit, status) in [(watermark - 1, 124), (watermark, 0), (watermark + 1, 0)] {
        let fx = Fixture::new();
        fs::write(fx.dir.0.join("case.agent"), "body").unwrap();
        let out = fx.run(fx.anix().args([
            "-q",
            "--no-compact",
            "--max-tokens",
            &limit.to_string(),
            "case.agent",
            "--",
            &input,
        ]));
        expect_code(&out, status);
        assert_eq!(
            fx.capture.is_file(),
            status == 0,
            "watermark={watermark}, limit={limit}"
        );
        if status == 124 {
            assert!(!out.stderr.is_empty());
        }
    }
}

// RFC v1.1 §6.1
#[test]
fn auto_compaction_runs_before_inference_and_rechecks_the_new_window_against_the_cap() {
    let input = "a".repeat(3600);
    let watermark = initial_watermark(&input);
    for (threshold, summary, compacts, status) in [
        (watermark + 1, "small".to_owned(), false, 0),
        (watermark - 1, "small".to_owned(), true, 0),
        (watermark - 1, "s".repeat(9000), true, 124),
    ] {
        let fx = Fixture::new();
        let root = &fx.dir.0;
        fs::write(root.join("case.agent"), "body").unwrap();
        fs::write(root.join("summary"), summary).unwrap();
        let compact = root.join("compress");
        fs::write(
            &compact,
            "#!/bin/sh\n/bin/cat > compact.ir\n/bin/cat summary\n",
        )
        .unwrap();
        fs::set_permissions(&compact, fs::Permissions::from_mode(0o700)).unwrap();
        let config = root.join("config/anix/config.toml");
        fs::write(
            &config,
            fs::read_to_string(&config).unwrap() + "\n[compact]\nbin='compress'\n",
        )
        .unwrap();
        let out = fx.run(fx.anix().args([
            "-q",
            "--max-tokens",
            "2048",
            "--compact-threshold",
            &threshold.to_string(),
            "case.agent",
            "--",
            &input,
        ]));
        expect_code(&out, status);
        assert_eq!(root.join("compact.ir").exists(), compacts);
        assert_eq!(fx.capture.exists(), status == 0);
        if compacts {
            let original = jsonl(&root.join("compact.ir"));
            assert_eq!(original[0]["tools"], json!([]));
            assert!(
                original.last().unwrap()["text"]
                    .as_str()
                    .unwrap()
                    .contains(&input)
            );
            if status == 0 {
                assert_eq!(
                    jsonl(&fx.capture)[2]["text"],
                    "<checkpoint>\nsmall\n</checkpoint>"
                );
            }
        }
    }
}

// RFC v1.1 §6.1, §8.2
#[test]
fn builtin_compactor_uses_configured_provider_model_and_literal_or_file_prompt() {
    for from_file in [false, true] {
        let fx = Fixture::new();
        let root = &fx.dir.0;
        fs::write(root.join("case.agent"), "regular prompt").unwrap();
        let conf = root.join("config/anix/config.toml");
        let prompt = "summarize only the fixture";
        let setting = if from_file {
            fs::write(root.join("custom.txt"), prompt).unwrap();
            "prompt_file='custom.txt'".to_owned()
        } else {
            format!("prompt='{prompt}'")
        };
        let original = fs::read_to_string(&conf).unwrap();
        let config = format!(
            "{original}\n[providers.other]\nbin='summarizer'\nmode='oneshot'\n[compact]\nprovider='other'\nmodel='other-model'\n{setting}\n"
        );
        fs::write(&conf, config).unwrap();
        super::executable(
            &root.join("summarizer"),
            r#"#!/bin/sh
set -eu
/bin/cat > summary-request
printf %s "$ANIX_MODEL" > summary-model
printf '%s\n' '{"type":"text_delta","text":"local summary"}' '{"type":"done","finish_reason":"stop"}'
"#,
        );
        scripted_provider(
            &fx,
            &[
                with_tools([tool(
                    "c",
                    r#"printf '%s\n' '{"op":"compact"}' > "$ANIX_CTL""#,
                )]),
                success_text("normal reply"),
            ],
        );
        expect_code(
            &fx.run(
                fx.anix()
                    .args(["-y", "--shell", "/bin/sh", "case.agent", "--", "go"]),
            ),
            0,
        );
        let ir = jsonl(&root.join("summary-request"));
        assert_eq!(ir[0]["tools"], json!([]));
        assert!(ir.iter().any(|r| r["text"] == prompt));
        assert_eq!(
            fs::read_to_string(root.join("summary-model")).unwrap(),
            "other-model"
        );
        let normal = jsonl(&root.join("request-2"));
        assert!(!normal[0]["tools"].as_array().unwrap().is_empty());
        assert_eq!(
            normal[2]["text"],
            "<checkpoint>\nlocal summary\n</checkpoint>"
        );
    }
}

// RFC v1.1 §6.1, §6.2, §9.1, §9.2, §9.3
#[test]
fn two_checkpoints_carry_folded_usage_once_and_refresh_the_prefix_across_processes() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    hook(&fx);
    let fold =
        r#"printf '%s\n' '{"op":"fold","from_turn":1,"summary":"early fold"}' > "$ANIX_CTL""#;
    let compact = r#"printf '%s\n' '{"op":"compact"}' > "$ANIX_CTL""#;
    scripted_provider(
        &fx,
        &[
            with_usage(with_tools([tool("a", fold)]), counts(10, 3, 2, 1, 1)),
            with_usage(with_tools([tool("b", compact)]), counts(13, 5, 3, 2, 2)),
            with_usage(success_text("summary one"), counts(17, 7, 4, 2, 3)),
            with_usage(success_text("first final"), counts(19, 11, 5, 3, 4)),
            with_usage(with_tools([tool("e", compact)]), counts(23, 13, 7, 4, 5)),
            with_usage(success_text("summary two"), counts(29, 17, 8, 4, 6)),
            with_usage(success_text("second final"), counts(31, 19, 9, 5, 7)),
            with_usage(success_text("resumed final"), counts(37, 23, 11, 5, 8)),
        ],
    );
    let run = |prompt: &str, mode: &str, input: &str, usage: Value, session: Value| {
        fs::write(root.join("case.agent"), prompt).unwrap();
        expect_code(
            &fx.run(fx.anix().args([
                "-c",
                "history",
                "-y",
                "--no-compact",
                "--shell",
                "/bin/sh",
                "--stderr",
                mode,
                "case.agent",
                "--",
                input,
            ])),
            0,
        );
        let exit = &exit_report(&fx);
        assert_eq!(exit["usage"], usage, "{input}");
        assert_eq!(exit["session_usage"], session, "{input}");
    };
    run(
        "first prefix",
        "explicit",
        "one",
        counts(59, 26, 14, 8, 10),
        counts(59, 26, 14, 8, 10),
    );
    let first_bytes = fs::read(root.join("history")).unwrap();
    run(
        "second prefix",
        "auto",
        "two",
        counts(83, 49, 24, 13, 18),
        counts(142, 75, 38, 21, 28),
    );
    let second_bytes = fs::read(root.join("history")).unwrap();
    assert!(second_bytes.starts_with(&first_bytes));
    run(
        "unused third prefix",
        "auto",
        "three",
        counts(37, 23, 11, 5, 8),
        counts(179, 98, 49, 26, 36),
    );
    assert!(
        fs::read(root.join("history"))
            .unwrap()
            .starts_with(&second_bytes)
    );

    let wal = jsonl(&root.join("history"));
    let sections: Vec<_> = wal
        .iter()
        .enumerate()
        .filter(|(_, r)| r["type"] == "checkpoint")
        .collect();
    assert_eq!(sections.len(), 2);
    assert_eq!(wal.iter().filter(|r| r["type"] == "fold").count(), 1);
    for ((index, cp), (mode, prefix, summary, usage, cumulative)) in sections.into_iter().zip([
        (
            "explicit",
            "first prefix",
            "summary one",
            counts(17, 7, 4, 2, 3),
            counts(40, 15, 9, 5, 6),
        ),
        (
            "auto",
            "second prefix",
            "summary two",
            counts(29, 17, 8, 4, 6),
            counts(111, 56, 29, 16, 21),
        ),
    ]) {
        assert_eq!(cp["ir_version"], 1);
        assert_eq!(cp["compact_usage"], usage);
        assert_eq!(cp["cumulative_usage"], cumulative);
        assert_eq!(cp["config"]["stderr"], mode);
        assert_eq!(cp["config"]["approval"], "auto");
        assert_eq!(wal[index + 1]["role"], "system");
        assert_eq!(wal[index + 1]["turn_id"], cp["turn_id"]);
        assert_eq!(wal[index + 1]["text"], prefix);
        assert_eq!(wal[index + 2]["role"], "user");
        assert_eq!(
            wal[index + 2]["text"],
            format!("<checkpoint>\n{summary}\n</checkpoint>")
        );
    }
    // Inference resumes with the new section only. The changed script takes
    // effect at the second checkpoint, not when opening the WAL afterwards.
    let ir = jsonl(&root.join("request-8"));
    let text: Vec<_> = ir[1..]
        .iter()
        .map(|r| r["text"].as_str().unwrap())
        .collect();
    assert_eq!(
        text,
        [
            "second prefix",
            "<checkpoint>\nsummary two\n</checkpoint>",
            "second final",
            "<args>three</args>"
        ]
    );
}

fn run_tail_turn(fx: &Fixture, input: &str) {
    let out = fx.run(fx.anix().args([
        "--output",
        "none",
        "-q",
        "-y",
        "--shell",
        "/bin/sh",
        "-c",
        "history",
        "case.agent",
        "--",
        input,
    ]));
    expect_code(&out, 0);
    assert!(out.stdout.is_empty() && out.stderr.is_empty());
}

struct TailFold {
    fx: Fixture,
    before: Vec<Value>,
    prefix: Vec<Value>,
    from: u64,
    to: u64,
}

fn request_auto_fold(plugin_response: &str) -> TailFold {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    fs::write(root.join("case.agent"), "frozen prefix marker").unwrap();
    let cfg = root.join("config/anix/config.toml");
    let settings = fs::read_to_string(&cfg).unwrap();
    fs::write(cfg, settings + "\n[compact]\nbin='local-summary'\n").unwrap();
    super::executable(
        &root.join("local-summary"),
        &format!(
            "#!/bin/sh\nset -eu\n/bin/cat > summary-input\nprintf 'called\\n' >> summaries\n{plugin_response}\n"
        ),
    );
    scripted_provider(
        &fx,
        &[
            success_text("prior committed reply"),
            with_tools([tool("left", "printf left"), tool("right", "printf right")]),
            success_text("after both tools"),
            with_tools([tool(
                "fold",
                r#"/bin/cat fold-frame > "$ANIX_CTL"; printf fold-note"#,
            )]),
            success_text("after fold attempt"),
            success_text("restored"),
        ],
    );
    run_tail_turn(&fx, "prior committed input");
    run_tail_turn(&fx, "tool batch input");
    // Choose the second tool's real turn ID so the compactor must include
    // the whole earlier assistant batch, including its first tool result.
    let wal = jsonl(&root.join("history"));
    let selected = wal.iter().find(|r| r["tool_call_id"] == "right").unwrap()["turn_id"]
        .as_u64()
        .unwrap();
    let from = wal
        .iter()
        .find(|r| r["tool_calls"][0]["id"] == "left")
        .unwrap()["turn_id"]
        .as_u64()
        .unwrap();
    fs::write(
        root.join("fold-frame"),
        format!("{}\n", json!({"op":"fold","from_turn":selected})),
    )
    .unwrap();
    let saved = fs::read(root.join("history")).unwrap();
    run_tail_turn(&fx, "fold the tail");
    assert!(fs::read(root.join("history")).unwrap().starts_with(&saved));
    let before = jsonl(&root.join("request-4"));
    let prefix: Vec<_> = before
        .iter()
        .skip(1)
        .take_while(|r| r["turn_id"].as_u64().unwrap() < from)
        .cloned()
        .collect();

    let request = jsonl(&root.join("summary-input"));
    assert_eq!(request[0]["type"], "header");
    assert_eq!(request[0]["ir_version"], 1);
    assert_eq!(request[0]["tools"], json!([]));
    let raw = fs::read_to_string(root.join("summary-input")).unwrap();
    for row in &prefix {
        let text = row["text"].as_str().unwrap();
        assert!(
            !raw.contains(text),
            "prefix text in compactor input: {text}"
        );
    }
    let old_tail = &before[prefix.len() + 1..];
    let positions: Vec<_> = old_tail
        .iter()
        .map(|row| {
            request
                .iter()
                .position(|r| r == row)
                .unwrap_or_else(|| panic!("missing summary input: {row}"))
        })
        .collect();
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
    let triggering_assistant = request
        .iter()
        .position(|r| r["tool_calls"][0]["id"] == "fold")
        .unwrap();
    let triggering_tool = &request[triggering_assistant + 1];
    assert_eq!(triggering_tool["role"], "tool");
    assert_eq!(triggering_tool["tool_call_id"], "fold");
    assert!(
        triggering_tool["text"]
            .as_str()
            .unwrap()
            .contains("fold-note")
    );
    let to = triggering_tool["turn_id"].as_u64().unwrap();
    assert!(to > selected && selected > from);
    run_tail_turn(&fx, "resume");
    assert_eq!(fs::read(root.join("summaries")).unwrap(), b"called\n");
    TailFold {
        fx,
        before,
        prefix,
        from,
        to,
    }
}

// RFC v1.1 §6.1, §6.2, §7.2, §9.1
#[test]
fn external_auto_fold_receives_the_aligned_tail_and_replays_only_its_summary() {
    let case = request_auto_fold("printf 'two tools · 已归档'");
    let root = &case.fx.dir.0;
    let folds: Vec<_> = jsonl(&root.join("history"))
        .into_iter()
        .filter(|r| r["type"] == "fold")
        .collect();
    assert_eq!(folds.len(), 1);
    assert_eq!(folds[0]["from_turn"], case.from);
    assert_eq!(folds[0]["to_turn"], case.to);
    assert_eq!(folds[0]["summary"], "two tools · 已归档");
    let marker = format!(
        "<folded turns=\"{}..{}\">two tools · 已归档</folded>",
        case.from, case.to
    );
    for file in ["request-5", "request-6"] {
        let ir = jsonl(&root.join(file));
        assert_eq!(&ir[1..1 + case.prefix.len()], case.prefix);
        let next = &ir[1 + case.prefix.len()];
        assert_eq!(next["role"], "user");
        assert_eq!(next["text"], marker);
        assert!(
            !ir.iter()
                .any(|r| r["role"] == "tool" || r.get("tool_calls").is_some())
        );
    }
}

// RFC v1.1 §6.1, §6.2, §7.2, §9.1
#[test]
fn empty_or_failed_auto_summary_keeps_the_subwindow_and_reports_a_tool_error() {
    for response in [":", "printf discarded; exit 7"] {
        let case = request_auto_fold(response);
        let root = &case.fx.dir.0;
        let wal = jsonl(&root.join("history"));
        assert!(!wal.iter().any(|r| r["type"] == "fold"));
        for file in ["request-5", "request-6"] {
            let ir = jsonl(&root.join(file));
            assert_eq!(&ir[1..case.before.len()], &case.before[1..]);
            let last_tool = ir.iter().find(|r| r["tool_call_id"] == "fold").unwrap();
            assert_eq!(last_tool["turn_id"], case.to);
            assert_eq!(last_tool["is_error"], true);
            assert!(last_tool["text"].as_str().unwrap().contains("fold-note"));
        }
        let saved = wal.iter().find(|r| r["tool_call_id"] == "fold").unwrap();
        assert_eq!(saved["is_error"], true);
    }
}
