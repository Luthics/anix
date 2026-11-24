//! Unified side-channel control frames written to `$AGENT_CTL` (`fd 6`) by `a*` tools (Section 7.2).

use serde::{Deserialize, Serialize};

/// Hard limit per JSONL control frame on `$AGENT_CTL` (64 KiB, Section 7.2).
pub const MAX_CTL_FRAME_BYTES: usize = 64 * 1024;

/// Single JSONL control frame sent from `aview` / `aclr` to `agent` over `fd 6`.
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
    /// Emitted by `aclr -t <N> "<summary>"` to fold tail turns `N..=now` preserving KV-cache prefix.
    Fold { from_turn: u64, summary: String },
}

#[cfg(test)]
mod tests {
    use super::*;

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
                summary: "dead end".into(),
            },
        ];

        for frame in frames {
            let line = serde_json::to_string(&frame).expect("serialize ctl frame");
            assert!(line.len() <= MAX_CTL_FRAME_BYTES);
            let parsed: CtlFrame = serde_json::from_str(&line).expect("deserialize ctl frame");
            assert_eq!(parsed, frame);
        }
    }
}
