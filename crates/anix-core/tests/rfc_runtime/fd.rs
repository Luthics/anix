use super::fixture::Fixture;
use serde_json::Value;
use std::fs;
use std::process::Output;

fn run(fixture: &Fixture, redirections: &str, cli: Option<u8>, env: Option<u8>) -> Output {
    fs::write(fixture.dir.0.join("case.agent"), "fixture script").unwrap();
    // Limit inherited descriptors to those each case explicitly sets up in its child.
    let shell = format!("exec 3>&- 4>&- 5>&- 6>&- 7>&- 8>&- 9>&-\nexec \"$@\" {redirections}");
    let mut cmd = fixture.command_for("/bin/sh");
    cmd.args(["-c", &shell, "fd-fixture", env!("CARGO_BIN_EXE_anix"), "-q"]);
    if let Some(fd) = cli {
        cmd.arg(format!("--trace-fd={fd}"));
    }
    if let Some(fd) = env {
        cmd.env("ANIX_TRACE_FD", fd.to_string());
    }
    fixture.run(cmd.args(["case.agent", "--", "first", "argument"]))
}

fn assert_success(output: &Output) {
    assert_eq!(
        output.status.code(),
        Some(0),
        "{:?}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"ok");
    assert!(
        output.stderr.is_empty(),
        "{:?}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_trace(text: &str) {
    assert!(text.ends_with('\n'), "truncated JSONL: {text:?}");
    let events: Vec<Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let lifecycle: Vec<_> = events
        .iter()
        .filter_map(|e| match e["type"].as_str().unwrap() {
            kind @ ("run_start" | "cycle_start" | "cycle_end" | "run_end") => Some(kind),
            _ => None,
        })
        .collect();
    assert_eq!(
        lifecycle,
        ["run_start", "cycle_start", "cycle_end", "run_end"]
    );
    assert_eq!(
        events.iter().find(|e| e["type"] == "run_end").unwrap()["exit_code"],
        0
    );
}

// RFC §4.1-09 — 有效 CLI trace FD 覆盖环境设定，只有被选中的 FD 收到 JSONL。
#[test]
fn only_the_explicit_trace_sink_receives_jsonl_and_cli_precedes_environment() {
    for (cli, env, selected) in [
        (None, None, None),
        (Some(3), None, Some(3)),
        (None, Some(9), Some(9)),
        (Some(7), Some(3), Some(7)),
        (Some(3), Some(9), Some(3)),
    ] {
        let fixture = Fixture::new();
        let root = &fixture.dir.0;
        let prefix = "existing fd contents\n";
        for fd in [3, 7, 9] {
            fs::write(root.join(format!("sink{fd}")), prefix).unwrap();
        }
        assert_success(&run(&fixture, "3>>sink3 7>>sink7 9>>sink9", cli, env));
        assert!(fixture.capture.exists());
        for fd in [3, 7, 9] {
            let data = fs::read_to_string(root.join(format!("sink{fd}"))).unwrap();
            let appended = data.strip_prefix(prefix).unwrap();
            if selected == Some(fd) {
                assert_trace(appended);
            } else {
                assert!(
                    appended.is_empty(),
                    "unexpected trace on fd {fd} with {cli:?}/{env:?}"
                );
            }
        }
    }
}

// RFC §4.1-08 — 可读写的显式 Trace FD 不进入静态输入。
#[test]
fn each_read_write_trace_fd_is_excluded_from_the_ordered_initial_user_input() {
    for selected in 4..=9 {
        let fixture = Fixture::new();
        let root = &fixture.dir.0;
        fs::write(root.join("stdin"), "stdin-marker").unwrap();
        for fd in 4..=9 {
            fs::write(root.join(format!("in{fd}")), format!("payload-{fd}")).unwrap();
        }
        let redirs = "<stdin 4<>in4 5<>in5 6<>in6 7<>in7 8<>in8 9<>in9";
        assert_success(&run(&fixture, redirs, Some(selected), None));
        let request = fs::read_to_string(&fixture.capture).unwrap();
        let users: Vec<Value> = request
            .lines()
            .map(|l| serde_json::from_str::<Value>(l).unwrap())
            .filter(|line| line["role"] == "user")
            .collect();
        assert_eq!(users.len(), 1);
        let text = users[0]["text"].as_str().unwrap();
        let (args, rest) = text.split_once("stdin-marker").unwrap();
        assert_eq!(args.trim_end(), "<args>first argument</args>");
        let mut rest = rest.trim_start();
        for fd in 4..=9 {
            let data = fs::read_to_string(root.join(format!("in{fd}"))).unwrap();
            if fd == selected {
                assert_trace(&data);
                continue;
            }
            let tag = format!("<input fd=\"{fd}\">");
            let body = rest
                .strip_prefix(&tag)
                .unwrap_or_else(|| panic!("expected {tag} in {rest:?}"));
            let (body, next) = body.split_once("</input>").unwrap();
            assert_eq!(body.trim(), format!("payload-{fd}"));
            assert_eq!(data, format!("payload-{fd}"));
            rest = next.trim_start();
        }
        assert!(rest.is_empty(), "unexpected user input: {rest:?}");
    }
}

// RFC §4.1-09 — 传入不可写或未打开的 trace FD 时返回 2，Provider 未被调用。
#[test]
fn invalid_trace_fd_reports_usage_to_stderr_before_invoking_the_provider() {
    for fd in [0, 1, 2, 7, 8] {
        for (cli, env) in [(Some(fd), Some(3)), (None, Some(fd))] {
            let fixture = Fixture::new();
            let root = &fixture.dir.0;
            fs::write(root.join("sink3"), "unused").unwrap();
            fs::write(root.join("input8"), "read only").unwrap();
            let out = run(&fixture, "3>>sink3 8<input8", cli, env);
            assert_eq!(
                out.status.code(),
                Some(2),
                "cli={cli:?}, env={env:?}, stderr={:?}",
                String::from_utf8_lossy(&out.stderr)
            );
            assert!(out.stdout.is_empty());
            assert!(!out.stderr.is_empty() && out.stderr.ends_with(b"\n"));
            assert!(!fixture.capture.exists());
            assert_eq!(fs::read(root.join("sink3")).unwrap(), b"unused");
            assert_eq!(fs::read(root.join("input8")).unwrap(), b"read only");
        }
    }
}
