//! §§6.1 and 7.2. Read the actual bytes written through an inherited fd 6.
//! stdout is intentionally discarded so a tool printing JSON to stdout cannot pass.
use image::{DynamicImage, ImageFormat};
use serde_json::{Value, json};
use std::fs;
use std::io::{Cursor, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Output, Stdio};

fn invoke(tool: &str, args: &[&str], input: &[u8]) -> Output {
    let mut child = Command::new("/bin/sh")
        .env_clear()
        .env("AGENT_CTL", "/dev/fd/6")
        .args(["-c", r#"exec 6>&1; exec "$@" >/dev/null"#, "fixture", tool])
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}

fn frame(result: &Output) -> Value {
    assert!(
        result.status.success(),
        "status {}: {}",
        result.status,
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stdout.ends_with(b"\n"));
    assert_eq!(result.stdout.iter().filter(|b| **b == b'\n').count(), 1);
    serde_json::from_slice(&result.stdout).unwrap()
}

#[test]
fn aclr_emits_a_single_compact_or_fold_frame_with_lossless_json_escaping() {
    let bin = env!("CARGO_BIN_EXE_aclr");
    assert_eq!(frame(&invoke(bin, &["-s"], b"")), json!({"op":"compact"}));
    for summary in [
        "space and\t tab",
        "first\nsecond",
        "quote \" and slash \\",
        "总结 🦀",
    ] {
        assert_eq!(
            frame(&invoke(bin, &["-t", "3", summary], b"")),
            json!({"op":"fold","from_turn":3,"summary":summary})
        );
    }
}
#[test]
fn invalid_aclr_argv_never_submits_a_frame() {
    for args in [
        &[][..],
        &["-t"],
        &["-t", "2"],
        &["-t", "2", ""],
        &["-t", "0", "s"],
        &["-t", "-1", "s"],
        &["-t", "18446744073709551616", "s"],
        &["-s", "unexpected"],
    ] {
        let result = invoke(env!("CARGO_BIN_EXE_aclr"), args, b"");
        assert_eq!(result.status.code(), Some(2), "{args:?}");
        assert!(
            result.stdout.is_empty() && !result.stderr.is_empty(),
            "{args:?}"
        );
    }
}
#[test]
fn fold_control_frames_stay_within_64_kib_even_after_json_escaping() {
    // Leave a margin on either side: both interpretations of whether the final LF
    // counts agree here. Use many short argv words to stay under OS per-argument limits.
    for (words, succeeds) in [(900, true), (1100, false)] {
        let word = "\\".repeat(32); // Encodes to 64 bytes, excluding separators.
        let mut args = vec!["-t", "1"];
        args.extend(std::iter::repeat_n(word.as_str(), words));
        let result = invoke(env!("CARGO_BIN_EXE_aclr"), &args, b"");
        if succeeds {
            let event = frame(&result);
            assert_eq!(event["summary"], vec![word; words].join(" "));
            assert!(result.stdout.len() <= 65_536);
        } else {
            assert!(!result.status.success());
            assert!(result.stdout.is_empty());
        }
    }
}

fn png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    DynamicImage::new_rgb8(width, height)
        .write_to(&mut bytes, ImageFormat::Png)
        .unwrap();
    bytes.into_inner()
}

fn take_image(record: &Value) -> (u32, u32) {
    assert_eq!(record["op"], "media");
    assert_eq!(record["mime"], "image/png");
    let path = Path::new(record["path"].as_str().unwrap());
    assert_eq!(path.parent(), Some(Path::new("/tmp")));
    assert_eq!(path.extension().unwrap(), "png");
    uuid::Uuid::parse_str(
        path.file_stem()
            .unwrap()
            .to_str()
            .unwrap()
            .strip_prefix("aview_")
            .unwrap(),
    )
    .unwrap();
    let meta = fs::symlink_metadata(path).unwrap();
    assert!(meta.is_file());
    assert_eq!(meta.permissions().mode() & 0o777, 0o600);
    let bytes = fs::read(path).unwrap();
    fs::remove_file(path).unwrap();
    let im = image::load_from_memory_with_format(&bytes, ImageFormat::Png).unwrap();
    (im.width(), im.height())
}

#[test]
fn aview_resizes_portrait_landscape_and_square_images_only_when_needed() {
    for (w, h, limit, expected) in [
        (32, 16, 16, (16, 8)),
        (16, 32, 16, (8, 16)),
        (6, 6, 5, (5, 5)),
        (4, 3, 4, (4, 3)),
        (4, 3, 5, (4, 3)),
        (3136, 2, 1568, (1568, 1)),
    ] {
        let image = png(w, h);
        let limit_arg = limit.to_string();
        let args = if limit == 1568 {
            vec!["-"]
        } else {
            vec!["--max-dim", &limit_arg, "-"]
        };
        let event = frame(&invoke(env!("CARGO_BIN_EXE_aview"), &args, &image));
        assert_eq!(take_image(&event), expected);
    }
}

#[test]
fn aview_file_input_and_stdin_agree_on_image_dimensions() {
    let bytes = png(5, 3);
    let path = std::env::temp_dir().join(format!("os-agent-image-{}.png", uuid::Uuid::new_v4()));
    fs::write(&path, &bytes).unwrap();
    let file_result = invoke(env!("CARGO_BIN_EXE_aview"), &[path.to_str().unwrap()], b"");
    fs::remove_file(&path).unwrap();
    assert_eq!(take_image(&frame(&file_result)), (5, 3));
    assert_eq!(
        take_image(&frame(&invoke(env!("CARGO_BIN_EXE_aview"), &[], &bytes))),
        (5, 3)
    );
}

#[test]
fn undecodable_input_or_invalid_options_do_not_emit_media() {
    for (args, input) in [
        (&["-"][..], &b"not an image"[..]),
        (&["--max-dim", "0", "-"], b""),
        (&["--pages", "0", "-"], b"%PDF-1.4\n"),
    ] {
        let out = invoke(env!("CARGO_BIN_EXE_aview"), args, input);
        assert!(!out.status.success() && out.stdout.is_empty());
        assert!(!out.stderr.is_empty());
    }
}
