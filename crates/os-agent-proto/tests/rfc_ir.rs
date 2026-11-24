//! RFC §9.1: Golden JSONL records and the invariants required before dispatch.
use os_agent_proto::ir::IrStream;
use serde_json::{Value, json};

fn header() -> Value {
    json!({"ir_version":1,"type":"header","tools":[],"meta":{"cwd":"/work","stderr_mode":"auto"}})
}
fn message(role: &str, turn_id: u64, text: &str) -> Value {
    json!({"role":role,"turn_id":turn_id,"text":text})
}
fn input(head: &Value, messages: &[Value]) -> String {
    std::iter::once(head)
        .chain(messages)
        .map(|v| format!("{v}\n"))
        .collect()
}
fn valid(messages: &[Value]) -> bool {
    IrStream::from_jsonl(&input(&header(), messages))
        .and_then(|ir| ir.validate_invariants())
        .is_ok()
}
fn calls(ids: &[&str]) -> Value {
    let tools: Vec<_> = ids
        .iter()
        .map(|id| json!({"id":id,"name":"msh","arguments":{"cmd":"pwd"}}))
        .collect();
    json!({"role":"assistant","turn_id":2,"tool_calls":tools})
}
fn result(id: &str, tid: u64) -> Value {
    json!({"role":"tool","turn_id":tid,"tool_call_id":id,"text":"result"})
}

#[test]
fn required_header_fields_and_types_cannot_be_dropped_or_moved() {
    let body = message("user", 1, "x");
    for key in ["type", "ir_version", "tools", "meta"] {
        let mut h = header();
        h.as_object_mut().unwrap().remove(key);
        assert!(
            IrStream::from_jsonl(&input(&h, std::slice::from_ref(&body))).is_err(),
            "{key}"
        );
    }
    for (key, v) in [
        ("ir_version", json!(0)),
        ("ir_version", json!(2)),
        ("ir_version", json!("1")),
        ("type", json!("message")),
        ("tools", json!({})),
        ("meta", json!(null)),
    ] {
        let mut h = header();
        h[key] = v;
        assert!(
            IrStream::from_jsonl(&input(&h, std::slice::from_ref(&body))).is_err(),
            "{h}"
        );
    }
    assert!(!valid(&[]));
    assert!(IrStream::from_jsonl(&input(&body, &[header()])).is_err());
    assert!(IrStream::from_jsonl(&input(&header(), &[header(), body])).is_err());
}

#[test]
fn golden_ir_keeps_frozen_text_extensions_and_attachment_references_without_opening_files() {
    let mut h = header();
    h["x_header"] = json!({"opaque":[1,true]});
    let mut sys = message("system", 0, "  冻结\r\n\n🦀\t ");
    sys["x_note"] = json!("preserved");
    let mut user = message("user", 1, "读图");
    user["media"] = json!([{"mime":"image/png","path":"/no-files-needed/image.png"}]);
    let rows = [sys, user, calls(&["a"]), result("a", 3)];
    let original = input(&h, &rows);
    let parsed = IrStream::from_jsonl(&original).unwrap();
    parsed.validate_invariants().unwrap();
    let out = parsed.to_jsonl().unwrap();
    assert!(out.ends_with('\n'));
    let expected: Vec<_> = std::iter::once(h).chain(rows).collect();
    let actual: Vec<Value> = out
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    assert_eq!(actual, expected);
}

#[test]
fn tool_replies_form_a_bijection_and_are_contiguous() {
    for first in ["a", "b", "other"] {
        for second in ["a", "b", "other"] {
            let rows = [calls(&["a", "b"]), result(first, 3), result(second, 4)];
            assert_eq!(
                valid(&rows),
                first != second && first != "other" && second != "other",
                "{first}, {second}"
            );
        }
    }
    for bad in [
        vec![result("a", 3)],
        vec![calls(&["a"])],
        vec![calls(&["a", "b"]), result("a", 3)],
        vec![
            calls(&["a"]),
            message("user", 3, "interleaved"),
            result("a", 4),
        ],
        vec![calls(&["a"]), result("a", 3), result("a", 4)],
    ] {
        assert!(!valid(&bad), "{bad:?}");
    }
}

#[test]
fn assistant_requires_text_or_calls_even_with_reasoning_or_opaque_state() {
    for extra in [
        json!({}),
        json!({"text":""}),
        json!({"tool_calls":[]}),
        json!({"reasoning":"thinking","provider_opaque":{"sig":1}}),
    ] {
        let mut m = json!({"turn_id":2,"role":"assistant"});
        m.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        assert!(!valid(&[m.clone()]), "{m}");
        m["text"] = json!("answer");
        assert!(valid(&[m]));
    }
}

#[test]
fn ordinary_turns_increase_strictly_and_checkpoint_anchor_may_be_shared() {
    assert!(valid(&[
        message("system", 9, "frozen"),
        message("user", 9, "<checkpoint>\ns\n</checkpoint>")
    ]));
    for tid in [0, 2, 7, 8] {
        let rows = [
            message("user", 7, "first"),
            message("assistant", tid, "next"),
        ];
        assert_eq!(
            valid(&rows),
            tid > 7,
            "successive conversational turn IDs 7 and {tid}"
        );
    }
}
