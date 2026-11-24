//! §3.2 rules 1, 3 and 4 through the shipping CLI. A local executable records
//! exactly the IR seen by the provider. Its capture file also witnesses invocation.

use std::fs::{self, File, Permissions};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::TempDir;

struct Fixture {
    dir: TempDir,
    capture: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let dir = TempDir::new();
        let config = dir.0.join("config/agent");
        let hooks = config.join("hooks/pre-context.d");
        fs::create_dir_all(&hooks).unwrap();
        // Mask built-ins using §8.3 so the first system message is just the script body.
        for name in ["00-posix-runtime", "20-skills", "30-tips"] {
            fs::write(hooks.join(name), "").unwrap();
        }
        fs::write(
            config.join("config.toml"),
            r#"
[defaults]
model = "fixture-model"
provider = "fixture"

[providers.fixture]
bin = "record-provider"
timeout_sec = 2
"#,
        )
        .unwrap();
        let provider = dir.0.join("record-provider");
        fs::write(
            &provider,
            r#"#!/bin/sh
set -eu
/bin/cat > request.jsonl
printf '%s\n' '{"type":"text_delta","text":"ok"}' '{"type":"done","finish_reason":"stop"}'
"#,
        )
        .unwrap();
        fs::set_permissions(&provider, Permissions::from_mode(0o700)).unwrap();
        let capture = dir.0.join("request.jsonl");
        Self { dir, capture }
    }

    fn run(&self, script: &[u8]) -> (Option<i32>, Vec<u8>, Vec<u8>) {
        let root = &self.dir.0;
        fs::write(root.join("case.agent"), script).unwrap();
        let stdout = root.join("stdout");
        let stderr = root.join("stderr");
        let mut child = Command::new(env!("CARGO_BIN_EXE_agent"))
            .env_clear()
            .env("HOME", root)
            .env("PATH", root)
            .env("XDG_CONFIG_HOME", root.join("config"))
            .env("XDG_RUNTIME_DIR", root.join("run"))
            .current_dir(root)
            .args(["--stderr", "explicit", "case.agent", "--", "user input"])
            .stdin(Stdio::null())
            .stdout(File::create(&stdout).unwrap())
            .stderr(File::create(&stderr).unwrap())
            .spawn()
            .unwrap();
        // A regression that waits for input must fail the test without stalling the suite.
        let deadline = Instant::now() + Duration::from_secs(10);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("agent timed out for {script:?}");
            }
            thread::sleep(Duration::from_millis(10));
        };
        (
            status.code(),
            fs::read(stdout).unwrap(),
            fs::read(stderr).unwrap(),
        )
    }

    fn assert_rejected(&self, label: &str, source: &[u8]) {
        let (code, stdout, stderr) = self.run(source);
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
    let (code, stdout, stderr) = fixture.run(source.as_bytes());
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
