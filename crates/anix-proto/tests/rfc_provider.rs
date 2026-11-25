//! Wire values are supplied independently of the production serializer.
use anix_proto::provider::ProviderEvent;
use serde_json::{Value, json};

// RFC §9.2-12 — 逐个覆盖五项计数器的 uint64 边界并拒绝非整数。
#[test]
fn each_usage_counter_accepts_the_full_uint64_range_and_requires_integer_values() {
    let counts = [
        "input_tokens",
        "output_tokens",
        "cached_tokens",
        "cache_write_tokens",
        "reasoning_tokens",
    ];
    for field in counts {
        for literal in [
            "0",
            "1",
            "4294967296",
            "9007199254740993",
            "9223372036854775808",
            "18446744073709551615",
        ] {
            let n = literal.parse::<u64>().unwrap();
            let mut value =
                json!({"type":"usage","input_tokens":u64::MAX,"output_tokens":u64::MAX});
            value[field] = json!(n);
            let event: ProviderEvent = serde_json::from_str(&value.to_string()).unwrap();
            let decoded = serde_json::to_value(event).unwrap();
            assert_eq!(decoded[field].as_u64(), Some(n), "{field}={literal}");
        }
        // Insert the numerical literal as text so MAX+1 reaches the parser
        // exactly. Parsing the fixture via a float first would round it.
        for invalid in [
            "-1",
            "0.5",
            "18446744073709551616",
            "\"1\"",
            "true",
            "[]",
            "{}",
        ] {
            let mut value = json!({"type":"usage","input_tokens":1,"output_tokens":1});
            value[field] = json!("COUNT");
            let wire = value.to_string().replace("\"COUNT\"", invalid);
            assert!(
                serde_json::from_str::<ProviderEvent>(&wire).is_err(),
                "{wire}"
            );
        }
    }
}

// RFC §9.2-12 — 即使三个明细齐全也不能缺少 input_tokens 或 output_tokens。
#[test]
fn usage_requires_both_totals_even_when_all_three_details_are_present() {
    let full = json!({"type":"usage","input_tokens":19,"output_tokens":7,
                      "cached_tokens":3,"cache_write_tokens":2,"reasoning_tokens":1});
    for field in ["input_tokens", "output_tokens"] {
        let mut missing = full.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(
            serde_json::from_value::<ProviderEvent>(missing).is_err(),
            "missing {field}"
        );
        let mut null = full.clone();
        null[field] = Value::Null;
        assert!(
            serde_json::from_value::<ProviderEvent>(null).is_err(),
            "null {field}"
        );
    }
}
