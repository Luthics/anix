//! Maildir message envelope framing for `--inbox`, `anix send [--wait]`, and `--notify` (Section 4.3).
//!
//! Plain text files placed directly into `<MAILDIR>/new/` are accepted with zero framing;
//! structured messages prepend `Anix-*:` header lines terminated by a blank line (`\n\n`).

/// Optional structured headers at the top of a Maildir message file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MaildirHeaders {
    /// Globally unique message ID used for WAL crash deduplication (`Anix-Message-Id`).
    pub message_id: Option<String>,
    /// Temporary reply Maildir path for synchronous `anix send --wait` (`Anix-Reply-To`).
    pub reply_to: Option<String>,
    /// Correlated lease or message ID; releases matching `hold` leases (`Anix-In-Reply-To`).
    pub in_reply_to: Option<String>,
    /// Optional sender label (`Anix-From`).
    pub from: Option<String>,
    /// Kernel control operation such as `"compact"` (`Anix-Control`).
    pub control: Option<String>,
    /// Process exit code carried on `--wait` replies and `--notify` callbacks (`Anix-Exit-Code`).
    pub exit_code: Option<i32>,
}

impl MaildirHeaders {
    pub fn is_empty(&self) -> bool {
        self.message_id.is_none()
            && self.reply_to.is_none()
            && self.in_reply_to.is_none()
            && self.from.is_none()
            && self.control.is_none()
            && self.exit_code.is_none()
    }
}

/// Parsed Maildir message (optional `Anix-*:` headers + UTF-8 body).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaildirEnvelope {
    pub headers: MaildirHeaders,
    pub body: String,
}

impl MaildirEnvelope {
    /// Format into a Maildir file payload. Emits raw body when all headers are empty.
    pub fn format(&self) -> String {
        if self.headers.is_empty() {
            return self.body.clone();
        }
        let mut out = String::new();
        if let Some(ref v) = self.headers.message_id {
            out.push_str("Anix-Message-Id: ");
            out.push_str(&sanitize_header_val(v));
            out.push('\n');
        }
        if let Some(ref v) = self.headers.reply_to {
            out.push_str("Anix-Reply-To: ");
            out.push_str(&sanitize_header_val(v));
            out.push('\n');
        }
        if let Some(ref v) = self.headers.in_reply_to {
            out.push_str("Anix-In-Reply-To: ");
            out.push_str(&sanitize_header_val(v));
            out.push('\n');
        }
        if let Some(ref v) = self.headers.from {
            out.push_str("Anix-From: ");
            out.push_str(&sanitize_header_val(v));
            out.push('\n');
        }
        if let Some(ref v) = self.headers.control {
            out.push_str("Anix-Control: ");
            out.push_str(&sanitize_header_val(v));
            out.push('\n');
        }
        if let Some(code) = self.headers.exit_code {
            out.push_str(&format!("Anix-Exit-Code: {code}\n"));
        }
        out.push('\n');
        out.push_str(&self.body);
        out
    }

    /// Parse raw Maildir file contents; falls back cleanly to plain text when no `Anix-*:` header
    /// prefix is present. `fallback_id` (typically the Maildir filename) populates `message_id`
    /// whenever `Anix-Message-Id` is omitted.
    pub fn parse(raw: &str, fallback_id: &str) -> Self {
        if !starts_with_anix_header(raw) {
            return Self {
                headers: MaildirHeaders {
                    message_id: Some(fallback_id.to_owned()),
                    ..Default::default()
                },
                body: raw.to_owned(),
            };
        }

        let mut headers = MaildirHeaders::default();
        let mut cursor = 0usize;
        let bytes = raw.as_bytes();
        let mut body_start = raw.len();

        while cursor < bytes.len() {
            let line_end = bytes[cursor..]
                .iter()
                .position(|&b| b == b'\n')
                .map(|rel| cursor + rel)
                .unwrap_or(bytes.len());
            let mut line = &raw[cursor..line_end];
            if let Some(stripped) = line.strip_suffix('\r') {
                line = stripped;
            }
            let next_cursor = if line_end < bytes.len() {
                line_end + 1
            } else {
                bytes.len()
            };

            if line.is_empty() {
                body_start = next_cursor;
                break;
            }

            if let Some((key, val)) = line.split_once(':') {
                let val = val.trim();
                match key.trim().to_ascii_lowercase().as_str() {
                    "anix-message-id" if !val.is_empty() => {
                        headers.message_id = Some(val.to_owned());
                    }
                    "anix-reply-to" if !val.is_empty() => {
                        headers.reply_to = Some(val.to_owned());
                    }
                    "anix-in-reply-to" if !val.is_empty() => {
                        headers.in_reply_to = Some(val.to_owned());
                    }
                    "anix-from" if !val.is_empty() => {
                        headers.from = Some(val.to_owned());
                    }
                    "anix-control" if !val.is_empty() => {
                        headers.control = Some(val.to_owned());
                    }
                    "anix-exit-code" => {
                        if let Ok(code) = val.parse::<i32>() {
                            headers.exit_code = Some(code);
                        }
                    }
                    _ => {}
                }
            }
            cursor = next_cursor;
        }

        if headers.message_id.is_none() {
            headers.message_id = Some(fallback_id.to_owned());
        }

        Self {
            headers,
            body: raw[body_start..].to_owned(),
        }
    }

    /// Return the resolved non-empty message ID.
    pub fn message_id(&self) -> &str {
        self.headers.message_id.as_deref().unwrap_or("")
    }
}

fn starts_with_anix_header(raw: &str) -> bool {
    let first_line = raw.lines().next().unwrap_or("").trim_end_matches('\r');
    let Some((key, _)) = first_line.split_once(':') else {
        return false;
    };
    let lower = key.trim().to_ascii_lowercase();
    lower.starts_with("anix-") && !lower.contains(char::is_whitespace)
}

fn sanitize_header_val(v: &str) -> String {
    v.replace(['\r', '\n'], " ").trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_maildir_parses_with_filename_fallback_id() {
        let env = MaildirEnvelope::parse("Check deploy status\nline 2", "001-check.msg");
        assert_eq!(env.message_id(), "001-check.msg");
        assert_eq!(env.headers.reply_to, None);
        assert_eq!(env.body, "Check deploy status\nline 2");
    }

    #[test]
    fn framed_maildir_envelope_round_trips_all_headers() {
        let orig = MaildirEnvelope {
            headers: MaildirHeaders {
                message_id: Some("msg-123".into()),
                reply_to: Some("/tmp/anix-reply-1".into()),
                in_reply_to: Some("hold-9".into()),
                from: Some("msh".into()),
                control: Some("compact".into()),
                exit_code: Some(0),
            },
            body: "Job done.\n\nDetails here.".into(),
        };
        let encoded = orig.format();
        let decoded = MaildirEnvelope::parse(&encoded, "fallback");
        assert_eq!(decoded, orig);
    }

    #[test]
    fn control_only_header_without_body_parses_cleanly() {
        let env = MaildirEnvelope::parse("Anix-Control: compact\n", "ctl-1.msg");
        assert_eq!(env.message_id(), "ctl-1.msg");
        assert_eq!(env.headers.control.as_deref(), Some("compact"));
        assert!(env.body.is_empty());
    }
}
