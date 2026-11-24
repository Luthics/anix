//! Black-box conformance tests for the shipping `agent` executable using local providers.
mod configuration;
#[path = "../common/fixture.rs"]
mod fixture;
mod io;
#[path = "../common/tempdir.rs"]
mod tempdir;
use fixture::Fixture;
use tempdir::TempDir;

use serde_json::Value;
use std::fs;
use std::path::Path;
use std::process::Output;

fn jsonl(path: &Path) -> Vec<Value> {
    fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn expect_code(result: &Output, expected: i32) {
    assert_eq!(
        result.status.code(),
        Some(expected),
        "stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );
}

use serde_json::json;

fn done() -> Value {
    json!({"type":"done","finish_reason":"stop"})
}

fn tool(id: &str, cmd: &str) -> Value {
    json!({"type":"tool_call","id":id,"name":"msh","arguments":{"cmd":cmd}})
}

/// Each invocation consumes a numbered reply and records the actual request.
/// Exhausting the fixture is an error, so unexpected retries or extra turns are visible.
fn scripted_provider(fx: &Fixture, turns: &[Vec<Value>]) {
    for (idx, events) in turns.iter().enumerate() {
        let mut body = String::new();
        for event in events {
            body.push_str(&event.to_string());
            body.push('\n');
        }
        fs::write(fx.dir.0.join(format!("reply-{}", idx + 1)), body).unwrap();
    }
    fs::write(
        fx.dir.0.join("record-provider"),
        r#"#!/bin/sh
set -eu
n=0
if [ -f "$HOME/count" ]; then read -r n < "$HOME/count"; fi
n=$((n + 1))
printf '%s\n' "$n" > "$HOME/count"
/bin/cat > "$HOME/request-$n"
/bin/cat "$HOME/reply-$n"
"#,
    )
    .unwrap();
}

fn success_text(text: &str) -> Vec<Value> {
    vec![json!({"type":"text_delta","text":text}), done()]
}

fn with_tools(calls: impl IntoIterator<Item = Value>) -> Vec<Value> {
    calls
        .into_iter()
        .chain([json!({"type":"done","finish_reason":"tool_calls"})])
        .collect()
}
mod compaction;
mod doctor;
mod lifecycle;
mod signals;
mod terminal;
mod trace;
mod wal;
