//! End-to-end integration tests for `anix-provider` across all built-in dialects
//! (`openai-responses`, `openai-compat`, `anthropic`), verifying BusyBox `argv[0]`
//! symlink dispatch against a local loopback mock HTTP server (zero external network calls).

use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use anix_proto::IR_VERSION;
use anix_proto::ir::{
    HeaderTag, IrHeader, IrMessage, IrMeta, IrStream, MediaAttachment, StderrMode,
};
use anix_proto::provider::{ERROR_CODE_UNSUPPORTED_MEDIA, ProviderEvent, ProviderRequestEnvelope};
use serde_json::{Value, json};

fn provider_bin() -> PathBuf {
    let compile_time = PathBuf::from(env!("CARGO_BIN_EXE_anix-provider"));
    if compile_time.exists() {
        return compile_time;
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        for candidate in [dir.join("anix-provider"), dir.join("../bin/anix-provider")] {
            if candidate.exists() {
                return candidate;
            }
        }
    }
    compile_time
}

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone)]
struct CapturedRequest {
    request_line: String,
    headers: BTreeMap<String, String>,
    body: Value,
}

struct TempSymlinkDir {
    dir: PathBuf,
}

impl TempSymlinkDir {
    fn new() -> Self {
        let id = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("os-anix-providers-e2e-{}-{id}", std::process::id()));
        fs::create_dir_all(&dir).expect("create temp symlink dir");
        Self { dir }
    }

    fn symlink_as(&self, name: &str) -> PathBuf {
        let link_path = self.dir.join(name);
        symlink(provider_bin(), &link_path).expect("create busybox symlink");
        link_path
    }
}

impl Drop for TempSymlinkDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn spawn_mock_server(
    status_line: &'static str,
    content_type: &'static str,
    response_body: String,
) -> (
    String,
    Arc<Mutex<Option<CapturedRequest>>>,
    thread::JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let addr = listener.local_addr().expect("local addr");
    let captured = Arc::new(Mutex::new(None));
    let cap_clone = Arc::clone(&captured);

    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));

        let mut request_line = String::new();
        reader.read_line(&mut request_line).expect("read req line");

        let mut headers = BTreeMap::new();
        let mut content_length: usize = 0;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).expect("read header");
            let trimmed = line.trim_end_matches(&['\r', '\n'][..]);
            if trimmed.is_empty() {
                break;
            }
            if let Some((k, v)) = trimmed.split_once(':') {
                let key = k.trim().to_ascii_lowercase();
                let val = v.trim().to_owned();
                if key == "content-length" {
                    content_length = val.parse().unwrap_or(0);
                }
                headers.insert(key, val);
            }
        }

        let mut body_bytes = vec![0u8; content_length];
        reader.read_exact(&mut body_bytes).expect("read body");
        let body: Value = serde_json::from_slice(&body_bytes).expect("valid json body");

        *cap_clone.lock().expect("lock") = Some(CapturedRequest {
            request_line: request_line.trim().to_owned(),
            headers,
            body,
        });

        let resp = format!(
            "HTTP/1.1 {status_line}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        stream.write_all(resp.as_bytes()).expect("write response");
        let _ = stream.flush();
    });

    (format!("http://{addr}"), captured, handle)
}

fn sample_ir_jsonl() -> String {
    let stream = IrStream {
        header: IrHeader {
            ir_version: IR_VERSION,
            record_type: HeaderTag::Header,
            tools: vec![IrStream::msh_tool_def()],
            max_output_tokens: Some(512),
            params: BTreeMap::from([("temperature".into(), json!(0.1))]),
            meta: IrMeta {
                output_mode: None,
                stderr_mode: StderrMode::Auto,
                cwd: "/tmp".into(),
                turn_budget: Some(5),
                extra: BTreeMap::new(),
            },
            extra: BTreeMap::new(),
        },
        messages: vec![
            IrMessage::system(0, "You are helpful."),
            IrMessage::user(1, "Say hi"),
        ],
    };
    stream.to_jsonl().expect("serialize ir")
}

fn run_bin(
    exe: &Path,
    extra_args: &[&str],
    base_url: &str,
    api_key: &str,
    model: &str,
    stdin_payload: &str,
) -> (i32, Vec<ProviderEvent>, String) {
    let mut child = Command::new(exe)
        .args(extra_args)
        .env("ANIX_PROVIDER_BASE_URL", base_url)
        .env("ANIX_PROVIDER_API_KEY", api_key)
        .env("ANIX_PROVIDER_TIMEOUT_SEC", "10")
        .env("ANIX_MODEL", model)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn provider");

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(stdin_payload.as_bytes())
            .expect("write stdin");
    }

    let output = child.wait_with_output().expect("wait provider");
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    let stderr = String::from_utf8(output.stderr).expect("utf8 stderr");
    let events: Vec<ProviderEvent> = stdout
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("valid ProviderEvent JSONL"))
        .collect();

    (output.status.code().unwrap_or(-1), events, stderr)
}

// RFC v1.1 §2, §9.2
#[test]
fn version_flag_works_for_base_binary_and_busybox_symlinks() {
    let out = Command::new(provider_bin())
        .arg("--version")
        .output()
        .expect("run base --version");
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    assert!(stdout.contains("anix-provider"));
    assert!(stdout.contains("openai-responses,openai-compat,anthropic"));

    let tmp = TempSymlinkDir::new();
    for name in [
        "anix-provider-openai-responses",
        "anix-provider-openai-compat",
        "anix-provider-anthropic",
    ] {
        let link = tmp.symlink_as(name);
        let o = Command::new(&link)
            .arg("--version")
            .output()
            .expect("run symlink --version");
        assert!(o.status.success());
        let s = String::from_utf8(o.stdout).expect("utf8");
        assert!(s.starts_with(name), "got: {s}");
        assert!(s.contains("ir_version=1"));
    }
}

// RFC v1.1 §6.5, §9.1, §9.2
#[test]
fn unsupported_ir_version_exits_2() {
    let bad_ir = r#"{"ir_version":99,"type":"header","tools":[],"meta":{"stderr_mode":"auto","cwd":"/tmp"}}"#;
    let (code, events, _) = run_bin(
        &provider_bin(),
        &["openai-compat"],
        "http://127.0.0.1:1/v1",
        "",
        "m",
        bad_ir,
    );
    assert_eq!(code, 2);
    assert!(matches!(
        events.as_slice(),
        [ProviderEvent::Error { code, retryable: false, .. }] if code == "unsupported_ir_version"
    ));
}

// RFC v1.1 §2, §9.2
#[test]
fn busybox_symlink_openai_responses_end_to_end() {
    let sse = concat!(
        "event: response.output_text.delta\n",
        "data: {\"type\":\"response.output_text.delta\",\"delta\":\"Responses OK\"}\n\n",
        "event: response.completed\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"output\":[],\"usage\":{\"input_tokens\":10,\"output_tokens\":2}}}\n\n"
    )
    .to_owned();

    let (origin, captured, handle) = spawn_mock_server("200 OK", "text/event-stream", sse);
    let tmp = TempSymlinkDir::new();
    let link = tmp.symlink_as("anix-provider-openai-responses");

    let (code, events, _) = run_bin(
        &link,
        &[],
        &format!("{origin}/v1"),
        "sk-resp",
        "gpt-5",
        &sample_ir_jsonl(),
    );
    handle.join().expect("server");

    assert_eq!(code, 0);
    let req = captured.lock().expect("lock").clone().expect("captured");
    assert_eq!(req.request_line, "POST /v1/responses HTTP/1.1");
    assert_eq!(
        req.headers.get("authorization").map(String::as_str),
        Some("Bearer sk-resp")
    );
    assert_eq!(req.body["instructions"], "You are helpful.");
    assert!(events.contains(&ProviderEvent::TextDelta {
        text: "Responses OK".into()
    }));
}

// RFC v1.1 §2, §9.2
#[test]
fn busybox_symlink_openai_compat_end_to_end_and_unauthenticated_local() {
    let sse = concat!(
        "data: {\"id\":\"1\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Compat OK\"},\"finish_reason\":\"stop\"}]}\n\n",
        "data: {\"id\":\"1\",\"choices\":[],\"usage\":{\"prompt_tokens\":42,\"completion_tokens\":9,\"prompt_tokens_details\":{\"cached_tokens\":30}}}\n\n",
        "data: [DONE]\n\n"
    )
    .to_owned();

    let (origin, captured, handle) = spawn_mock_server("200 OK", "text/event-stream", sse);
    let tmp = TempSymlinkDir::new();
    let link = tmp.symlink_as("anix-provider-openai-compat");

    let (code, events, _) = run_bin(
        &link,
        &[],
        &format!("{origin}/v1"),
        "",
        "local-vllm",
        &sample_ir_jsonl(),
    );
    handle.join().expect("server");

    assert_eq!(code, 0);
    let req = captured.lock().expect("lock").clone().expect("captured");
    assert_eq!(req.request_line, "POST /v1/chat/completions HTTP/1.1");
    assert!(!req.headers.contains_key("authorization"));
    assert_eq!(req.body["model"], "local-vllm");
    assert_eq!(req.body["max_tokens"], 512);
    assert!(events.contains(&ProviderEvent::Usage {
        input_tokens: 42,
        output_tokens: 9,
        cached_tokens: Some(30),
        cache_write_tokens: None,
        reasoning_tokens: None,
    }));
}

// RFC v1.1 §2, §9.2
#[test]
fn busybox_symlink_anthropic_end_to_end_with_inclusive_usage_and_headers() {
    let sse = concat!(
        "event: message_start\n",
        "data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":100,\"cache_read_input_tokens\":900,\"cache_creation_input_tokens\":50,\"output_tokens\":1}}}\n\n",
        "event: content_block_start\n",
        "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
        "event: content_block_delta\n",
        "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Claude OK\"}}\n\n",
        "event: content_block_stop\n",
        "data: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
        "event: message_delta\n",
        "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":12}}\n\n",
        "event: message_stop\n",
        "data: {\"type\":\"message_stop\"}\n\n"
    )
    .to_owned();

    let (origin, captured, handle) = spawn_mock_server("200 OK", "text/event-stream", sse);
    let tmp = TempSymlinkDir::new();
    let link = tmp.symlink_as("anix-provider-anthropic");

    let (code, events, _) = run_bin(
        &link,
        &[],
        &origin,
        "sk-ant-test",
        "claude-sonnet-4-5",
        &sample_ir_jsonl(),
    );
    handle.join().expect("server");

    assert_eq!(code, 0);
    let req = captured.lock().expect("lock").clone().expect("captured");
    assert_eq!(req.request_line, "POST /v1/messages HTTP/1.1");
    assert_eq!(
        req.headers.get("x-api-key").map(String::as_str),
        Some("sk-ant-test")
    );
    assert_eq!(
        req.headers.get("anthropic-version").map(String::as_str),
        Some("2023-06-01")
    );
    assert_eq!(req.body["model"], "claude-sonnet-4-5");
    assert_eq!(req.body["max_tokens"], 512);
    assert_eq!(
        events,
        vec![
            ProviderEvent::TextDelta {
                text: "Claude OK".into()
            },
            ProviderEvent::Usage {
                input_tokens: 1050,
                output_tokens: 12,
                cached_tokens: Some(900),
                cache_write_tokens: Some(50),
                reasoning_tokens: None,
            },
            ProviderEvent::Done {
                finish_reason: "stop".into()
            },
        ]
    );
}

// RFC v1.1 §9.2
#[test]
fn http_429_and_529_map_to_retryable_errors_and_400_is_non_retryable() {
    let (o1, _, h1) = spawn_mock_server(
        "429 Too Many Requests",
        "application/json",
        r#"{"error":{"message":"slow down"}}"#.into(),
    );
    let (c1, ev1, _) = run_bin(
        &provider_bin(),
        &["openai-compat"],
        &format!("{o1}/v1"),
        "sk",
        "m",
        &sample_ir_jsonl(),
    );
    h1.join().expect("h1");
    assert_eq!(c1, 1);
    assert!(matches!(
        ev1.as_slice(),
        [ProviderEvent::Error { code, retryable: true, .. }] if code == "rate_limit"
    ));

    let (o2, _, h2) = spawn_mock_server(
        "529 Overloaded",
        "application/json",
        r#"{"type":"error","error":{"type":"overloaded_error","message":"busy"}}"#.into(),
    );
    let (c2, ev2, _) = run_bin(
        &provider_bin(),
        &["anthropic"],
        &o2,
        "sk-ant",
        "claude",
        &sample_ir_jsonl(),
    );
    h2.join().expect("h2");
    assert_eq!(c2, 1);
    assert!(matches!(
        ev2.as_slice(),
        [ProviderEvent::Error { code, retryable: true, .. }] if code == "server_error"
    ));

    let (o3, _, h3) = spawn_mock_server(
        "400 Bad Request",
        "application/json",
        r#"{"error":{"message":"invalid"}}"#.into(),
    );
    let (c3, ev3, _) = run_bin(
        &provider_bin(),
        &["openai-responses"],
        &format!("{o3}/v1"),
        "sk",
        "m",
        &sample_ir_jsonl(),
    );
    h3.join().expect("h3");
    assert_eq!(c3, 1);
    assert!(matches!(
        ev3.as_slice(),
        [ProviderEvent::Error { code, retryable: false, .. }] if code == "client_error"
    ));
}

// RFC v1.1 §9.2
#[test]
fn coprocess_mode_serves_multiple_requests_survives_error_and_exits_0_on_eof() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let addr = listener.local_addr().expect("addr");
    let captured: Arc<Mutex<Vec<CapturedRequest>>> = Arc::new(Mutex::new(Vec::new()));
    let captured_bg = Arc::clone(&captured);

    let responses = vec![
        (
            "200 OK",
            "text/event-stream",
            concat!(
                "event: response.output_text.delta\n",
                "data: {\"delta\":\"turn-one\"}\n\n",
                "event: response.completed\n",
                "data: {\"response\":{\"status\":\"completed\",\"usage\":{\"input_tokens\":10,\"output_tokens\":2}}}\n\n",
            )
            .to_owned(),
        ),
        (
            "429 Too Many Requests",
            "application/json",
            r#"{"error":{"message":"slow down"}}"#.to_owned(),
        ),
        (
            "200 OK",
            "text/event-stream",
            concat!(
                "event: response.output_text.delta\n",
                "data: {\"delta\":\"turn-two\"}\n\n",
                "event: response.completed\n",
                "data: {\"response\":{\"status\":\"completed\",\"usage\":{\"input_tokens\":20,\"output_tokens\":4}}}\n\n",
            )
            .to_owned(),
        ),
    ];

    let server = thread::spawn(move || {
        for (status, ctype, body) in responses {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut reader = BufReader::new(stream.try_clone().expect("clone"));
            let mut request_line = String::new();
            reader.read_line(&mut request_line).expect("req line");
            let mut headers = BTreeMap::new();
            let mut content_len = 0usize;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).expect("hdr");
                let t = line.trim();
                if t.is_empty() {
                    break;
                }
                if let Some((k, v)) = t.split_once(':') {
                    let kl = k.trim().to_ascii_lowercase();
                    let vv = v.trim().to_owned();
                    if kl == "content-length" {
                        content_len = vv.parse().unwrap_or(0);
                    }
                    headers.insert(kl, vv);
                }
            }
            let mut buf = vec![0u8; content_len];
            reader.read_exact(&mut buf).expect("body");
            let parsed_body: Value = serde_json::from_slice(&buf).expect("json");
            captured_bg.lock().unwrap().push(CapturedRequest {
                request_line: request_line.trim().to_owned(),
                headers,
                body: parsed_body,
            });

            let resp = format!(
                "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(resp.as_bytes()).expect("write resp");
            stream.flush().expect("flush");
        }
    });

    let stream1 = IrStream::from_jsonl(&sample_ir_jsonl()).expect("sample ir");
    let env1 = ProviderRequestEnvelope::from_stream(&stream1, "req_1", "model-a", 30);
    let env2 = ProviderRequestEnvelope::from_stream(&stream1, "req_2", "model-b", 30);
    let env3 = ProviderRequestEnvelope::from_stream(&stream1, "req_3", "model-c", 30);

    let mut child = Command::new(provider_bin())
        .arg("openai-responses")
        .env("ANIX_PROVIDER_MODE", "coprocess")
        .env("ANIX_PROVIDER_BASE_URL", format!("http://{addr}/v1"))
        .env("ANIX_PROVIDER_API_KEY", "sk-coproc")
        .env("ANIX_PROVIDER_TIMEOUT_SEC", "30")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn coprocess");

    let mut stdin = child.stdin.take().expect("stdin");
    let mut stdout = BufReader::new(child.stdout.take().expect("stdout"));

    let read_until_terminal = |out: &mut BufReader<_>| -> Vec<ProviderEvent> {
        let mut evs = Vec::new();
        loop {
            let mut line = String::new();
            let n = out.read_line(&mut line).expect("read stdout line");
            assert!(n > 0, "unexpected EOF before terminal event");
            let ev: ProviderEvent = serde_json::from_str(line.trim()).expect("parse event");
            let is_terminal =
                matches!(ev, ProviderEvent::Done { .. } | ProviderEvent::Error { .. });
            evs.push(ev);
            if is_terminal {
                break;
            }
        }
        evs
    };

    writeln!(stdin, "{}", serde_json::to_string(&env1).unwrap()).unwrap();
    stdin.flush().unwrap();
    let r1 = read_until_terminal(&mut stdout);
    assert!(
        r1.iter()
            .any(|e| matches!(e, ProviderEvent::TextDelta { text } if text == "turn-one"))
    );
    assert!(matches!(r1.last(), Some(ProviderEvent::Done { .. })));

    writeln!(stdin, "{}", serde_json::to_string(&env2).unwrap()).unwrap();
    stdin.flush().unwrap();
    let r2 = read_until_terminal(&mut stdout);
    assert!(matches!(
        r2.as_slice(),
        [ProviderEvent::Error { code, retryable: true, .. }] if code == "rate_limit"
    ));

    // Coprocess MUST remain alive after an error and serve req 3 with overridden model `model-c`.
    writeln!(stdin, "{}", serde_json::to_string(&env3).unwrap()).unwrap();
    stdin.flush().unwrap();
    let r3 = read_until_terminal(&mut stdout);
    assert!(
        r3.iter()
            .any(|e| matches!(e, ProviderEvent::TextDelta { text } if text == "turn-two"))
    );
    assert!(matches!(r3.last(), Some(ProviderEvent::Done { .. })));

    // Close stdin EOF => coprocess exits 0 cleanly.
    drop(stdin);
    let status = child.wait().expect("wait coprocess");
    server.join().expect("server");
    assert_eq!(status.code(), Some(0));

    let reqs = captured.lock().unwrap();
    assert_eq!(reqs.len(), 3);
    assert_eq!(reqs[0].body["model"], "model-a");
    assert_eq!(reqs[1].body["model"], "model-b");
    assert_eq!(reqs[2].body["model"], "model-c");
}

// RFC v1.1 §5.5, §9.2
#[test]
fn http_400_image_rejection_maps_to_unsupported_media_non_retryable() {
    let tmp = TempSymlinkDir::new();
    let img_path = tmp.dir.join("tiny.png");
    fs::write(&img_path, b"\x89PNG\r\n\x1a\nfake").expect("write dummy png");

    let stream = IrStream {
        header: IrHeader {
            ir_version: IR_VERSION,
            record_type: HeaderTag::Header,
            tools: Vec::new(),
            max_output_tokens: None,
            params: BTreeMap::new(),
            meta: IrMeta {
                output_mode: None,
                stderr_mode: StderrMode::Auto,
                cwd: "/tmp".into(),
                turn_budget: None,
                extra: BTreeMap::new(),
            },
            extra: BTreeMap::new(),
        },
        messages: vec![
            IrMessage::system(0, "sys"),
            IrMessage::User {
                turn_id: 1,
                text: "look".into(),
                media: vec![MediaAttachment {
                    mime: "image/png".into(),
                    path: img_path.display().to_string(),
                }],
                extra: BTreeMap::new(),
            },
        ],
    };

    let (origin, _, handle) = spawn_mock_server(
        "400 Bad Request",
        "application/json",
        r#"{"error":{"message":"This model does not support image inputs"}}"#.into(),
    );
    let (code, events, _) = run_bin(
        &provider_bin(),
        &["openai-compat"],
        &format!("{origin}/v1"),
        "sk",
        "text-only",
        &stream.to_jsonl().unwrap(),
    );
    handle.join().expect("server");

    assert_eq!(code, 1);
    assert!(matches!(
        events.as_slice(),
        [ProviderEvent::Error { code, retryable: false, .. }] if code == ERROR_CODE_UNSUPPORTED_MEDIA
    ));
}

// RFC v1.1.1 §9.2
#[test]
fn session_affinity_headers_and_body_mapping_across_all_dialects() {
    let make_ir = |params: BTreeMap<String, Value>| -> String {
        let stream = IrStream {
            header: IrHeader {
                ir_version: IR_VERSION,
                record_type: HeaderTag::Header,
                tools: Vec::new(),
                max_output_tokens: Some(128),
                params,
                meta: IrMeta {
                    output_mode: None,
                    stderr_mode: StderrMode::Auto,
                    cwd: "/tmp".into(),
                    turn_budget: None,
                    extra: BTreeMap::new(),
                },
                extra: BTreeMap::new(),
            },
            messages: vec![IrMessage::system(0, "sys"), IrMessage::user(1, "hello")],
        };
        stream.to_jsonl().expect("ir jsonl")
    };

    let run_with_session = |dialect: &str, base_url: &str, sid: &str, ir: &str| {
        let mut child = Command::new(provider_bin())
            .arg(dialect)
            .env("ANIX_PROVIDER_MODE", "oneshot")
            .env("ANIX_PROVIDER_BASE_URL", base_url)
            .env("ANIX_PROVIDER_API_KEY", "sk-sid")
            .env("ANIX_PROVIDER_TIMEOUT_SEC", "10")
            .env("ANIX_MODEL", "m-sid")
            .env("ANIX_SESSION_ID", sid)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn");
        child
            .stdin
            .take()
            .unwrap()
            .write_all(ir.as_bytes())
            .unwrap();
        let out = child.wait_with_output().expect("wait");
        assert_eq!(
            out.status.code(),
            Some(0),
            "stderr={}",
            String::from_utf8_lossy(&out.stderr)
        );
    };

    let resp_sse = concat!(
        "event: response.output_text.delta\n",
        "data: {\"type\":\"response.output_text.delta\",\"delta\":\"ok\"}\n\n",
        "event: response.completed\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"output\":[]}}\n\n"
    )
    .to_owned();
    let compat_sse = concat!(
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"ok\"},\"finish_reason\":\"stop\"}]}\n\n",
        "data: [DONE]\n\n"
    )
    .to_owned();
    let anth_sse = concat!(
        "event: content_block_start\n",
        "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"ok\"}}\n\n",
        "event: message_delta\n",
        "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"}}\n\n",
        "event: message_stop\n",
        "data: {\"type\":\"message_stop\"}\n\n"
    )
    .to_owned();

    // 1. openai-responses default: `x-session-id` header + `prompt_cache_key` in body.
    let (o1, cap1, h1) = spawn_mock_server("200 OK", "text/event-stream", resp_sse.clone());
    run_with_session(
        "openai-responses",
        &format!("{o1}/v1"),
        "sid-resp-1",
        &make_ir(BTreeMap::new()),
    );
    h1.join().unwrap();
    let r1 = cap1.lock().unwrap().clone().unwrap();
    assert_eq!(
        r1.headers.get("x-session-id").map(String::as_str),
        Some("sid-resp-1")
    );
    assert_eq!(r1.body["prompt_cache_key"], "sid-resp-1");

    // 2. openai-responses with opt-out `prompt_cache_key=false` + custom `session_header`.
    let (o2, cap2, h2) = spawn_mock_server("200 OK", "text/event-stream", resp_sse);
    let mut p2 = BTreeMap::new();
    p2.insert("prompt_cache_key".into(), json!(false));
    p2.insert("session_header".into(), json!("X-Conversation-Id"));
    run_with_session(
        "openai-responses",
        &format!("{o2}/v1"),
        "sid-resp-2",
        &make_ir(p2),
    );
    h2.join().unwrap();
    let r2 = cap2.lock().unwrap().clone().unwrap();
    assert!(!r2.headers.contains_key("x-session-id"));
    assert_eq!(
        r2.headers.get("x-conversation-id").map(String::as_str),
        Some("sid-resp-2")
    );
    assert!(r2.body.get("prompt_cache_key").is_none());

    // 3. openai-compat default: `x-session-id` header ONLY (no body keys to avoid 400).
    let (o3, cap3, h3) = spawn_mock_server("200 OK", "text/event-stream", compat_sse.clone());
    run_with_session(
        "openai-compat",
        &format!("{o3}/v1"),
        "sid-compat-1",
        &make_ir(BTreeMap::new()),
    );
    h3.join().unwrap();
    let r3 = cap3.lock().unwrap().clone().unwrap();
    assert_eq!(
        r3.headers.get("x-session-id").map(String::as_str),
        Some("sid-compat-1")
    );
    assert!(r3.body.get("session_id").is_none());
    assert!(r3.body.get("prompt_cache_key").is_none());

    // 4. openai-compat opt-in: `session_id = true` and `prompt_cache_key = true` bind dynamic sid.
    let (o4, cap4, h4) = spawn_mock_server("200 OK", "text/event-stream", compat_sse);
    let mut p4 = BTreeMap::new();
    p4.insert("session_id".into(), json!(true));
    p4.insert("prompt_cache_key".into(), json!(true));
    run_with_session(
        "openai-compat",
        &format!("{o4}/v1"),
        "sid-compat-2",
        &make_ir(p4),
    );
    h4.join().unwrap();
    let r4 = cap4.lock().unwrap().clone().unwrap();
    assert_eq!(r4.body["session_id"], "sid-compat-2");
    assert_eq!(r4.body["prompt_cache_key"], "sid-compat-2");

    // 5. anthropic default: `x-session-id` header only, clean body; `session_header=false` disables.
    let (o5, cap5, h5) = spawn_mock_server("200 OK", "text/event-stream", anth_sse.clone());
    run_with_session("anthropic", &o5, "sid-anth-1", &make_ir(BTreeMap::new()));
    h5.join().unwrap();
    let r5 = cap5.lock().unwrap().clone().unwrap();
    assert_eq!(
        r5.headers.get("x-session-id").map(String::as_str),
        Some("sid-anth-1")
    );
    assert!(r5.body.get("session_id").is_none());
    assert!(r5.body.get("prompt_cache_key").is_none());

    let (o6, cap6, h6) = spawn_mock_server("200 OK", "text/event-stream", anth_sse);
    let mut p6 = BTreeMap::new();
    p6.insert("session_header".into(), json!(false));
    run_with_session("anthropic", &o6, "sid-anth-2", &make_ir(p6));
    h6.join().unwrap();
    let r6 = cap6.lock().unwrap().clone().unwrap();
    assert!(!r6.headers.contains_key("x-session-id"));
}
