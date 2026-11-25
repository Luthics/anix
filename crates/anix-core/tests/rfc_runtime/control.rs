//! RFC v1.1 §§6.1–6.2 and 7.2. Drive the receiving end of fd 6 over actual pipes.
use super::{Fixture, expect_code, jsonl, scripted_provider, success_text, tool, with_tools};
use serde_json::{Value, json};
use std::fs;

fn run(fx: &Fixture) {
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
    expect_code(&out, 0);
    assert!(out.stdout.is_empty() && out.stderr.is_empty());
}

// RFC v1.1 §7.2, §6.2, §9.1
#[test]
fn concatenated_and_split_media_frames_keep_order_and_tool_identity_after_resume() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    fs::write(root.join("case.agent"), "frozen").unwrap();
    let images: Vec<_> = ["left.gif", "中.gif", "right.gif"]
        .iter()
        .map(|name| {
            let path = root.join(name);
            fs::write(&path, super::SINGLE_PIXEL_GIF).unwrap();
            json!({"mime":"image/gif","path":path})
        })
        .collect();
    let frames = |indices: &[usize]| {
        let mut bytes = Vec::new();
        for &i in indices {
            let frame = json!({"op":"media","mime":"image/gif","path":images[i]["path"],"source":"fixture"});
            serde_json::to_writer(&mut bytes, &frame).unwrap();
            bytes.push(b'\n');
        }
        bytes
    };
    fs::write(root.join("batch"), frames(&[2, 0, 2])).unwrap();
    let split = frames(&[1, 2]);
    let at = split
        .windows("中".len())
        .position(|w| w == "中".as_bytes())
        .unwrap()
        + 1;
    fs::write(root.join("part-a"), &split[..at]).unwrap();
    fs::write(root.join("part-b"), &split[at..]).unwrap();
    scripted_provider(
        &fx,
        &[
            with_tools([
                tool("batch", r#"/bin/cat batch > "$ANIX_CTL""#),
                tool(
                    "split",
                    r#"{ /bin/cat part-a; /bin/cat part-b; } > "$ANIX_CTL""#,
                ),
            ]),
            success_text("finished"),
            success_text("resumed"),
        ],
    );
    run(&fx);
    run(&fx);
    for file in ["request-2", "history", "request-3"] {
        let tools: Vec<_> = jsonl(&root.join(file))
            .into_iter()
            .filter(|r| r["role"] == "tool")
            .collect();
        assert_eq!(tools.len(), 2, "{file}");
        for (row, (id, expected)) in tools.iter().zip([
            ("batch", json!([images[2], images[0], images[2]])),
            ("split", json!([images[1], images[2]])),
        ]) {
            assert_eq!(row["tool_call_id"], id, "{file}");
            assert_eq!(row["media"], expected, "{file} {id}");
            assert_eq!(
                row["text"],
                format!("[t{}] exit=0\n", row["turn_id"]),
                "{file}"
            );
        }
    }
}

// RFC v1.1 §7.2, §6.1, §6.2
#[test]
fn two_valid_fold_frames_from_one_tool_are_appended_in_arrival_order() {
    // t1 is the user, t2 the assistant, t3 its tool. Folding from t3 first
    // aligns to t2 and leaves t1 available for the next frame.
    let first = json!({"op":"fold","from_turn":3,"summary":"first"});
    let second = json!({"op":"fold","from_turn":1,"summary":"second"});
    for (frames, expected) in [
        (vec![&first], vec![(2, "first")]),
        (vec![&second], vec![(1, "second")]),
        (vec![&first, &second], vec![(2, "first"), (1, "second")]),
    ] {
        let fx = Fixture::new();
        let root = &fx.dir.0;
        fs::write(root.join("case.agent"), "frozen").unwrap();
        let mut input = Vec::new();
        for frame in frames {
            serde_json::to_writer(&mut input, frame).unwrap();
            input.push(b'\n');
        }
        fs::write(root.join("fold-frames"), input).unwrap();
        scripted_provider(
            &fx,
            &[
                with_tools([tool("fold", r#"/bin/cat fold-frames > "$ANIX_CTL""#)]),
                success_text("finished"),
                success_text("resumed"),
            ],
        );
        run(&fx);
        run(&fx);
        let folds: Vec<Value> = jsonl(&root.join("history"))
            .into_iter()
            .filter(|r| r["type"] == "fold")
            .collect();
        let observed: Vec<_> = folds
            .iter()
            .map(|r| {
                (
                    r["from_turn"].as_u64().unwrap(),
                    r["summary"].as_str().unwrap(),
                )
            })
            .collect();
        assert_eq!(observed, expected);
        let (start, text) = expected.last().unwrap();
        let end = folds.last().unwrap()["to_turn"].as_u64().unwrap();
        let marker = format!("<folded turns=\"{start}..{end}\">{text}</folded>");
        for file in ["request-2", "request-3"] {
            let rows = jsonl(&root.join(file));
            assert_eq!(rows[1]["text"], "frozen");
            let matches: Vec<_> = rows
                .iter()
                .filter(|r| {
                    r["role"] == "user"
                        && r["text"]
                            .as_str()
                            .is_some_and(|s| s.starts_with("<folded turns="))
                })
                .collect();
            assert_eq!(matches.len(), 1, "{file}");
            assert_eq!(matches[0]["text"], marker, "{file}");
            assert!(!rows.iter().any(|r| r["role"] == "tool"), "{file}");
        }
    }
}
