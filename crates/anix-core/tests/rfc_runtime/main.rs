//! RFC v1.1 runtime conformance through the installed CLI contract and local providers.

mod background;
mod compaction;
mod configuration;
mod control;
mod doctor;
mod fd;
#[path = "../common/fixture.rs"]
mod fixture;
mod hooks;
mod inbox;
mod io;
mod provider;
mod session;
mod signals;
#[path = "../common/tempdir.rs"]
mod tempdir;
mod trace;
mod vision;
mod wal;
mod wal_replay;

use fixture::Fixture;
use tempdir::TempDir;

use serde_json::{Value, json};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Output;

const SINGLE_PIXEL_GIF: &[u8] =
    b"GIF89a\x01\0\x01\0\x80\0\0\0\0\0\xff\xff\xff,\0\0\0\0\x01\0\x01\0\0\x02\x02D\x01\0;";

fn jsonl(path: &Path) -> Vec<Value> {
    let data = fs::read_to_string(path).unwrap();
    assert!(data.ends_with('\n'), "incomplete final record in {path:?}");
    data.lines()
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

fn executable(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}
