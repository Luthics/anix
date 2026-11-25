//! Unified side-channel control frames written to `$ANIX_CTL` (`fd 6`) by `a*` tools (Section 7.2).

use serde::{Deserialize, Serialize};

/// Hard limit per JSONL control frame on `$ANIX_CTL` (64 KiB, Section 7.2).
pub const MAX_CTL_FRAME_BYTES: usize = 64 * 1024;

/// Single JSONL control frame sent from `aview` / `aclr` to `anix` over `fd 6`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum CtlFrame {
    /// Emitted by `aview` to attach a rendered/resized local image to the current tool turn.
    Media {
        mime: String,
        path: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source: Option<String>,
    },
    /// Emitted by `aclr -s` to trigger full context compaction & new WAL checkpoint.
    Compact,
    /// Emitted by `aclr -t <N> ["<summary>"]` to fold tail turns `N..=now` preserving KV-cache prefix.
    /// When `summary` is omitted (`None`), the runtime invokes `[compact]` to auto-summarize `N..=now` (v1.1 §6.1).
    Fold {
        from_turn: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        summary: Option<String>,
    },
    /// Emitted by `msh` when a command yields to background (either `timeout_sec == 0`
    /// or foreground wait expired with `kill_on_timeout == false`, v1.1.2 §§7.1.5 & 7.2).
    BgJob {
        job_id: String,
        pid: u32,
        pgid: u32,
        cmd: String,
        log_path: String,
        exit_file: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    // RFC v1.1 §7.2
    #[test]
    fn ctl_frames_round_trip() {
        let frames = [
            CtlFrame::Media {
                mime: "image/png".into(),
                path: "/tmp/aview_123.png".into(),
                source: Some("screenshot.png".into()),
            },
            CtlFrame::Compact,
            CtlFrame::Fold {
                from_turn: 3,
                summary: Some("dead end".into()),
            },
            CtlFrame::Fold {
                from_turn: 4,
                summary: None,
            },
            CtlFrame::BgJob {
                job_id: "j_1234abcd".into(),
                pid: 4242,
                pgid: 4242,
                cmd: "cargo build".into(),
                log_path: "/tmp/anix-1000/jobs/j_1234abcd.log".into(),
                exit_file: "/tmp/anix-1000/jobs/j_1234abcd.exit".into(),
            },
        ];

        for frame in frames {
            let line = serde_json::to_string(&frame).expect("serialize ctl frame");
            assert!(line.len() <= MAX_CTL_FRAME_BYTES);
            if matches!(frame, CtlFrame::Fold { summary: None, .. }) {
                assert_eq!(line, "{\"op\":\"fold\",\"from_turn\":4}");
            }
            let parsed: CtlFrame = serde_json::from_str(&line).expect("deserialize ctl frame");
            assert_eq!(parsed, frame);
        }
    }
}
