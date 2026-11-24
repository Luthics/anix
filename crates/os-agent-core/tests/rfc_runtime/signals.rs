//! RFC §§4.2, 6.1–6.4. Deliver real signals to processes started by the test.
use super::{Fixture, expect_code, jsonl, scripted_provider, success_text};
use std::fs::{self, File};
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::thread;
use std::time::{Duration, Instant};

struct Agent(Child);

impl Agent {
    fn start(fx: &Fixture, args: &[&str]) -> Self {
        fs::write(fx.dir.0.join("case.agent"), "frozen-test-text").unwrap();
        let c = fx
            .agent()
            .args(args)
            .stdout(Stdio::null())
            .stderr(File::create(fx.dir.0.join("agent-stderr")).unwrap())
            .spawn()
            .unwrap();
        Self(c)
    }
    fn signal(&self, signo: i32) {
        // SAFETY: Only target the positive PID returned by our own Child. It has
        // not been reaped so the kernel cannot reuse it.
        let rc = unsafe { libc::kill(self.0.id() as libc::pid_t, signo) };
        assert_eq!(rc, 0, "kill: {}", io::Error::last_os_error());
    }
    fn finish(&mut self, expected: i32) {
        until("agent termination", || self.0.try_wait().unwrap().is_some());
        assert_eq!(self.0.wait().unwrap().code(), Some(expected));
    }
}

impl Drop for Agent {
    fn drop(&mut self) {
        if matches!(self.0.try_wait(), Ok(None)) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

fn until(what: &str, mut ready: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(12);
    while !ready() {
        assert!(Instant::now() < end, "deadline waiting for {what}");
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn interruption_reaps_the_provider_discards_partial_assistant_and_releases_lock() {
    for (signal, code) in [(libc::SIGINT, 130), (libc::SIGTERM, 143)] {
        let fx = Fixture::new();
        let root = &fx.dir.0;
        fs::write(
            root.join("record-provider"),
            r#"#!/bin/sh
set -eu
/bin/cat > request
trap 'printf terminated > provider-stopped; exit 0' TERM
printf '%s\n' '{"type":"text_delta","text":"uncommitted text"}'
printf ready > provider-ready
i=0
while [ "$i" -lt 100 ]; do /bin/sleep 0.1; i=$((i + 1)); done
exit 1
"#,
        )
        .unwrap();
        let opts = ["-q", "-c", "history", "case.agent", "--", "initial"];
        let mut agent = Agent::start(&fx, &opts);
        until("active provider", || root.join("provider-ready").exists());
        agent.signal(signal);
        agent.finish(code);
        assert!(root.join("provider-stopped").exists());
        let rows = jsonl(&root.join("history"));
        assert!(rows.iter().all(|r| r["role"] != "assistant"));
        assert!(
            !fs::read_to_string(root.join("history"))
                .unwrap()
                .contains("uncommitted text")
        );
        scripted_provider(&fx, &[success_text("finished after resume")]);
        expect_code(&fx.run(fx.agent().args(opts)), 0);
    }
}

fn publish(root: &Path, name: &str, text: &str) {
    let tmp = root.join("mail/tmp").join(name);
    fs::write(&tmp, text).unwrap();
    fs::rename(tmp, root.join("mail/new").join(name)).unwrap();
}

#[test]
fn idle_process_holds_wal_and_usr2_snapshots_exact_ir_at_a_safe_boundary() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    scripted_provider(&fx, &[success_text("first response")]);
    let mut agent = Agent::start(
        &fx,
        &[
            "--inbox",
            "mail",
            "--idle-timeout",
            "0",
            "-q",
            "-c",
            "history",
            "case.agent",
        ],
    );
    until("inbox initialization", || root.join("mail/done").is_dir());
    publish(root, "job", "user input");
    until("job completion", || root.join("mail/done/job").exists());
    let frozen = fs::read(root.join("history")).unwrap();
    expect_code(
        &fx.run(
            fx.agent()
                .args(["-c", "history", "case.agent", "--", "cannot enter"]),
        ),
        1,
    );
    agent.signal(libc::SIGSTOP);
    let pid = agent.0.id() as libc::pid_t;
    let mut status = 0;
    until("stopped child", || {
        // SAFETY: pid is the test child; status is writable. WNOHANG prevents blocking.
        let result = unsafe { libc::waitpid(pid, &mut status, libc::WUNTRACED | libc::WNOHANG) };
        assert!(result >= 0);
        result == pid && libc::WIFSTOPPED(status)
    });
    agent.signal(libc::SIGCONT);
    agent.signal(libc::SIGUSR2);
    let prefix = format!("/tmp/agent_{pid}_");
    let path = || {
        fs::read_to_string(root.join("agent-stderr"))
            .unwrap()
            .lines()
            .flat_map(str::split_whitespace)
            .find(|s| s.starts_with(&prefix) && s.ends_with(".ctx.jsonl"))
            .map(PathBuf::from)
    };
    until("snapshot path on stderr", || path().is_some());
    let snapshot = path().unwrap();
    let rows = jsonl(&snapshot);
    let mode = fs::metadata(&snapshot).unwrap().permissions().mode() & 0o777;
    fs::remove_file(&snapshot).unwrap();
    assert_eq!(mode, 0o600);
    assert_eq!(rows[0]["type"], "header");
    assert_eq!(rows[0]["ir_version"], 1);
    let request = jsonl(&root.join("request-1"));
    assert_eq!(&rows[1..request.len()], &request[1..]);
    assert_eq!(rows.len(), request.len() + 1);
    assert_eq!(rows.last().unwrap()["text"], "first response");
    assert_eq!(fs::read(root.join("history")).unwrap(), frozen);
    agent.signal(libc::SIGTERM);
    agent.finish(143);
}

#[test]
fn usr1_forces_one_tool_free_compaction_before_the_next_inbox_inference() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    scripted_provider(
        &fx,
        &[
            success_text("before"),
            success_text("snapshot summary"),
            success_text("after"),
        ],
    );
    let mut agent = Agent::start(
        &fx,
        &[
            "-q",
            "--no-compact",
            "--inbox",
            "mail",
            "--idle-timeout",
            "0",
            "-c",
            "history",
            "case.agent",
        ],
    );
    until("inbox initialization", || root.join("mail/done").is_dir());
    publish(root, "first", "first job");
    until("first reply", || root.join("mail/done/first").is_file());
    agent.signal(libc::SIGUSR1);
    publish(root, "second", "second job");
    until("second reply", || root.join("mail/done/second").is_file());
    agent.signal(libc::SIGTERM);
    agent.finish(143);
    let compact = jsonl(&root.join("request-2"));
    assert!(compact[0]["tools"].as_array().unwrap().is_empty());
    let fresh = jsonl(&root.join("request-3"));
    assert!(!fresh[0]["tools"].as_array().unwrap().is_empty());
    assert_eq!(
        fresh[2]["text"],
        "<checkpoint>\nsnapshot summary\n</checkpoint>"
    );
    assert_eq!(
        jsonl(&root.join("history"))
            .iter()
            .filter(|r| r["type"] == "checkpoint")
            .count(),
        1
    );
    assert!(!root.join("request-4").exists());
}
