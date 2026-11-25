use super::{
    Fixture, executable, expect_code, jsonl, scripted_provider, success_text, tool, with_tools,
};
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

// RFC v1.1 §3.1, §3.3, §3.4, §5.5, §6.2, §8.2
#[test]
fn vision_resolves_from_cli_env_script_selected_provider_then_defaults() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    let path = root.join("config/anix/config.toml");
    let base = fs::read_to_string(&path).unwrap();
    // Each source can explicitly win with either boolean. For every nonempty
    // subset, give all lower-priority sources the opposite value.
    for mask in 0u32..32 {
        let highest = (0..5).rev().find(|i| mask & (1 << i) != 0);
        for expected in [true, false] {
            if highest.is_none() && !expected {
                continue;
            }
            let setting = |layer: u32| {
                (mask & (1u32 << layer) != 0).then_some(if Some(layer) == highest {
                    expected
                } else {
                    !expected
                })
            };
            let mut config = base.clone();
            for (i, section) in [(0, "[defaults]"), (1, "[providers.fixture]")] {
                if let Some(value) = setting(i) {
                    config = config.replace(section, &format!("{section}\nvision={value}"));
                }
            }
            // An unselected provider never contributes its vision setting.
            config.push_str(&format!("\n[providers.other]\nvision={}\n", !expected));
            fs::write(&path, config).unwrap();
            let body = match setting(2) {
                Some(value) => format!("---\nvision: {value}\n---\nfixture"),
                None => "fixture".to_owned(),
            };
            fs::write(root.join("case.agent"), body).unwrap();
            let mut cmd = fx.anix();
            if let Some(value) = setting(3) {
                let flag = match (value, mask % 2 == 0) {
                    (true, true) => "1",
                    (true, false) => "true",
                    (false, true) => "0",
                    (false, false) => "false",
                };
                cmd.env("ANIX_VISION", flag);
            }
            if let Some(value) = setting(4) {
                cmd.arg(if value { "--vision" } else { "--no-vision" });
            }
            let wal = root.join(format!("history-{mask}-{expected}"));
            let out = fx.run(
                cmd.args(["-q", "-c"])
                    .arg(&wal)
                    .args(["case.agent", "--", "go"]),
            );
            expect_code(&out, 0);
            let rows = jsonl(&wal);
            assert_eq!(rows[0]["type"], "session_init");
            assert_eq!(rows[0]["config"]["vision"], expected, "mask={mask:05b}");
            assert_eq!(rows.last().unwrap()["text"], "ok");
        }
    }
}

const SOURCE: &str = "两帧 截图.gif";
const DESCRIPTION: &str = "第一帧 · green\n第二帧 · blue";

fn media_fixture(bridge: Option<(&str, u64)>) -> Fixture {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    let path = root.canonicalize().unwrap().join("local image.gif");
    fs::write(&path, super::SINGLE_PIXEL_GIF).unwrap();
    let frame = json!({"op":"media","mime":"image/gif","path":path,"source":SOURCE});
    fs::write(root.join("frame.jsonl"), format!("{frame}\n")).unwrap();
    if let Some((body, timeout)) = bridge {
        let config = root.join("config/anix/config.toml");
        let original = fs::read_to_string(&config).unwrap();
        fs::write(
            &config,
            format!("{original}\n[vision]\nbin='local-bridge'\ntimeout_sec={timeout}\n"),
        )
        .unwrap();
        executable(&root.join("local-bridge"), body);
    }
    fs::write(root.join("case.agent"), "media fixture").unwrap();
    scripted_provider(
        &fx,
        &[
            with_tools([tool(
                "image-call",
                r#"/bin/cat frame.jsonl > "$ANIX_CTL"; printf tool-note"#,
            )]),
            success_text("first result accepted"),
            success_text("restored"),
        ],
    );
    fx
}

fn run_media(fx: &Fixture, vision: bool, arg: &str) {
    let flag = if vision { "--vision" } else { "--no-vision" };
    let result = fx.run(fx.anix().args([
        "-q",
        "-y",
        "--shell",
        "/bin/sh",
        flag,
        "-c",
        "history",
        "case.agent",
        "--",
        arg,
    ]));
    expect_code(&result, 0);
    assert!(result.stdout.is_empty() && result.stderr.is_empty());
}

fn tool_message(path: &Path) -> Value {
    let rows: Vec<_> = jsonl(path)
        .into_iter()
        .filter(|r| r["role"] == "tool")
        .collect();
    assert_eq!(rows.len(), 1, "{}", path.display());
    rows.into_iter().next().unwrap()
}

// Compare the tool message the provider received immediately, its WAL record,
// and the reconstructed message sent by a fresh anix process.
fn round_trip(fx: &Fixture, vision: bool) -> Value {
    run_media(fx, vision, "first");
    let root = &fx.dir.0;
    let ir = tool_message(&root.join("request-2"));
    let wal = tool_message(&root.join("history"));
    for key in [
        "turn_id",
        "role",
        "text",
        "is_error",
        "tool_call_id",
        "media",
    ] {
        assert_eq!(wal.get(key), ir.get(key), "{key}");
    }
    assert_eq!(ir["tool_call_id"], "image-call");
    assert!(ir["text"].as_str().unwrap().contains("tool-note"));
    run_media(fx, vision, "resume");
    assert_eq!(tool_message(&root.join("request-3")), ir);
    assert_eq!(tool_message(&root.join("history")), wal);
    ir
}

// RFC v1.1 §5.5, §6.2, §7.2, §9.1
#[test]
fn native_vision_keeps_the_attachment_through_resume_and_never_calls_a_bridge() {
    let fx = media_fixture(Some((
        "#!/bin/sh\nprintf unexpected > bridge-called\nexit 1\n",
        10,
    )));
    let ir = round_trip(&fx, true);
    assert!(!ir["is_error"].as_bool().unwrap_or(false));
    assert_eq!(
        ir["media"],
        json!([{
            "mime":"image/gif","path":fx.dir.0.canonicalize().unwrap().join("local image.gif")
        }])
    );
    assert!(!fx.dir.0.join("bridge-called").exists());
}

// RFC v1.1 §5.5, §6.2, §7.2, §9.1
#[test]
fn no_native_vision_without_a_bridge_returns_the_documented_error_through_resume() {
    let fx = media_fixture(None);
    let ir = round_trip(&fx, false);
    assert_eq!(ir["is_error"], true);
    assert!(ir.get("media").is_none());
    assert!(ir["text"].as_str().unwrap().contains("Error: current model does not support native vision (vision=false). Use text tools (e.g. OCR/pdftotext) or delegate to a vision-capable sub-agent."));
}

// RFC v1.1 §5.5, §6.2, §7.2, §9.1
#[test]
fn bridge_gets_one_utf8_json_line_then_eof_and_its_description_survives_resume() {
    let fx = media_fixture(Some((
        "#!/bin/sh\n/bin/cat > bridge-request\nprintf '第一帧 · green\\n第二帧 · blue'\n",
        10,
    )));
    let ir = round_trip(&fx, false);
    let root = fx.dir.0.canonicalize().unwrap();
    assert!(!ir["is_error"].as_bool().unwrap_or(false), "{ir}");
    let input = jsonl(&root.join("bridge-request"));
    assert_eq!(input.len(), 1);
    for (key, expected) in [
        ("v", json!(1)),
        ("mime", json!("image/gif")),
        ("path", json!(root.join("local image.gif"))),
        ("source", json!(SOURCE)),
        ("cwd", json!(root)),
    ] {
        assert_eq!(input[0][key], expected, "{key}");
    }
    assert!(ir.get("media").is_none());
    assert!(ir["text"].as_str().unwrap().contains(&format!(
        "<vision_description source=\"{SOURCE}\">{DESCRIPTION}</vision_description>"
    )));
}

// RFC v1.1 §5.5, §6.2, §7.2, §9.1
#[test]
fn bridge_nonzero_exit_and_timeout_both_return_stderr_as_a_recoverable_tool_error() {
    for (termination, timeout) in [("exit 17", 10), ("exec /bin/sleep 15", 4)] {
        let bridge =
            format!("#!/bin/sh\n/bin/cat >/dev/null\nprintf '转换失败' >&2\n{termination}\n");
        let fx = media_fixture(Some((&bridge, timeout)));
        let ir = round_trip(&fx, false);
        assert_eq!(ir["is_error"], true, "{termination}");
        assert!(ir.get("media").is_none());
        let text = ir["text"].as_str().unwrap();
        assert!(text.contains("转换失败"), "{termination}: {text}");
        assert!(
            !text.contains("<vision_description"),
            "{termination}: {text}"
        );
    }
}
