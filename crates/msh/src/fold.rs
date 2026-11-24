//! Output folding (`MSH_MAX_LINES=200`, `MSH_MAX_BYTES=8192`) (Section 7.1.4).
//!
//! When cleaned output exceeds either threshold, full text is persisted to
//! `/tmp/msh_<uuid>.log` (mode `0600`) and replaced with `exit_code`, head 20
//! lines, omitted statistics, tail 20 lines, and full log path.

use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;

use uuid::Uuid;

pub const DEFAULT_MAX_LINES: usize = 200;
pub const DEFAULT_MAX_BYTES: usize = 8192;
const HEAD_LINES: usize = 20;
const TAIL_LINES: usize = 20;

/// Effective folding thresholds resolved from environment variables.
#[derive(Debug, Clone, Copy)]
pub struct FoldLimits {
    pub max_lines: usize,
    pub max_bytes: usize,
}

impl FoldLimits {
    pub fn from_env() -> Self {
        let max_lines = std::env::var("MSH_MAX_LINES")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|&n| n > 0)
            .unwrap_or(DEFAULT_MAX_LINES);
        let max_bytes = std::env::var("MSH_MAX_BYTES")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|&n| n > 0)
            .unwrap_or(DEFAULT_MAX_BYTES);
        Self {
            max_lines,
            max_bytes,
        }
    }
}

/// Fold `cleaned` if it exceeds `limits.max_lines` or `limits.max_bytes`.
pub fn fold_if_needed(
    cleaned: &str,
    exit_code: i32,
    limits: FoldLimits,
) -> std::io::Result<String> {
    let lines: Vec<&str> = cleaned.lines().collect();
    if lines.len() <= limits.max_lines && cleaned.len() <= limits.max_bytes {
        return Ok(cleaned.to_owned());
    }

    let log_path = write_secure_tmp_log(cleaned)?;
    let total_lines = lines.len();
    let total_bytes = cleaned.len();

    let mut out = String::new();
    if total_lines > HEAD_LINES + TAIL_LINES {
        let head = &lines[..HEAD_LINES];
        let tail = &lines[total_lines - TAIL_LINES..];
        let omitted = total_lines - HEAD_LINES - TAIL_LINES;

        // Bound individual line lengths when a line itself is huge.
        let per_line_cap = (limits.max_bytes / (HEAD_LINES + TAIL_LINES)).max(256);
        for line in head {
            out.push_str(&truncate_line(line, per_line_cap));
            out.push('\n');
        }
        out.push_str(&format!(
            "\n... [{omitted} lines omitted | total {total_lines} lines, {total_bytes}B | exit_code={exit_code} | full output saved to {}] ...\n\n",
            log_path.display()
        ));
        for line in tail {
            out.push_str(&truncate_line(line, per_line_cap));
            out.push('\n');
        }
    } else {
        // Few lines, but total byte length exceeded `max_bytes` (e.g. minified JSON/log).
        let half = (limits.max_bytes / 2).max(512);
        let head_str = slice_prefix_utf8(cleaned, half);
        let tail_str = slice_suffix_utf8(cleaned, half);
        let omitted_bytes = total_bytes.saturating_sub(head_str.len() + tail_str.len());
        out.push_str(head_str);
        out.push_str(&format!(
            "\n... [{omitted_bytes}B omitted | total {total_lines} lines, {total_bytes}B | exit_code={exit_code} | full output saved to {}] ...\n",
            log_path.display()
        ));
        out.push_str(tail_str);
        if !out.ends_with('\n') {
            out.push('\n');
        }
    }

    Ok(out)
}

fn write_secure_tmp_log(text: &str) -> std::io::Result<PathBuf> {
    let path = PathBuf::from(format!("/tmp/msh_{}.log", Uuid::new_v4()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)?;
    file.write_all(text.as_bytes())?;
    file.flush()?;
    Ok(path)
}

fn truncate_line(line: &str, max_bytes: usize) -> String {
    if line.len() <= max_bytes {
        return line.to_owned();
    }
    let prefix = slice_prefix_utf8(line, max_bytes);
    format!("{prefix}...[truncated {}B]", line.len() - prefix.len())
}

fn slice_prefix_utf8(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

fn slice_suffix_utf8(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut start = s.len() - max_bytes;
    while start < s.len() && !s.is_char_boundary(start) {
        start += 1;
    }
    &s[start..]
}
