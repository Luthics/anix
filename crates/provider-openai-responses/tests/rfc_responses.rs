//! §§9.1–9.2. Exercise stdin, the actual loopback HTTP request, SSE, stdout,
//! stderr and the process exit status. No upstream account or network is used.
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

fn head() -> Value {
    json!({"type":"header","ir_version":1,"meta":{"cwd":"/","stderr_mode":"explicit"},"tools":[]})
}
fn prompt() -> Value {
    json!({"role":"system","turn_id":0,"text":"  冻结\n原文\t"})
}
fn request(stream: &mut TcpStream) -> Value {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    assert_eq!(line.trim_end(), "POST /v1/responses HTTP/1.1");
    let mut length = None;
    let mut chunked = false;
    loop {
        line.clear();
        assert_ne!(reader.read_line(&mut line).unwrap(), 0);
        if line == "\r\n" {
            break;
        }
        let (key, value) = line.split_once(':').unwrap();
        if key.eq_ignore_ascii_case("content-length") {
            length = Some(value.trim().parse::<usize>().unwrap());
        }
        if key.eq_ignore_ascii_case("transfer-encoding") {
            chunked = value.trim().eq_ignore_ascii_case("chunked");
        }
    }
    let mut bytes = Vec::new();
    if chunked {
        loop {
            line.clear();
            reader.read_line(&mut line).unwrap();
            let size = usize::from_str_radix(line.trim().split(';').next().unwrap(), 16).unwrap();
            if size == 0 {
                break;
            }
            let start = bytes.len();
            bytes.resize(start + size, 0);
            reader.read_exact(&mut bytes[start..]).unwrap();
            let mut crlf = [0; 2];
            reader.read_exact(&mut crlf).unwrap();
            assert_eq!(&crlf, b"\r\n");
        }
    } else {
        bytes.resize(length.unwrap(), 0);
        reader.read_exact(&mut bytes).unwrap();
    }
    serde_json::from_slice(&bytes).unwrap()
}

fn run(rows: &[Value], status: u16, body: &str) -> (Vec<Value>, Output, Vec<Value>) {
    let server = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    server.set_nonblocking(true).unwrap();
    let url = format!("http://{}/v1", server.local_addr().unwrap());
    let mut child = Command::new(env!("CARGO_BIN_EXE_agent-provider-openai-responses"))
        .env_clear()
        .env("AGENT_PROVIDER_BASE_URL", url)
        .env("AGENT_MODEL", "fixture")
        .env("AGENT_PROVIDER_API_KEY", "local-test-only")
        .env("AGENT_PROVIDER_TIMEOUT_SEC", "8")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    for row in rows {
        writeln!(input, "{row}").unwrap();
    }
    drop(input);
    let end = Instant::now() + Duration::from_secs(15);
    let mut requests = Vec::new();
    loop {
        match server.accept() {
            Ok((mut stream, _)) => {
                stream.set_nonblocking(false).unwrap();
                requests.push(request(&mut stream));
                write!(stream,"HTTP/1.1 {status} Fixture\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(e) => panic!("fixture accept: {e}"),
        }
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if Instant::now() >= end {
            let _ = child.kill();
            let _ = child.wait();
            panic!("adapter exceeded deadline");
        }
        thread::sleep(Duration::from_millis(5));
    }
    let output = child.wait_with_output().unwrap();
    let events = std::str::from_utf8(&output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    (requests, output, events)
}

fn sse(events: &[Value]) -> String {
    events
        .iter()
        .map(|v| {
            format!(
                "event: {}\r\ndata: {v}\r\n\r\n",
                v["type"].as_str().unwrap()
            )
        })
        .collect()
}

#[test]
fn maps_request_and_streams_text_reasoning_usage_and_done() {
    let mut h = head();
    h["max_output_tokens"] = json!(17);
    h["params"] = json!({"temperature":0.25,"unsupported-key":"ignore"});
    let response = sse(&[
        json!({"type":"response.reasoning_summary_text.delta","delta":"思"}),
        json!({"type":"response.output_text.delta","delta":" a"}),
        json!({"type":"response.output_text.delta","delta":"\n好"}),
        json!({"type":"response.completed","response":{"usage":{"input_tokens":12,"output_tokens":4,"input_tokens_details":{"cached_tokens":7}}}}),
    ]);
    let (requests, out, events) = run(
        &[
            h,
            prompt(),
            json!({"role":"user","turn_id":1,"text":"input"}),
        ],
        200,
        &response,
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(requests.len(), 1);
    let req = &requests[0];
    assert_eq!(req["instructions"], prompt()["text"]);
    assert_eq!(req["model"], "fixture");
    assert_eq!(req["input"][0]["content"][0]["text"], "input");
    assert_eq!(req["max_output_tokens"], 17);
    assert!(req.get("tools").is_none());
    assert_eq!(events.len(), 5);
    assert_eq!(events[0], json!({"type":"reasoning_delta","text":"思"}));
    assert_eq!(events[1], json!({"type":"text_delta","text":" a"}));
    assert_eq!(events[2], json!({"type":"text_delta","text":"\n好"}));
    assert_eq!(events[3]["input_tokens"], 12);
    assert_eq!(events[3]["output_tokens"], 4);
    assert_eq!(events[3]["cached_tokens"], 7);
    assert_eq!(events[4], json!({"type":"done","finish_reason":"stop"}));
}

#[test]
fn repeated_sse_notifications_emit_one_complete_tool_call() {
    let item = json!({"id":"f1","call_id":"c1","type":"function_call","name":"msh","arguments":"{\"cmd\":\"pwd\"}"});
    let mut start = item.clone();
    start["arguments"] = json!("");
    let response = sse(&[
        json!({"type":"response.output_item.added","item":start}),
        json!({"type":"response.function_call_arguments.delta","item_id":"f1","delta":"{\"cmd\":"}),
        json!({"type":"response.function_call_arguments.delta","item_id":"f1","delta":"\"pwd\"}"}),
        json!({"type":"response.function_call_arguments.done","item_id":"f1","arguments":"{\"cmd\":\"pwd\"}"}),
        json!({"type":"response.output_item.done","item":item}),
        json!({"type":"response.completed","response":{"output":[item]}}),
    ]);
    let mut h = head();
    h["tools"] = json!([{"name":"msh","description":"shell","parameters":{"type":"object"}}]);
    let (requests, out, events) = run(&[h, prompt()], 200, &response);
    assert!(out.status.success());
    assert_eq!(requests[0]["tools"][0]["name"], "msh");
    assert_eq!(
        events,
        [
            json!({"type":"tool_call","id":"c1","name":"msh","arguments":{"cmd":"pwd"}}),
            json!({"type":"done","finish_reason":"tool_calls"})
        ]
    );
}

#[test]
fn transient_http_errors_are_retryable_but_adapter_never_retries_itself() {
    for status in [429, 500, 502, 503] {
        let (requests, out, events) = run(&[head(), prompt()], status, "service unavailable");
        assert_eq!(requests.len(), 1, "HTTP {status}");
        assert!(!out.status.success());
        assert!(
            events
                .iter()
                .any(|e| e["type"] == "error" && e["retryable"] == true)
        );
        assert!(events.iter().all(|e| e["type"] != "done"));
    }
}

#[test]
fn missing_terminal_event_is_not_reported_as_success() {
    let (requests, out, events) = run(
        &[head(), prompt()],
        200,
        &sse(&[json!({"type":"response.output_text.delta","delta":"partial"})]),
    );
    assert_eq!(requests.len(), 1);
    assert!(!out.status.success());
    assert!(!out.stderr.is_empty());
    assert!(
        events
            .iter()
            .any(|e| e["type"] == "text_delta" && e["text"] == "partial")
    );
    assert!(events.iter().any(|e| e["type"] == "error"));
    assert!(events.iter().all(|e| e["type"] != "done"));
}

#[test]
fn unsupported_ir_version_and_missing_media_fail_before_http() {
    let mut bad = head();
    bad["ir_version"] = json!(2);
    let missing =
        std::env::temp_dir().join(format!("absent-os-agent-{}-image.png", std::process::id()));
    assert!(!missing.exists());
    for (rows, expected) in [
        (vec![bad, prompt()], 2),
        (
            vec![
                head(),
                prompt(),
                json!({"role":"user","turn_id":1,"text":"","media":[{"mime":"image/png","path":missing}]}),
            ],
            1,
        ),
    ] {
        let (requests, out, events) = run(&rows, 200, "");
        assert!(requests.is_empty());
        assert_eq!(out.status.code(), Some(expected));
        assert!(!out.stderr.is_empty());
        if expected == 1 {
            assert!(events.iter().any(|e| e["type"] == "error"
                && e["code"] == "media_not_found"
                && e["retryable"] == false));
        }
    }
}
