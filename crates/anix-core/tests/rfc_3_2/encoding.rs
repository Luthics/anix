//! §3.2 rule 1. Exercise the file API because a Rust `&str` is already UTF-8.

use std::fs;

use anix_core::frontmatter::parse_script_file;

use super::TempDir;

// RFC §3.2-01 — 遍历全部 256 个单字节，区分合法 ASCII 与非法 UTF-8。
#[test]
fn all_256_single_bytes_inside_a_plain_prompt() {
    let dir = TempDir::new();
    let path = dir.0.join("byte.agent");
    for byte in 0..=u8::MAX {
        let bytes = [b'L', byte, b'R'];
        fs::write(&path, bytes).unwrap();
        let result = parse_script_file(&path);
        if byte.is_ascii() {
            let prompt = result
                .unwrap_or_else(|e| panic!("valid byte {byte:#04x}: {e}"))
                .prompt;
            // A bare CR is not a line ending defined by §3.2. Only assert acceptance for it.
            if byte != b'\r' {
                assert_eq!(prompt.as_bytes(), bytes, "ASCII byte {byte:#04x}");
            }
        } else {
            assert!(
                result.is_err(),
                "accepted isolated non-ASCII byte {byte:#04x}"
            );
        }
    }
}

// RFC §3.2-01 — 合法多字节字符在 UTF-8 边界上逐字节保留。
#[test]
fn valid_multibyte_boundaries_survive_file_decoding() {
    let dir = TempDir::new();
    let path = dir.0.join("unicode.agent");
    // Shortest and longest scalar for each UTF-8 width and both sides of the surrogate gap.
    for scalar in [
        '\u{80}',
        '\u{7ff}',
        '\u{800}',
        '\u{d7ff}',
        '\u{e000}',
        '\u{ffff}',
        '\u{10000}',
        '\u{10ffff}',
    ] {
        let source = format!("L{scalar}R");
        fs::write(&path, &source).unwrap();
        assert_eq!(
            parse_script_file(&path).unwrap().prompt,
            source,
            "U+{:04X}",
            scalar as u32
        );
    }
}

// RFC §3.2-01 — 在 shebang、头部及正文中分别插入畸形 UTF-8，解析须失败。
#[test]
fn malformed_sequences_are_rejected_in_every_script_region() {
    let dir = TempDir::new();
    let path = dir.0.join("invalid.agent");
    let sequences: &[(&str, &[u8])] = &[
        ("lone continuation", &[0x80]),
        ("overlong two-byte", &[0xc0, 0xaf]),
        ("overlong three-byte", &[0xe0, 0x80, 0xaf]),
        ("overlong four-byte", &[0xf0, 0x80, 0x80, 0xaf]),
        ("first surrogate", &[0xed, 0xa0, 0x80]),
        ("last surrogate", &[0xed, 0xbf, 0xbf]),
        ("above U+10FFFF", &[0xf4, 0x90, 0x80, 0x80]),
        ("invalid lead", &[0xf5, 0x80, 0x80, 0x80]),
        ("bad second byte", &[0xe2, b'A', 0xac]),
        ("bad third byte", &[0xe2, 0x82, b'A']),
        ("bad fourth byte", &[0xf0, 0x9f, 0x98, b'A']),
    ];
    // Each surrounding file is valid with an ASCII marker, including the ignored regions.
    let regions = [
        ("plain prompt", "L", "R"),
        ("ignored shebang", "#!anix ", "\nbody"),
        ("YAML comment", "---\n# ", "\n---\nbody"),
        ("extension value", "---\nx-meta: '", "'\n---\nbody"),
        ("body after YAML", "---\nname: demo\n---\n", ""),
    ];
    for (region, prefix, suffix) in regions {
        fs::write(&path, format!("{prefix}A{suffix}")).unwrap();
        parse_script_file(&path).unwrap_or_else(|e| panic!("invalid control in {region}: {e}"));
        for &(kind, bytes) in sequences {
            let script = [prefix.as_bytes(), bytes, suffix.as_bytes()].concat();
            fs::write(&path, script).unwrap();
            assert!(
                parse_script_file(&path).is_err(),
                "accepted {kind} in {region}: {bytes:02x?}"
            );
        }
    }
}

// RFC §3.2-01 — 多字节字符在任一内部位置遇 EOF 均应拒绝。
#[test]
fn eof_at_each_interior_byte_of_a_utf8_scalar() {
    let dir = TempDir::new();
    let path = dir.0.join("truncated.agent");
    for scalar in ["é", "界", "🦀"] {
        for cut in 0..=scalar.len() {
            let script = [b"body ".as_slice(), &scalar.as_bytes()[..cut]].concat();
            fs::write(&path, &script).unwrap();
            let result = parse_script_file(&path);
            if cut == 0 || cut == scalar.len() {
                assert_eq!(result.unwrap().prompt.as_bytes(), script);
            } else {
                assert!(
                    result.is_err(),
                    "accepted {cut}/{} bytes of {scalar}",
                    scalar.len()
                );
            }
        }
    }
}
