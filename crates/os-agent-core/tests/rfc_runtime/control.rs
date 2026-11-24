//! §§6.1–6.2 and 7.2. Drive the receiving end of fd 6 over actual pipes.
use super::{Fixture, expect_code, jsonl, scripted_provider, success_text, tool, with_tools};
use serde_json::{Value, json};
use std::fs;

fn run(fx: &Fixture) {
    let out = fx.run(fx.agent().args([
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

// RFC §7.2, §9.1, §6.2
#[test]
fn tool_control_frames_cannot_publish_invalid_media_references_to_wal_or_provider_ir() {
    let mut violations = Vec::new();
    for (mime, absolute) in [
        ("image/png", true),
        ("image/png", false),
        ("text/plain", true),
    ] {
        let fx = Fixture::new();
        let root = &fx.dir.0;
        fs::write(root.join("case.agent"), "frozen").unwrap();
        let image = root.join("fixture.png");
        fs::write(
            &image,
            include_bytes!("../../../provider-openai-responses/tests/media/blue.png"),
        )
        .unwrap();
        let path = if absolute {
            image.as_path()
        } else {
            std::path::Path::new("fixture.png")
        };
        let frame = json!({"op":"media","mime":mime,"path":path,"source":"fixture"});
        fs::write(root.join("frame"), format!("{frame}\n")).unwrap();
        scripted_provider(
            &fx,
            &[
                with_tools([tool(
                    "emit",
                    r#"/bin/cat frame > "$AGENT_CTL" && : > delivered"#,
                )]),
                success_text("done"),
            ],
        );
        let out = fx.run(fx.agent().args([
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
        assert!(
            root.join("delivered").is_file(),
            "{frame}: {:?}",
            out.stderr
        );
        let valid = mime == "image/png" && absolute;
        if valid {
            expect_code(&out, 0);
        }
        for name in ["history", "request-2"] {
            let input = root.join(name);
            if !input.exists() {
                assert!(!valid, "{name}: {frame}");
                continue;
            }
            let media: Vec<_> = jsonl(&input)
                .into_iter()
                .filter(|r| r["role"] == "tool")
                .flat_map(|r| r["media"].as_array().cloned().unwrap_or_default())
                .collect();
            if valid {
                assert_eq!(media, [json!({"mime":mime,"path":image})]);
            }
            // The error policy for malformed control frames is independent of
            // the schema of records that do reach the provider or the WAL.
            for item in media {
                let allowed = matches!(
                    item["mime"].as_str(),
                    Some("image/png" | "image/jpeg" | "image/webp" | "image/gif")
                );
                let absolute_path = item["path"]
                    .as_str()
                    .is_some_and(|s| std::path::Path::new(s).is_absolute());
                if !allowed || !absolute_path {
                    violations.push(format!("{name}: {item}"));
                }
            }
        }
    }
    assert!(
        violations.is_empty(),
        "invalid published attachments: {violations:?}"
    );
}

// RFC §7.2, §6.2, §9.1
#[test]
fn concatenated_and_split_media_frames_keep_order_and_tool_identity_after_resume() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    fs::write(root.join("case.agent"), "frozen").unwrap();
    let images: Vec<_> = ["left.png", "中.png", "right.png"]
        .iter()
        .map(|name| {
            let path = root.join(name);
            fs::write(
                &path,
                include_bytes!("../../../provider-openai-responses/tests/media/blue.png"),
            )
            .unwrap();
            json!({"mime":"image/png","path":path})
        })
        .collect();
    let frames = |indices: &[usize]| {
        let mut bytes = Vec::new();
        for &i in indices {
            let frame = json!({"op":"media","mime":"image/png","path":images[i]["path"],"source":"fixture"});
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
                tool("batch", r#"/bin/cat batch > "$AGENT_CTL""#),
                tool(
                    "split",
                    r#"{ /bin/cat part-a; /bin/cat part-b; } > "$AGENT_CTL""#,
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

// RFC §7.2, §6.1, §6.2
#[test]
fn two_valid_fold_frames_from_one_tool_are_appended_in_arrival_order() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    fs::write(root.join("case.agent"), "frozen").unwrap();
    // t1 is the user, t2 is the assistant, t3 is its tool. After folding
    // the tool pair, t1 still exists and is valid for the second fold.
    let cmd = r#"printf '%s\n' '{"op":"fold","from_turn":3,"summary":"first"}' '{"op":"fold","from_turn":1,"summary":"second"}' > "$AGENT_CTL""#;
    scripted_provider(
        &fx,
        &[
            with_tools([tool("fold", cmd)]),
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
        .map(|r| (r["from_turn"].as_u64(), r["summary"].as_str()))
        .collect();
    assert_eq!(
        observed,
        [(Some(2), Some("first")), (Some(1), Some("second"))]
    );
    let marker = format!(
        "<folded turns=\"1..{}\">second</folded>",
        folds[1]["to_turn"]
    );
    for file in ["request-2", "request-3"] {
        let rows = jsonl(&root.join(file));
        assert_eq!(rows[2]["role"], "user");
        assert_eq!(rows[2]["text"], marker);
        assert!(!rows.iter().any(|r| r["role"] == "tool"));
    }
}
