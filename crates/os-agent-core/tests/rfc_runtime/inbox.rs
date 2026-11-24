//! §4.2. Hold a tool open while multiple independent publishers finish their renames.
use super::background::{Agent, until};
use super::{Fixture, jsonl, scripted_provider, success_text, tool, with_tools};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::sync::Barrier;
use std::thread;

// RFC §4.2, §6.2
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
