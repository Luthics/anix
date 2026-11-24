//! Binary output circuit breaker (Section 7.1.2).
//!
//! Runs BEFORE ANSI cleaning: if the first 8192 bytes contain a NUL (`0x00`) or
//! the entire buffer fails UTF-8 validation, raw bytes are saved to
//! `/tmp/msh_<uuid>.bin` (mode `0600`) and replaced with a single metadata line.

use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;

use uuid::Uuid;

const BINARY_PROBE_BYTES: usize = 8192;

/// Outcome of the binary circuit breaker check.
#[derive(Debug)]
pub enum OutputKind<'a> {
    Text(&'a str),
    BinarySuppressed {
        bytes: usize,
        mime: &'static str,
        path: PathBuf,
    },
}

/// Inspect raw merged output. Returns `Ok(OutputKind::Text)` if valid UTF-8 with
/// no NUL byte in the first 8192 bytes; otherwise writes raw bytes to
/// `/tmp/msh_<uuid>.bin` (mode `0600`) and returns metadata.
pub fn inspect_or_suppress(raw: &[u8]) -> std::io::Result<OutputKind<'_>> {
    let probe_len = raw.len().min(BINARY_PROBE_BYTES);
    let has_nul = raw[..probe_len].contains(&0u8);

    if !has_nul && let Ok(text) = std::str::from_utf8(raw) {
        return Ok(OutputKind::Text(text));
    }

    let mime = sniff_mime(raw);
    let path = write_secure_tmp_bin(raw)?;
    Ok(OutputKind::BinarySuppressed {
        bytes: raw.len(),
        mime,
        path,
    })
}

/// Format the normative single-line suppression notice (Section 7.1.2).
pub fn format_suppression_line(bytes: usize, mime: &str, path: &std::path::Path) -> String {
    format!(
        "[binary output suppressed: {bytes}B, mime: {mime}, saved to {}. Use xxd/strings/pdftotext or aview to inspect]",
        path.display()
    )
}

fn write_secure_tmp_bin(raw: &[u8]) -> std::io::Result<PathBuf> {
    let path = PathBuf::from(format!("/tmp/msh_{}.bin", Uuid::new_v4()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)?;
    file.write_all(raw)?;
    file.flush()?;
    Ok(path)
}

/// Magic-byte MIME sniffer (fallback: `application/octet-stream`).
pub fn sniff_mime(data: &[u8]) -> &'static str {
    if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "image/jpeg"
    } else if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        "image/gif"
    } else if data.len() >= 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        "image/webp"
    } else if data.starts_with(b"%PDF-") {
        "application/pdf"
    } else if data.starts_with(&[0x1F, 0x8B]) {
        "application/gzip"
    } else if data.starts_with(b"PK\x03\x04") || data.starts_with(b"PK\x05\x06") {
        "application/zip"
    } else if data.starts_with(b"\x7fELF") {
        "application/x-elf"
    } else if data.starts_with(&[0xFE, 0xED, 0xFA, 0xCE])
        || data.starts_with(&[0xFE, 0xED, 0xFA, 0xCF])
        || data.starts_with(&[0xCE, 0xFA, 0xED, 0xFE])
        || data.starts_with(&[0xCF, 0xFA, 0xED, 0xFE])
        || data.starts_with(&[0xCA, 0xFE, 0xBA, 0xBE])
    {
        "application/x-mach-binary"
    } else if data.starts_with(b"SQLite format 3\0") {
        "application/vnd.sqlite3"
    } else if data.starts_with(b"\0asm") {
        "application/wasm"
    } else if data.starts_with(&[0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00]) {
        "application/x-xz"
    } else if data.starts_with(&[0x28, 0xB5, 0x2F, 0xFD]) {
        "application/zstd"
    } else if data.starts_with(b"BZh") {
        "application/x-bzip2"
    } else if data.len() >= 263 && &data[257..263] == b"ustar\0" {
        "application/x-tar"
    } else {
        "application/octet-stream"
    }
}
