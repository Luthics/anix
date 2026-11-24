//! §3.2 rules 1, 3 and 4 through the shipping CLI. A local executable records
//! exactly the IR seen by the provider. Its capture file also witnesses invocation.

use super::fixture::Fixture;
use serde_json::{Value, json};
use std::fs;

fn run_script(fixture: &Fixture, script: &[u8]) -> std::process::Output {
    fs::write(fixture.dir.0.join("case.agent"), script).unwrap();
    fixture.run(
        fixture
            .agent()
            .args(["--stderr", "explicit", "case.agent", "--", "user input"]),
    )
}

impl Fixture {
    fn assert_rejected(&self, label: &str, source: &[u8]) {
        let result = run_script(self, source);
        let (code, stdout, stderr) = (result.status.code(), result.stdout, result.stderr);
        assert_eq!(
            code,
            Some(2),
            "{label}: {}",
            String::from_utf8_lossy(&stderr)
        );
        assert!(
            stdout.is_empty(),
            "{label}: diagnostic on stdout: {stdout:?}"
        );
        assert!(
            stderr.iter().any(|b| !b.is_ascii_whitespace()),
            "{label}: missing diagnostic"
        );
        assert!(
            !self.capture.try_exists().unwrap(),
            "{label}: invalid script reached provider"
        );
    }
}

#[test]
fn crlf_script_reaches_the_provider_as_the_intended_system_message() {
    let fixture = Fixture::new();
    let source = "#!fixture --not-the-prompt\r\n---\r\nx-extra: [one, two]\r\nparams: {sample: 7}\r\n---\r\n\r\n  执行任务  \r\n\r\n第二行\r\n\r\n";
    let result = run_script(&fixture, source.as_bytes());
    let (code, stdout, stderr) = (result.status.code(), result.stdout, result.stderr);
    assert_eq!(code, Some(0), "{}", String::from_utf8_lossy(&stderr));
    assert!(stdout.is_empty());
    let ir = fs::read_to_string(fixture.capture).unwrap();
    let rows: Vec<Value> = ir
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    assert_eq!(rows[0]["params"]["sample"], json!(7));
    assert_eq!(rows[1]["role"], "system");
    let prompt = rows[1]["text"].as_str().unwrap();
    assert_eq!(
        prompt.lines().collect::<Vec<_>>(),
        ["  执行任务  ", "", "第二行"]
    );
}

#[test]
fn unknown_top_level_fields_fail_closed_regardless_of_spelling_or_position() {
    let fixture = Fixture::new();
    for key in [
        "unknown",
        "nAme",
        "max-turns",
        "x_private",
        "X-vendor",
        "extra.x-key",
    ] {
        for fields in [
            format!("{key}: wrong\nname: ok\nx-vendor: ok"),
            format!("name: ok\n{key}: wrong\nx-vendor: ok"),
            format!("name: ok\nx-vendor: ok\n{key}: wrong"),
        ] {
            let script = format!("---\n{fields}\n---\nbody");
            fixture.assert_rejected(&fields, script.as_bytes());
        }
    }
}

#[test]
fn invalid_file_bytes_and_yaml_report_usage_errors_to_stderr() {
    let fixture = Fixture::new();
    for (case, source) in [
        ("invalid shebang bytes", b"#!\xff\nbody".as_slice()),
        (
            "invalid prompt bytes",
            b"---\r\nname: demo\r\n---\r\nbody\xc3",
        ),
        ("missing closing delimiter", b"---\nname: demo\n"),
        ("ill-formed YAML", b"---\nname: [\n---\nbody"),
    ] {
        fixture.assert_rejected(case, source);
    }
}
