//! RFC v1.1 §9.1: Golden JSONL records and the invariants required before dispatch.
use anix_proto::ir::IrStream;
use anix_proto::provider::ProviderRequestEnvelope;
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

// RFC §9.1-01 — header 必填字段不得移入消息或其他对象。
// RFC §9.1-03 — 删除或移位必填 header 字段应拒绝。
// RFC §9.1-05 — 从 header 移除 tools 应导致 IR 无效。
// RFC §9.1-19 — 要求首行 header 及后续消息的位置和字段类型合法。
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

// RFC §9.1-07 — 对 header.params 和 meta 逐项验证 wire 类型。
// RFC §9.1-08 — 验证 meta 的 cwd、stderr_mode 与预算字段的类型。
#[test]
fn meta_and_header_options_have_their_declared_wire_types() {
    let body = [message("user", 1, "go")];
    for key in ["cwd", "stderr_mode"] {
        let mut h = header();
        h["meta"].as_object_mut().unwrap().remove(key);
        assert!(IrStream::from_jsonl(&input(&h, &body)).is_err(), "{key}");
    }
    for (key, invalid) in [
        ("params", vec![json!([]), json!("object"), json!(false)]),
        (
            "max_output_tokens",
            vec![json!(-1), json!(1.5), json!(u64::from(u32::MAX) + 1)],
        ),
    ] {
        for value in invalid {
            let mut h = header();
            h[key] = value;
            assert!(IrStream::from_jsonl(&input(&h, &body)).is_err(), "{h}");
        }
    }
    for mode in ["auto", "explicit"] {
        let mut h = header();
        h["meta"]["stderr_mode"] = json!(mode);
        h["max_output_tokens"] = json!(u32::MAX - 1);
        h["params"] = json!({"future": ["a", {"b":true}]});
        let ir = IrStream::from_jsonl(&input(&h, &body)).unwrap();
        ir.validate_invariants().unwrap();
        let encoded: Value =
            serde_json::from_str(ir.to_jsonl().unwrap().lines().next().unwrap()).unwrap();
        assert_eq!(encoded, h);
    }
}

// RFC §9.1-12 — 非对象的 arguments 或 provider_opaque 不应通过已校验 IR 的序列化。
#[test]
fn validated_ir_only_serializes_objects_for_arguments_and_provider_opaque() {
    let mut violations = Vec::new();
    let mut head = header();
    head["tools"] = json!([{"name":"msh","description":"fixture","parameters":{"type":"object"}}]);
    for field in ["arguments", "provider_opaque"] {
        for value in [
            json!({}),
            json!({"nested":[3]}),
            json!("x"),
            json!(false),
            json!([1]),
            json!(7),
        ] {
            let mut assistant = calls(&["c"]);
            if field == "arguments" {
                assistant["tool_calls"][0][field] = value.clone();
            } else {
                assistant[field] = value.clone();
            }
            let parsed = IrStream::from_jsonl(&input(&head, &[assistant, result("c", 3)]))
                .and_then(|ir| {
                    ir.validate_invariants()?;
                    Ok(ir)
                });
            if value.is_object() {
                assert!(parsed.is_ok(), "{field} = {value}");
            }
            if let Ok(ir) = parsed {
                let wire = ir.to_jsonl().unwrap();
                let out: Value = serde_json::from_str(wire.lines().nth(1).unwrap()).unwrap();
                let emitted = if field == "arguments" {
                    out["tool_calls"][0].get(field)
                } else {
                    out.get(field)
                };
                if value.is_object() {
                    assert_eq!(emitted, Some(&value));
                } else if emitted.is_some_and(|v| !v.is_object())
                    || (field == "arguments" && emitted.is_none())
                {
                    violations.push(format!("{field} = {value}"));
                }
            }
        }
    }
    assert!(
        violations.is_empty(),
        "non-object fields in validated IR: {violations:?}"
    );
}

// RFC §9.1-02 — 逐行解码真实样例，保留冻结正文、扩展字段和附件引用。
// RFC §9.1-09 — IR 样例中的未知扩展字段在解码后保留。
#[test]
fn golden_ir_accepts_extensions_and_keeps_frozen_text_and_attachment_references() {
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
    // §9.1 permits readers to ignore unknown extensions. Compare the
    // specified fields independently of whether the library forwards them.
    let mut expected: Vec<_> = std::iter::once(h).chain(rows).collect();
    let mut actual: Vec<Value> = out
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    for records in [&mut expected, &mut actual] {
        records[0].as_object_mut().unwrap().remove("x_header");
        records[1].as_object_mut().unwrap().remove("x_note");
    }
    assert_eq!(actual, expected);
}

// RFC §9.1-13 — tool 结果必须连续且与对应调用一一配对。
// RFC §9.1-19 — 连续 tool 结果与同批调用一一对应。
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

// RFC §9.1-19 — 重复调用 ID 不能由同一个工具结果同时闭合。
#[test]
fn a_single_tool_result_cannot_close_two_calls_with_the_same_id() {
    for id in ["call-a", "call-b"] {
        assert!(valid(&[calls(&[id]), result(id, 3)]));
        let rows = [calls(&[id, id]), result(id, 3)];
        assert!(!valid(&rows), "one result closed two calls: {rows:?}");
    }
}

// RFC §9.1-02 — 分别删除或改错四种角色的必填字段，解码应失败。
// RFC §9.1-11 — user 缺必填字段或 turn_id 型别错误时拒绝。
// RFC §9.1-13 — tool 消息缺少必需的调用标识时拒绝。
#[test]
fn required_message_fields_and_turn_id_types_hold_for_all_four_roles() {
    for (row, required) in [
        (message("system", 0, "s"), &["role", "turn_id", "text"][..]),
        (message("user", 1, "u"), &["role", "turn_id", "text"][..]),
        (message("assistant", 2, "a"), &["role", "turn_id"][..]),
        (
            result("t", 3),
            &["role", "turn_id", "tool_call_id", "text"][..],
        ),
    ] {
        // Check decoding separately from conversation pairing. A standalone
        // tool row is a valid record and requires its assistant at dispatch.
        let parses = |r| serde_json::from_value::<anix_proto::ir::IrMessage>(r).is_ok();
        for key in required {
            let mut missing = row.clone();
            missing.as_object_mut().unwrap().remove(*key);
            assert!(!parses(missing), "{} missing {key}", row["role"]);
        }
        for (value, accepted) in [
            (json!(0), true),
            (json!(u64::MAX), true),
            (json!(-1), false),
            (json!(1.5), false),
            (json!("1"), false),
            (Value::Null, false),
        ] {
            let mut changed = row.clone();
            changed["turn_id"] = value.clone();
            assert_eq!(parses(changed), accepted, "{} turn_id={value}", row["role"]);
        }
    }
}

// RFC §9.1-12 — reasoning 或 opaque 单独存在时不能替代 text 或 tool_calls。
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

// RFC §9.1-19 — 普通轮次严格递增，仅 checkpoint 特例可共享锚点。
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

// RFC §9.1-01 — 协进程在请求信封传入 model，内嵌消息不变。
#[test]
fn coprocess_envelope_wraps_the_same_ir_without_moving_model_into_history() {
    let mut head = header();
    head["max_output_tokens"] = json!(31);
    head["params"] = json!({"temperature": 0.5, "nested": {"labels":["a", "b"]}});
    head["meta"]["turn_budget"] = json!(4);
    let messages = [
        message("system", 0, "  冻结\r\n\n\t"),
        message("user", 1, "quoted \"request\"\n🦀"),
        calls(&["a", "b"]),
        result("b", 3),
        result("a", 4),
    ];
    let ir = IrStream::from_jsonl(&input(&head, &messages)).unwrap();
    ir.validate_invariants().unwrap();
    let mut expected_header = head;
    for key in ["ir_version", "type"] {
        expected_header.as_object_mut().unwrap().remove(key);
    }
    for (id, model, timeout) in [("first", "model-a", 5), ("second", "model-b", 13)] {
        let envelope = ProviderRequestEnvelope::from_stream(&ir, id, model, timeout);
        let frame = serde_json::to_string(&envelope).unwrap();
        assert!(!frame.contains(['\r', '\n']));
        let wire: Value = serde_json::from_str(&frame).unwrap();
        let expected = json!({
            "type":"request", "ir_version":1, "request_id":id, "model":model,
            "timeout_sec":timeout, "header":expected_header, "messages":messages,
        });
        assert_eq!(wire, expected);
        let decoded: ProviderRequestEnvelope = serde_json::from_value(expected).unwrap();
        assert_eq!(decoded.into_ir_stream(), ir);
    }
}
