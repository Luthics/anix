use super::{Fixture, jsonl};
use serde_json::{Value, json};
use std::fs;

fn run(fixture: &Fixture, vision: &str, input: &str) -> Vec<Value> {
    let result = fixture.run(fixture.anix().args([
        "-q",
        "-c",
        "history",
        vision,
        "case.agent",
        "--",
        input,
    ]));
    assert_eq!(
        result.status.code(),
        Some(0),
        "{:?}",
        String::from_utf8_lossy(&result.stderr)
    );
    jsonl(&fixture.capture)
}

// RFC v1.1 §1.4, §6.2, §9.1, §9.2
#[test]
fn legacy_wal_without_vision_or_fold_usage_resumes_without_rewriting_the_prefix() {
    let fixture = Fixture::new();
    let root = &fixture.dir.0;
    let history = root.join("history");
    let old = concat!(
        "{\"type\":\"session_init\",\"ir_version\":1,\"ts\":\"2026-01-01T00:00:00Z\",\"config\":{\"stderr\":\"explicit\",\"approval\":\"ask\"}}\n",
        "{\"type\":\"message\",\"turn_id\":0,\"role\":\"system\",\"text\":\" frozen\\r\\n前缀 \",\"ts\":\"2026-01-01T00:00:01Z\"}\n",
        "{\"type\":\"message\",\"turn_id\":1,\"role\":\"user\",\"text\":\"first\",\"ts\":\"2026-01-01T00:00:02Z\"}\n",
        "{\"type\":\"message\",\"turn_id\":2,\"role\":\"assistant\",\"text\":\"kept\",\"usage\":{\"input_tokens\":9,\"output_tokens\":4},\"ts\":\"2026-01-01T00:00:03Z\"}\n",
        "{\"type\":\"message\",\"turn_id\":3,\"role\":\"user\",\"text\":\"folded-away-user\",\"ts\":\"2026-01-01T00:00:04Z\"}\n",
        "{\"type\":\"message\",\"turn_id\":4,\"role\":\"assistant\",\"text\":\"folded-away-answer\",\"ts\":\"2026-01-01T00:00:05Z\"}\n",
        "{\"type\":\"fold\",\"from_turn\":3,\"to_turn\":4,\"summary\":\"summary\",\"ts\":\"2026-01-01T00:00:06Z\"}\n",
    );
    fs::write(&history, old).unwrap();
    fs::write(root.join("case.agent"), "new script text").unwrap();
    let ir = run(&fixture, "--vision", "resume");
    assert_eq!(
        ir[1],
        json!({"turn_id":0,"role":"system","text":" frozen\r\n前缀 "})
    );
    assert_eq!(ir[2]["text"], "first");
    assert_eq!(ir[3], json!({"turn_id":2,"role":"assistant","text":"kept"}));
    assert_eq!(ir[4]["role"], "user");
    assert_eq!(ir[4]["text"], "<folded turns=\"3..4\">summary</folded>");
    assert_eq!(ir.last().unwrap()["text"], "<args>resume</args>");
    assert!(ir.last().unwrap()["turn_id"].as_u64().unwrap() > 4);
    assert!(ir.iter().all(|r| !matches!(
        r["text"].as_str(),
        Some("folded-away-user" | "folded-away-answer")
    )));
    assert!(fs::read(&history).unwrap().starts_with(old.as_bytes()));
    assert_eq!(jsonl(&history).last().unwrap()["text"], "ok");
}

// RFC v1.1 §3.1, §5.5, §6.2, §9.1
#[test]
fn repeated_resume_appends_one_vision_update_per_change_and_reuses_frozen_text() {
    let fixture = Fixture::new();
    let root = &fixture.dir.0;
    let history = root.join("history");
    fs::write(root.join("case.agent"), "frozen").unwrap();
    run(&fixture, "--no-vision", "first");
    assert_eq!(jsonl(&history)[0]["config"]["vision"], false);
    fs::write(root.join("case.agent"), "later edit").unwrap();
    for (option, expected) in [
        ("--vision", vec![true]),
        ("--vision", vec![true]),
        ("--no-vision", vec![true, false]),
    ] {
        let prefix = fs::read(&history).unwrap();
        let ir = run(&fixture, option, "next");
        assert!(fs::read(&history).unwrap().starts_with(&prefix));
        let wal = jsonl(&history);
        let updates: Vec<_> = wal
            .iter()
            .filter(|r| r["type"] == "runtime_update")
            .collect();
        let flags: Vec<_> = updates
            .iter()
            .map(|r| r["config"]["vision"].as_bool().unwrap())
            .collect();
        assert_eq!(flags, expected);
        assert_eq!(ir[1], json!({"turn_id":0,"role":"system","text":"frozen"}));
        let incremental: Vec<_> = ir
            .iter()
            .skip(2)
            .filter(|r| r["role"] == "system")
            .collect();
        assert_eq!(incremental.len(), updates.len());
        for (message, update) in incremental.iter().zip(updates) {
            assert_eq!(message["turn_id"], update["turn_id"]);
            assert_eq!(
                message["text"],
                format!(
                    "<runtime_update>{}</runtime_update>",
                    update["content"].as_str().unwrap()
                )
            );
        }
    }
}
