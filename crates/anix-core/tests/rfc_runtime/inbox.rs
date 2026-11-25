//! §4.2. Hold a tool open while multiple independent publishers finish their renames.
use super::background::{Agent, until};
use super::{Fixture, expect_code, jsonl, scripted_provider, success_text, tool, with_tools};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Barrier;
use std::thread;
use std::time::{Duration, Instant};

pub(super) fn publish(root: &Path, name: &str, text: &str) {
    let tmp = root.join("mail/tmp").join(name);
    fs::write(&tmp, text).unwrap();
    fs::rename(tmp, root.join("mail/new").join(name)).unwrap();
}

// RFC v1.1 §4.2, §6.2
#[test]
fn concurrent_publications_wait_for_tool_closure_and_preserve_each_complete_body() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    let mail = root.join("mail");
    let names = ["z-last", "B-middle", "A-first"];
    let bodies: Vec<_> = names
        .iter()
        .map(|n| format!("BEGIN-{n}\n{}\nEND-{n}", n.repeat(4096)))
        .collect();
    let command = r#": > tool-busy
i=0
while [ ! -f release ] && [ "$i" -lt 240 ]; do /bin/sleep 0.1; i=$((i+1)); done
[ -f release ] && printf complete"#;
    let mut turns = vec![
        with_tools([tool("hold", command)]),
        success_text("seed finished"),
    ];
    turns.extend(names.iter().map(|_| success_text("job finished")));
    scripted_provider(&fx, &turns);
    let mut agent = Agent::start(
        &fx,
        &[
            "-q",
            "-y",
            "--shell",
            "/bin/sh",
            "--inbox",
            "mail",
            "--idle-timeout",
            "0",
            "-c",
            "history",
            "case.agent",
        ],
    );
    until("inbox initialization", || mail.join("done").is_dir());
    for name in ["", "tmp", "new", "cur", "done"] {
        assert_eq!(
            fs::metadata(mail.join(name)).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }
    let seed = mail.join("tmp/seed");
    fs::write(&seed, "seed message").unwrap();
    fs::rename(seed, mail.join("new/seed")).unwrap();
    until("active tool", || root.join("tool-busy").exists());
    let barrier = Barrier::new(names.len());
    thread::scope(|scope| {
        for (name, body) in names.iter().zip(&bodies) {
            let mail = &mail;
            let barrier = &barrier;
            scope.spawn(move || {
                let staging = mail.join("tmp").join(name);
                barrier.wait();
                fs::write(&staging, body).unwrap();
                fs::rename(staging, mail.join("new").join(name)).unwrap();
            });
        }
    });
    for name in names {
        assert!(mail.join("new").join(name).is_file());
        assert!(!mail.join("cur").join(name).exists());
    }
    assert!(mail.join("cur/seed").is_file());
    assert!(!root.join("request-2").exists());
    fs::write(root.join("release"), "").unwrap();
    until("all commits", || {
        names.iter().all(|n| mail.join("done").join(n).is_file())
    });
    agent.signal(libc::SIGTERM);
    agent.finish(143);
    let wal = jsonl(&root.join("history"));
    let users: Vec<_> = wal.iter().filter(|m| m["role"] == "user").collect();
    assert_eq!(users.len(), 1 + names.len());
    let req = jsonl(&root.join("request-2"));
    let last = req.last().unwrap();
    assert_eq!(last["role"], "tool");
    assert_eq!(last["tool_call_id"], "hold");
    assert!(last["text"].as_str().unwrap().ends_with("complete"));
    for (i, (name, body)) in names.iter().zip(&bodies).rev().enumerate() {
        let request = jsonl(&root.join(format!("request-{}", i + 3)));
        let user = request.last().unwrap();
        assert_eq!(user["role"], "user");
        assert_eq!(user["text"], users[i + 1]["text"]);
        let enclosed = user["text"]
            .as_str()
            .unwrap()
            .strip_prefix(&format!("<input source=\"{name}\">"))
            .unwrap()
            .strip_suffix("</input>")
            .unwrap();
        assert_eq!(enclosed.trim_matches('\n'), body);
        assert_eq!(
            fs::read_to_string(mail.join("done").join(name)).unwrap(),
            *body
        );
    }
}

// RFC v1.1 §4.2, §6.2, §9.1
#[test]
fn a_publisher_can_queue_work_while_the_initial_cli_task_is_running() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    fs::write(root.join("case.agent"), "queue prompt").unwrap();
    for dir in ["tmp", "new", "cur", "done"] {
        fs::create_dir_all(root.join("mail").join(dir)).unwrap();
    }
    let enqueue = r#"printf 'queued body' > mail/tmp/100-20-1 && /bin/mv mail/tmp/100-20-1 mail/new/100-20-1"#;
    scripted_provider(
        &fx,
        &[
            with_tools([tool("publish", enqueue)]),
            success_text("initial complete"),
            success_text("queued complete"),
        ],
    );
    let out = fx.run(fx.anix().args([
        "-q",
        "-y",
        "--shell",
        "/bin/sh",
        "--inbox",
        "mail",
        "--idle-timeout",
        "1",
        "-c",
        "history",
        "case.agent",
        "--",
        "initial",
    ]));
    expect_code(&out, 0);
    let tool_result = jsonl(&root.join("request-2")).pop().unwrap();
    assert_eq!(tool_result["role"], "tool");
    assert_eq!(tool_result["tool_call_id"], "publish");
    assert_ne!(tool_result["is_error"], true, "{tool_result}");
    let archived = root.join("mail/done/100-20-1");
    assert!(
        archived.is_file(),
        "publication was not consumed: {tool_result}"
    );
    assert_eq!(fs::read(archived).unwrap(), b"queued body");
    let queued = jsonl(&root.join("request-3"));
    assert_eq!(queued[2]["text"], "<args>initial</args>");
    let tail = &queued[queued.len() - 2..];
    assert_eq!(tail[0]["text"], "initial complete");
    assert_eq!(tail[1]["role"], "user");
    assert!(
        tail[1]["text"]
            .as_str()
            .unwrap()
            .starts_with("<input source=\"100-20-1\">")
    );
    let wal = jsonl(&root.join("history"));
    assert_eq!(wal.last().unwrap()["text"], "queued complete");
}

// RFC v1.1 §4.2, §6.2
#[test]
fn idle_timeout_starts_after_the_last_job_finishes_and_releases_the_wal() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    scripted_provider(
        &fx,
        &[
            success_text("first complete"),
            success_text("second complete"),
            success_text("reopened"),
        ],
    );
    let provider = root.join("record-provider");
    let script = fs::read_to_string(&provider).unwrap().replace(
        "/bin/cat \"$HOME/reply-$n\"",
        r#"if [ "$n" = 1 ] || [ "$n" = 2 ]; then
    : > "busy-$n"
    i=0
    while [ ! -f "release-$n" ] && [ "$i" -lt 200 ]; do /bin/sleep 0.05; i=$((i+1)); done
    [ -f "release-$n" ] || exit 1
fi
/bin/cat "$HOME/reply-$n""#,
    );
    fs::write(provider, script).unwrap();
    for dir in ["tmp", "new"] {
        fs::create_dir_all(root.join("mail").join(dir)).unwrap();
    }
    publish(root, "100-20-1", "first");
    let mut agent = Agent::start(
        &fx,
        &[
            "-q",
            "--inbox",
            "mail",
            "--idle-timeout",
            "1",
            "-c",
            "history",
            "case.agent",
        ],
    );
    until("first task inside provider", || {
        root.join("busy-1").exists()
    });
    publish(root, "100-20-2", "second");
    assert!(root.join("mail/new/100-20-2").is_file());
    fs::write(root.join("release-1"), "").unwrap();
    until("second task inside provider", || {
        root.join("busy-2").exists()
    });
    assert!(root.join("mail/done/100-20-1").is_file());
    thread::sleep(Duration::from_millis(1100));
    assert!(agent.0.try_wait().unwrap().is_none());
    assert!(root.join("mail/cur/100-20-2").is_file());
    expect_code(
        &fx.run(
            fx.anix()
                .args(["-c", "history", "case.agent", "--", "lock probe"]),
        ),
        1,
    );
    let released = Instant::now();
    fs::write(root.join("release-2"), "").unwrap();
    agent.finish(0);
    assert!(
        released.elapsed() >= Duration::from_secs(1),
        "idle timer included time spent in the provider"
    );
    assert!(root.join("mail/done/100-20-2").is_file());
    expect_code(
        &fx.run(
            fx.anix()
                .args(["-c", "history", "case.agent", "--", "after idle exit"]),
        ),
        0,
    );
    let ir = jsonl(&root.join("request-3"));
    let replies: Vec<_> = ir
        .iter()
        .filter(|r| r["role"] == "assistant")
        .map(|r| r["text"].as_str().unwrap())
        .collect();
    assert_eq!(replies, ["first complete", "second complete"]);
}

// RFC v1.1 §4.2, §6.2, §6.5, §9.2
#[test]
fn oneshot_nonzero_after_done_discards_uncommitted_calls_and_preserves_later_jobs() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    fs::write(root.join("case.agent"), "queue prompt").unwrap();
    for dir in ["tmp", "new"] {
        fs::create_dir_all(root.join("mail").join(dir)).unwrap();
    }
    publish(root, "100-20-1", "first payload");
    publish(root, "100-20-2", "later payload");
    scripted_provider(
        &fx,
        &[
            with_tools([tool("uncommitted", "printf unexpected > tool-output")]),
            success_text("after failure"),
        ],
    );
    let provider = root.join("record-provider");
    fs::write(
        &provider,
        fs::read_to_string(&provider).unwrap()
            + "\nif [ \"$n\" = 1 ]; then printf 'failure after done\\n' >&2; exit 23; fi\n",
    )
    .unwrap();
    let out = fx.run(fx.anix().args([
        "-q",
        "-y",
        "--shell",
        "/bin/sh",
        "--inbox",
        "mail",
        "--idle-timeout",
        "1",
        "-c",
        "history",
        "case.agent",
    ]));
    expect_code(&out, 1);
    assert!(String::from_utf8_lossy(&out.stderr).contains("failure after done"));
    assert!(!root.join("tool-output").exists());
    assert!(!root.join("request-2").exists());
    assert_eq!(
        fs::read(root.join("mail/new/100-20-2")).unwrap(),
        b"later payload"
    );
    let wal = jsonl(&root.join("history"));
    assert!(
        wal.iter()
            .any(|r| r["role"] == "user" && r["text"].as_str().unwrap().contains("first payload"))
    );
    assert!(
        wal.iter()
            .all(|r| r["role"] != "assistant" && r["role"] != "tool")
    );
    expect_code(
        &fx.run(
            fx.anix()
                .args(["-c", "history", "case.agent", "--", "reopen"]),
        ),
        0,
    );
    let resumed = jsonl(&root.join("request-2"));
    assert!(
        resumed
            .iter()
            .all(|r| r["role"] != "assistant" && r["role"] != "tool")
    );
}

// RFC v1.1 §4.2, §6.3, §9.5
#[cfg(target_os = "linux")]
#[test]
fn an_idle_inbox_sleeps_without_periodic_wakeups_then_consumes_a_publication() {
    use std::io::{BufRead, BufReader};
    use std::os::unix::net::UnixListener;
    use std::process::Stdio;

    let fx = Fixture::new();
    let root = &fx.dir.0;
    fs::write(root.join("case.agent"), "idle test").unwrap();
    // Keep sun_path short even when the build runs under a long TMPDIR.
    let run = super::TempDir::under(Path::new("/tmp"));
    let journal = run.0.join("anix");
    fs::create_dir(&journal).unwrap();
    let listener = UnixListener::bind(journal.join("journal.sock")).unwrap();
    listener.set_nonblocking(true).unwrap();
    let mut agent = Agent(
        fx.anix()
            .env("XDG_RUNTIME_DIR", &run.0)
            .args(["-q", "--inbox", "mail", "--idle-timeout", "0", "case.agent"])
            .stderr(Stdio::null())
            .stdout(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let mut stream = None;
    until("journal connection", || match listener.accept() {
        Ok((peer, _)) => {
            stream = Some(peer);
            true
        }
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => false,
        Err(e) => panic!("journal accept: {e}"),
    });
    let peer = stream.unwrap();
    peer.set_read_timeout(Some(Duration::from_secs(8))).unwrap();
    let mut events = BufReader::new(peer)
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(&line.unwrap()).unwrap());
    let idle = events.find(|e| e["type"] == "idle").unwrap();
    assert_eq!(idle["idle_timeout_sec"], 0);

    // Observe the main event loop only. Involuntary scheduling (other processes
    // using the CPU) is deliberately excluded from this per-thread counter.
    let stat = std::path::PathBuf::from(format!("/proc/{}/status", agent.0.id()));
    let mut switches_before = 0;
    let voluntary_switches = |status: &str| -> u64 {
        status
            .lines()
            .find_map(|line| line.strip_prefix("voluntary_ctxt_switches:"))
            .unwrap()
            .trim()
            .parse()
            .unwrap()
    };
    until("main thread asleep after the idle event", || {
        let status = fs::read_to_string(&stat).unwrap();
        if status
            .lines()
            .any(|line| line.starts_with("State:") && line.contains("S (sleeping)"))
        {
            switches_before = voluntary_switches(&status);
            return true;
        }
        false
    });
    thread::sleep(Duration::from_millis(1250));
    assert_eq!(
        voluntary_switches(&fs::read_to_string(&stat).unwrap()),
        switches_before,
        "the idle event loop woke and slept again without work"
    );

    publish(root, "100-20-1", "wake once");
    let mut observed = Vec::new();
    for event in events.by_ref() {
        let idle = event["type"] == "idle";
        observed.push(event);
        if idle {
            break;
        }
    }
    assert_eq!(observed.last().unwrap()["type"], "idle");
    let cycles: Vec<_> = observed
        .iter()
        .filter(|e| e["type"] == "cycle_end")
        .collect();
    assert_eq!(
        cycles.len(),
        1,
        "new idle before the published task completed"
    );
    assert_eq!(cycles[0]["source"], "100-20-1");
    assert_eq!(cycles[0]["exit_code"], 0);
    assert_eq!(
        fs::read(root.join("mail/done/100-20-1")).unwrap(),
        b"wake once"
    );
    agent.signal(libc::SIGTERM);
    agent.finish(143);
}
