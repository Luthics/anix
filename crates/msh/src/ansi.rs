//! ANSI CSI / OSC escape sequence and `\r` progress-bar stripping (Section 7.1.3).

/// Strip ANSI escape sequences (CSI, OSC, DCS/PM/APC, 2-byte ESC) and collapse
/// carriage-return (`\r`) progress-bar overwrites on valid UTF-8 text.
pub fn clean_ansi_and_cr(input: &str) -> String {
    let stripped = strip_escape_sequences(input);
    collapse_carriage_returns(&stripped)
}

fn strip_escape_sequences(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == 0x1b {
            i += 1;
            if i >= bytes.len() {
                break;
            }
            match bytes[i] {
                // CSI: ESC [ params intermediates final (0x40..=0x7E)
                b'[' => {
                    i += 1;
                    while i < bytes.len() {
                        let b = bytes[i];
                        i += 1;
                        if (0x40..=0x7E).contains(&b) {
                            break;
                        }
                    }
                }
                // OSC: ESC ] ... (BEL 0x07 or ST ESC \)
                b']' => {
                    i += 1;
                    while i < bytes.len() {
                        if bytes[i] == 0x07 {
                            i += 1;
                            break;
                        }
                        if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'\\' {
                            i += 2;
                            break;
                        }
                        i += 1;
                    }
                }
                // DCS / SOS / PM / APC: ESC P/X/^/_ ... ST (ESC \)
                b'P' | b'X' | b'^' | b'_' => {
                    i += 1;
                    while i < bytes.len() {
                        if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'\\' {
                            i += 2;
                            break;
                        }
                        i += 1;
                    }
                }
                // Character set designation: ESC ( X / ESC ) X / ESC * X / ESC + X
                b'(' | b')' | b'*' | b'+' => {
                    i += 2;
                }
                // Other 2-byte escape sequences (ESC @ .. ESC ~)
                _ => {
                    i += 1;
                }
            }
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }

    // SAFETY: We only ever removed ASCII ESC sequences (all single-byte <= 0x7E)
    // from a valid UTF-8 input slice, preserving all multi-byte UTF-8 sequences.
    String::from_utf8(out).unwrap_or_else(|e| String::from_utf8_lossy(&e.into_bytes()).into_owned())
}

fn collapse_carriage_returns(input: &str) -> String {
    let normalized = input.replace("\r\n", "\n");
    let mut out = String::with_capacity(normalized.len());
    let has_trailing_nl = normalized.ends_with('\n');

    let mut first = true;
    for line in normalized.split('\n') {
        if !first {
            out.push('\n');
        }
        first = false;

        if line.contains('\r') {
            // Keep the last non-empty overwrite segment, or empty if all segments are empty.
            let final_seg = line.rsplit('\r').find(|seg| !seg.is_empty()).unwrap_or("");
            out.push_str(final_seg);
        } else {
            out.push_str(line);
        }
    }

    if has_trailing_nl && !out.ends_with('\n') {
        out.push('\n');
    }
    out
}
