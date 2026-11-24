//! §§6.4 and 9.2. Each adapter invocation has a distinct input file and exit status.
use super::{Fixture, done, expect_code, jsonl, scripted_provider, success_text};
use serde_json::{Value, json};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, Instant};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

fn traced(fx: &Fixture) -> std::process::Output {
    fx.run(fx.command_for("/bin/sh").args([
        "-c",
        r#"exec "$1" -q -c history case.agent -- go 3>trace"#,
        "fixture",
        env!("CARGO_BIN_EXE_agent"),
    ]))
}

#[test]
fn retryable_failures_exhaust_three_backoffs_and_preserve_the_request_and_final_stderr() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    fs::write(root.join("case.agent"), "body").unwrap();
    let turns=(1..=4).map(|i|vec![json!({"type":"error","code":"busy","message":format!("busy-{i}"),"retryable":true})]).collect::<Vec<_>>();
    scripted_provider(&fx, &turns);
    let path = root.join("record-provider");
    fs::write(
        &path,
        fs::read_to_string(&path).unwrap() + "\nprintf 'diagnostic-%s\\n' \"$n\" >&2\nexit 1\n",
    )
    .unwrap();
    let start = Instant::now();
    let out = traced(&fx);
    expect_code(&out, 1);
    assert!(start.elapsed() >= Duration::from_secs(7));
    assert!(
        String::from_utf8(out.stderr)
            .unwrap()
            .contains("diagnostic-4")
    );
    assert_eq!(fs::read_to_string(root.join("count")).unwrap().trim(), "4");
    let trace = jsonl(&root.join("trace"));
    let lifecycle: Vec<_> = trace
        .iter()
        .filter(|v| {
            matches!(
                v["type"].as_str(),
                Some("llm_request_start" | "provider_error")
            )
        })
        .collect();
    assert_eq!(lifecycle.len(), 8);
    let request = fs::read(root.join("request-1")).unwrap();
    for (i, pair) in lifecycle.chunks_exact(2).enumerate() {
        let n = i + 1;
        assert_eq!(pair[0]["type"], "llm_request_start");
        assert_eq!(pair[1]["type"], "provider_error");
        assert_eq!(pair[1]["attempt"], n);
        assert!(
            pair[1]["provider_stderr"]
                .as_str()
                .unwrap()
                .contains(&format!("diagnostic-{n}"))
        );
        assert_eq!(
            fs::read(root.join(format!("request-{n}"))).unwrap(),
            request
        );
        if i > 0 {
            let ts =
                |e: &Value| OffsetDateTime::parse(e["ts"].as_str().unwrap(), &Rfc3339).unwrap();
            let elapsed = (ts(pair[0]) - ts(lifecycle[2 * i - 1])).whole_milliseconds();
            let expected = [1000, 2000, 4000][i - 1];
            // Trace timestamps have millisecond precision. Process scheduling
            // can delay the next call; it cannot make a legitimate retry earlier.
            assert!(
                elapsed >= expected - 1,
                "attempt {n}: {elapsed}ms for {expected}ms delay"
            );
        }
    }
    assert!(
        jsonl(&root.join("history"))
            .iter()
            .all(|r| r["role"] != "assistant")
    );
}

fn check_terminal_failure(script: &str, timeout: u64, status: i32, single_attempt: bool) {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    fs::write(root.join("case.agent"), "body").unwrap();
    let conf = root.join("config/agent/config.toml");
    fs::write(
        &conf,
        fs::read_to_string(&conf)
            .unwrap()
            .replace("timeout_sec = 15", &format!("timeout_sec = {timeout}")),
    )
    .unwrap();
    fs::write(
        root.join("record-provider"),
        format!("#!/bin/sh\nprintf unique-diagnostic >&2\n/bin/cat >/dev/null\n{script}\n"),
    )
    .unwrap();
    let out = traced(&fx);
    expect_code(&out, status);
    let diagnostic = String::from_utf8(out.stderr).unwrap();
    if status == 1 {
        assert!(diagnostic.contains("unique-diagnostic"), "{diagnostic:?}");
    } else {
        assert!(!diagnostic.is_empty());
    }
    if single_attempt {
        assert_eq!(
            jsonl(&root.join("trace"))
                .iter()
                .filter(|e| e["type"] == "llm_request_start")
                .count(),
            1
        );
    }
    assert!(
        jsonl(&root.join("history"))
            .iter()
            .all(|r| r["role"] != "assistant")
    );
}

#[test]
fn nonretryable_error_preserves_provider_stderr_without_another_attempt() {
    check_terminal_failure(
        r#"printf '%s\n' '{"type":"error","code":"bad","message":"permanent","retryable":false}'; exit 1"#,
        15,
        1,
        true,
    );
}

#[test]
fn clean_provider_eof_without_done_preserves_stderr_and_discards_partial_text() {
    check_terminal_failure(
        r#"printf '%s\n' '{"type":"text_delta","text":"partial"}'; exit 0"#,
        15,
        1,
        false,
    );
}

#[test]
fn provider_timeout_maps_to_124_without_a_retry() {
    check_terminal_failure("exec /bin/sleep 15", 3, 124, true);
}

#[test]
fn opaque_round_trips_with_the_same_provider_and_is_removed_before_a_provider_switch() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    fs::write(root.join("case.agent"), "body").unwrap();
    let config = root.join("config/agent/config.toml");
    fs::write(
        &config,
        fs::read_to_string(&config).unwrap() + "\n[providers.second]\nbin='second-provider'\n",
    )
    .unwrap();
    let second = root.join("second-provider");
    fs::write(&second,"#!/bin/sh\n/bin/cat > second-input\nprintf '%s\\n' '{\"type\":\"text_delta\",\"text\":\"ok\"}' '{\"type\":\"done\",\"finish_reason\":\"stop\"}'\n").unwrap();
    fs::set_permissions(second, fs::Permissions::from_mode(0o700)).unwrap();
    let data = json!({"nested":["fixture-state",{"sequence":7}],"empty":{}});
    scripted_provider(
        &fx,
        &[
            vec![
                json!({"type":"opaque","data":data}),
                json!({"type":"text_delta","text":"first response"}),
                done(),
            ],
            success_text("second response"),
        ],
    );
    for provider in ["fixture", "fixture", "second"] {
        expect_code(
            &fx.run(fx.agent().args([
                "--provider",
                provider,
                "-q",
                "-c",
                "history",
                "case.agent",
                "--",
                "input",
            ])),
            0,
        );
    }
    let old_message = |rows: Vec<Value>| {
        rows.into_iter()
            .find(|r| r["role"] == "assistant" && r["text"] == "first response")
            .unwrap()
    };
    assert_eq!(
        old_message(jsonl(&root.join("history")))["provider_opaque"],
        data
    );
    assert_eq!(
        old_message(jsonl(&root.join("request-2")))["provider_opaque"],
        data
    );
    let switched = jsonl(&root.join("second-input"));
    assert!(
        switched
            .iter()
            .any(|r| r["role"] == "assistant" && r["text"] == "first response")
    );
    for row in switched {
        assert!(
            row["provider_opaque"].is_null() || row["provider_opaque"] == json!({}),
            "private state remained in new provider IR: {row}"
        );
    }
}
