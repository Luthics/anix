//! Unified side-channel control frames written to `$ANIX_CTL` by shell tools (Section 6.2).

use serde::{Deserialize, Serialize};

/// Hard limit per JSONL control frame on `$ANIX_CTL` (64 KiB, Section 6.2).
pub const MAX_CTL_FRAME_BYTES: usize = 64 * 1024;

/// Lifecycle state for a single live task checklist step emitted via `aplan` / `CtlFrame::Plan`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanStepStatus {
    Pending,
    InProgress,
    Done,
    Skipped,
    Failed,
}

/// Single step in an ephemeral live plan checklist broadcast to `--trace-fd` (never persisted to WAL).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanStep {
    pub title: String,
    pub status: PlanStepStatus,
}

/// Single JSONL control frame sent from child tools to `anix` over `$ANIX_CTL`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum CtlFrame {
    /// Attach a local image/document path to the current tool turn (passed through to Provider).
    Media {
        mime: String,
        path: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source: Option<String>,
    },
    /// Trigger full context compaction & new WAL checkpoint (`aclr -s`).
    Compact,
    /// Fold tail turns `from_turn..=now` into a branch summary preserving frozen prefix (`aclr -t`).
    Fold {
        from_turn: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        summary: Option<String>,
    },
    /// Register a generic lease preventing `--inbox` idle-timeout exit until either a message
    /// carrying `Anix-In-Reply-To: <id>` arrives, `Release` is emitted, or `deadline_sec` expires
    /// (synthesizing a `hold-expired` event, v0.4.0 §4.3.3 & §6.2).
    Hold {
        id: String,
        deadline_sec: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<String>,
    },
    /// Explicitly release a previously registered `Hold` lease.
    Release { id: String },
    /// Broadcast an ephemeral live checklist update to `--trace-fd` (`aplan`); empty `steps` clears.
    Plan {
        #[serde(default)]
        steps: Vec<PlanStep>,
    },
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
                summary: Some("dead end".into()),
            },
            CtlFrame::Fold {
                from_turn: 4,
                summary: None,
            },
            CtlFrame::Hold {
                id: "j_1234abcd".into(),
                deadline_sec: 600,
                label: Some("cargo build".into()),
            },
            CtlFrame::Release {
                id: "j_1234abcd".into(),
            },
            CtlFrame::Plan {
                steps: vec![
                    PlanStep {
                        title: "Inspect code".into(),
                        status: PlanStepStatus::Done,
                    },
                    PlanStep {
                        title: "Apply fix".into(),
                        status: PlanStepStatus::InProgress,
                    },
                ],
            },
            CtlFrame::Plan { steps: Vec::new() },
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
