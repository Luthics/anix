//! RFC §§4.2, 6.1–6.4. Deliver real signals to processes started by the test.
use super::background::{Agent, until};
use super::inbox::publish;
use super::{Fixture, expect_code, jsonl, scripted_provider, success_text};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::{Duration, Instant};

// RFC §6.4-01 — 中断后收回 Provider、不提交半截回答，下一进程可以重新获取 WAL 锁。
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
        expect_code(&fx.run(fx.anix().args(opts)), 0);
    }
}

// RFC §6.4-05 — 保持模式进程收到 STOP 后暂停，CONT 恢复后继续响应快照请求。
// RFC §6.4-07 — USR2 输出快照路径，文件权限为 0600，内容与活跃 IR 相符。
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
            fx.anix()
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
    let prefix = format!("/tmp/anix_{pid}_");
    let path = || {
        fs::read_to_string(root.join("anix-stderr"))
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

// RFC §6.4-06 — USR1 在下次推理前执行一次无工具的上下文压缩。
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

// RFC §10.2-02 — 在无 pidfd 的 Linux 配置里，SIGTERM 的子进程清理仍须按时结束。
#[test]
fn sigterm_bounds_cleanup_of_a_provider_that_acknowledges_term() {
    assert_bounded_provider_cleanup(libc::SIGTERM, 143);
}

// RFC §6.3-02 — 向响应 TERM 的 Provider 发 SIGINT，检查回收仍受截止时间限制。
// RFC §10.2-02 — 在无 pidfd 的 Linux 配置里，SIGINT 也必须按期限回收已收到 TERM 的 Provider。
#[test]
fn sigint_bounds_cleanup_of_a_provider_that_acknowledges_term() {
    assert_bounded_provider_cleanup(libc::SIGINT, 130);
}

fn assert_bounded_provider_cleanup(signal: libc::c_int, expected_code: i32) {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    fs::write(
        root.join("record-provider"),
        r#"#!/bin/sh
set -eu
/bin/cat > /dev/null
trap 'printf acknowledged > term-seen' TERM
: > ready
i=0
while [ "$i" -lt 200 ]; do /bin/sleep 0.1 || :; i=$((i+1)); done
printf finished > natural-exit
"#,
    )
    .unwrap();
    let mut agent = Agent::start(&fx, &["-q", "-c", "history", "case.agent", "--", "go"]);
    until("active provider", || root.join("ready").is_file());
    let start = Instant::now();
    agent.signal(signal);
    agent.finish(expected_code);
    assert!(start.elapsed() >= Duration::from_millis(2950));
    assert_eq!(
        fs::read_to_string(root.join("term-seen")).unwrap(),
        "acknowledged"
    );
    assert!(!root.join("natural-exit").exists());
    scripted_provider(&fx, &[success_text("new process took lock")]);
    expect_code(
        &fx.run(
            fx.anix()
                .args(["-q", "-c", "history", "case.agent", "--", "resume"]),
        ),
        0,
    );
}

// RFC §10.2-01 — Linux 上主进程异常终止后，活跃的 Provider 或 shell 子进程也终止。
#[cfg(target_os = "linux")]
#[test]
fn abrupt_parent_exit_stops_the_active_provider_or_shell_child() {
    use super::{tool, with_tools};
    for active_provider in [true, false] {
        let fx = Fixture::new();
        let root = &fx.dir.0;
        let script = r#"printf '%s\n' "$$" > "$HOME/child-pid.tmp"; /bin/mv "$HOME/child-pid.tmp" "$HOME/child-pid"; exec /bin/sleep 15"#;
        if active_provider {
            fs::write(
                root.join("record-provider"),
                format!("#!/bin/sh\n/bin/cat >/dev/null\n{script}\n"),
            )
            .unwrap();
        } else {
            scripted_provider(&fx, &[with_tools([tool("active", script)])]);
        }
        let mut agent = Agent::start(
            &fx,
            &["-q", "-y", "--shell", "/bin/sh", "case.agent", "--", "go"],
        );
        until("child pid published", || root.join("child-pid").exists());
        let pid: u32 = fs::read_to_string(root.join("child-pid"))
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let stat = PathBuf::from(format!("/proc/{pid}/stat"));
        let before = fs::read_to_string(&stat).unwrap();
        // proc stat fields begin with pid, (comm), state, ppid.
        assert_eq!(
            before
                .rsplit_once(')')
                .unwrap()
                .1
                .split_whitespace()
                .nth(1)
                .unwrap(),
            agent.0.id().to_string()
        );
        agent.signal(libc::SIGKILL);
        let status = agent.0.wait().unwrap();
        assert!(!status.success());
        until(
            "child terminated after abrupt parent death",
            || match fs::read_to_string(&stat) {
                Ok(row) => matches!(
                    row.rsplit_once(')').unwrap().1.split_whitespace().next(),
                    Some("Z" | "X")
                ),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => true,
                Err(e) => panic!("read {stat:?}: {e}"),
            },
        );
    }
}
