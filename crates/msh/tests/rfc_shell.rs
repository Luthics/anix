//! RFC §7.1. The first two boundaries are byte counts, not character counts.
use msh::{
    ansi,
    binary::{self, OutputKind},
    fold::{self, FoldLimits},
};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn take_artifact(path: &Path, extension: &str) -> Vec<u8> {
    assert_eq!(path.parent(), Some(Path::new("/tmp")));
    assert_eq!(path.extension().unwrap(), extension);
    let stem = path
        .file_stem()
        .unwrap()
        .to_str()
        .unwrap()
        .strip_prefix("msh_")
        .unwrap();
    uuid::Uuid::parse_str(stem).unwrap();
    let metadata = fs::symlink_metadata(path).unwrap();
    assert!(metadata.is_file());
    assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
    let bytes = fs::read(path).unwrap();
    fs::remove_file(path).unwrap();
    bytes
}

// RFC §7.1
#[test]
fn nul_probe_is_exactly_8192_bytes_utf8_validation_covers_the_entire_stream() {
    for (offset, byte, suppressed) in [
        (0, 0, true),
        (8190, 0, true),
        (8191, 0, true),
        (8192, 0, false),
        (8193, 0, false),
        (8191, 0xff, true),
        (8192, 0xff, true),
        (16384, 0xff, true),
    ] {
        let mut raw = vec![b'z'; offset + 2];
        raw[offset] = byte;
        match binary::inspect_or_suppress(&raw).unwrap() {
            OutputKind::Text(text) => {
                assert!(!suppressed, "byte {byte} at {offset}");
                assert_eq!(text.as_bytes(), raw);
            }
            OutputKind::BinarySuppressed { bytes, mime, path } => {
                let saved = take_artifact(&path, "bin");
                assert!(suppressed, "byte {byte} at {offset}");
                assert_eq!(bytes, raw.len());
                assert_eq!(mime, "application/octet-stream");
                assert_eq!(saved, raw);
            }
        }
    }
}

// RFC §7.1
#[test]
fn ansi_csi_final_byte_range_osc_terminators_and_carriage_return_overwrites() {
    for final_byte in 0x40..=0x7e {
        let input = format!("前\x1b[1;2{}後", char::from(final_byte));
        assert_eq!(
            ansi::clean_ansi_and_cr(&input),
            "前後",
            "CSI final byte {final_byte:#04x}"
        );
    }
    for terminator in ["\x07", "\x1b\\"] {
        let input = format!("A\x1b]8;;label{terminator}🦀\x1b]8;;{terminator}Z");
        assert_eq!(ansi::clean_ansi_and_cr(&input), "A🦀Z");
    }
    assert_eq!(
        ansi::clean_ansi_and_cr("0%\r50%\r100%\r\nnext\n"),
        "100%\nnext\n"
    );
    assert_eq!(ansi::clean_ansi_and_cr("  作業\n\n  "), "  作業\n\n  ");
}

fn folded_file(summary: &str) -> PathBuf {
    let (_, tail) = summary.split_once("/tmp/msh_").unwrap();
    let (id, _) = tail.split_once(".log").unwrap();
    PathBuf::from(format!("/tmp/msh_{id}.log"))
}

// RFC §7.1
#[test]
fn line_limit_is_inclusive_and_fold_preserves_exact_head_tail_count_and_full_text() {
    for count in [199, 200, 201, 403] {
        let text = (0..count)
            .map(|i| format!("row-{i:04}\n"))
            .collect::<String>();
        let summary = fold::fold_if_needed(
            &text,
            37,
            FoldLimits {
                max_lines: 200,
                max_bytes: 8192,
            },
        )
        .unwrap();
        if count <= 200 {
            assert_eq!(summary, text);
            continue;
        }
        assert_eq!(
            take_artifact(&folded_file(&summary), "log"),
            text.as_bytes()
        );
        let visible: Vec<_> = summary
            .lines()
            .filter(|line| line.starts_with("row-"))
            .collect();
        let expected: Vec<_> = text
            .lines()
            .take(20)
            .chain(text.lines().skip(count - 20))
            .collect();
        assert_eq!(visible, expected);
        assert!(
            summary.contains(&(count - 40).to_string()) && summary.contains("37"),
            "{summary}"
        );
    }
}

// RFC §7.1
#[test]
fn byte_limit_independent_from_line_limit_preserves_multibyte_file() {
    for size in [8191, 8192, 8193] {
        let text = "界".repeat(size / 3) + &"x".repeat(size % 3);
        let summary = fold::fold_if_needed(
            &text,
            0,
            FoldLimits {
                max_lines: 200,
                max_bytes: 8192,
            },
        )
        .unwrap();
        if size <= 8192 {
            assert_eq!(summary, text);
        } else {
            assert_eq!(
                take_artifact(&folded_file(&summary), "log"),
                text.as_bytes()
            );
            assert!(!summary.contains('\u{fffd}'));
        }
    }
}

fn msh(script: &str) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_msh"));
    c.env_clear()
        .env("PATH", "/usr/bin:/bin")
        .stdin(Stdio::null())
        .args(["-c", script]);
    c
}

// RFC §7.1
#[test]
fn cli_forces_noninteractive_environment_merges_streams_and_propagates_status() {
    let mut cmd = msh(
        r#"printf '%s|%s|%s|%s|%s|%s' "$NONINTERACTIVE" "$DEBIAN_FRONTEND" "$PAGER" "$GIT_TERMINAL_PROMPT" "$TERM" "$NO_COLOR"; printf '|stderr' >&2; exit 37"#,
    );
    for key in [
        "NONINTERACTIVE",
        "DEBIAN_FRONTEND",
        "PAGER",
        "GIT_TERMINAL_PROMPT",
        "TERM",
        "NO_COLOR",
    ] {
        cmd.env(key, "wrong");
    }
    let res = cmd.output().unwrap();
    assert_eq!(res.status.code(), Some(37));
    assert_eq!(res.stdout, b"1|noninteractive|cat|0|dumb|1|stderr");
    assert!(res.stderr.is_empty());
}

// RFC §7.1
#[test]
fn cli_suppresses_raw_bytes_before_ansi_filtering_or_folding() {
    let out = msh(r#"printf '\211PNG\r\n\032\n\033[31m\000'"#)
        .env("MSH_MAX_BYTES", "1")
        .output()
        .unwrap();
    assert!(out.status.success());
    let line = std::str::from_utf8(&out.stdout).unwrap();
    assert_eq!(line.lines().count(), 1);
    let (_, tail) = line.split_once("saved to ").unwrap();
    let (path, _) = tail.split_once(".bin. ").unwrap();
    let path = PathBuf::from(format!("{path}.bin"));
    let bytes = take_artifact(&path, "bin");
    assert_eq!(bytes, b"\x89PNG\r\n\x1a\n\x1b[31m\0");
    assert_eq!(
        line,
        format!(
            "[binary output suppressed: {}B, mime: image/png, saved to {}. Use xxd/strings/pdftotext or aview to inspect]\n",
            bytes.len(),
            path.display()
        )
    );
}

// RFC §7.1, §6.4
#[test]
fn timeout_reports_124_before_a_finite_longer_command_completes() {
    let start = Instant::now();
    let result = msh("exec /bin/sleep 20")
        .env("MSH_TIMEOUT_SEC", "1")
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(124));
    assert!(start.elapsed() < Duration::from_secs(18));
    assert!(!result.stdout.is_empty());
}
