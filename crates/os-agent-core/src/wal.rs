//! Append-Only WAL JSONL persistence with `flock(LOCK_EX | LOCK_NB)`, O(active-window)
//! reverse-read replay, tail corruption Fail-Closed, and Tool-Pair Closure (Section 6.2).

use std::collections::{BTreeMap, HashSet};
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};

use os_agent_proto::IR_VERSION;
use os_agent_proto::exit_code::ExitCode;
use os_agent_proto::ir::{IrMessage, ToolCall};
use os_agent_proto::provider::TokenUsage;
use os_agent_proto::wal::{WalConfigSnapshot, WalEvent, WalRole};
use thiserror::Error;

use crate::platform::posix;

const REVERSE_CHUNK_SIZE: usize = 64 * 1024;

#[derive(Debug, Error)]
pub enum WalError {
    #[error("WAL context file `{0}` is locked by another running agent process")]
    Locked(String),
    #[error("incompatible WAL ir_version {0} (expected {IR_VERSION})")]
    IncompatibleVersion(u32),
    #[error("corrupted trailing WAL line in `{path}`: {reason}")]
    CorruptedTail { path: String, reason: String },
    #[error("malformed WAL entry in `{path}`: {reason}")]
    MalformedEntry { path: String, reason: String },
    #[error("WAL I/O error on `{path}`: {source}")]
    Io {
        path: String,
        #[source]
        source: io::Error,
    },
}

impl WalError {
    pub fn exit_code(&self) -> ExitCode {
        match self {
            Self::IncompatibleVersion(_) => ExitCode::UsageError,
            _ => ExitCode::RuntimeError,
        }
    }
}

/// Active conversation window reconstructed from the latest WAL section.
#[derive(Debug, Clone)]
pub struct ReplayedSession {
    pub messages: Vec<IrMessage>,
    /// Recorded `usage` for surviving `role: "assistant"` turns in `messages` (keyed by `turn_id`).
    pub assistant_usages: BTreeMap<u64, TokenUsage>,
    /// Exact cross-compaction 5-field cumulative usage (`checkpoint.cumulative_usage` + all
    /// `assistant.usage` in this active window including turns pruned by `fold`).
    pub session_usage: TokenUsage,
    pub latest_config: Option<WalConfigSnapshot>,
    pub last_checkpoint_turn: u64,
    pub next_turn_id: u64,
    pub is_fresh: bool,
}

/// Append-only WAL file handle protected by an exclusive `flock`.
#[derive(Debug)]
pub struct WalStore {
    path: Option<PathBuf>,
    file: Option<File>,
}

impl WalStore {
    /// Create a strictly stateless in-memory store (no disk file).
    pub fn stateless() -> Self {
        Self {
            path: None,
            file: None,
        }
    }

    /// Open (or create with mode `0600`) a WAL file and acquire `flock(LOCK_EX | LOCK_NB)`.
    pub fn open(path: &Path) -> Result<Self, WalError> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
            && !parent.exists()
        {
            std::fs::create_dir_all(parent).map_err(|source| WalError::Io {
                path: path.display().to_string(),
                source,
            })?;
        }

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(path)
            .map_err(|source| WalError::Io {
                path: path.display().to_string(),
                source,
            })?;

        let _ = posix::set_cloexec(file.as_raw_fd());

        let acquired = posix::flock_exclusive_nonblocking(file.as_raw_fd()).map_err(|source| {
            WalError::Io {
                path: path.display().to_string(),
                source,
            }
        })?;
        if !acquired {
            return Err(WalError::Locked(path.display().to_string()));
        }

        Ok(Self {
            path: Some(path.to_path_buf()),
            file: Some(file),
        })
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Reconstruct active context via O(active-window) reverse-read from EOF (Section 6.2.4).
    pub fn load_active_window(&mut self) -> Result<ReplayedSession, WalError> {
        let Some(ref mut file) = self.file else {
            return Ok(ReplayedSession {
                messages: Vec::new(),
                assistant_usages: BTreeMap::new(),
                session_usage: TokenUsage::default(),
                latest_config: None,
                last_checkpoint_turn: 0,
                next_turn_id: 1,
                is_fresh: true,
            });
        };
        let path_str = self
            .path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default();

        let file_len = file.seek(SeekFrom::End(0)).map_err(|source| WalError::Io {
            path: path_str.clone(),
            source,
        })?;
        if file_len == 0 {
            return Ok(ReplayedSession {
                messages: Vec::new(),
                assistant_usages: BTreeMap::new(),
                session_usage: TokenUsage::default(),
                latest_config: None,
                last_checkpoint_turn: 0,
                next_turn_id: 1,
                is_fresh: true,
            });
        }

        let mut rev_reader = ReverseLineReader::new(file, file_len);
        let mut is_first_nonempty = true;
        let mut skip_ge: u64 = u64::MAX;
        let mut collected_rev: Vec<IrMessage> = Vec::new();
        let mut surviving_assistant_usages: BTreeMap<u64, TokenUsage> = BTreeMap::new();
        let mut window_usage = TokenUsage::default();
        let mut checkpoint_base_usage = TokenUsage::default();
        let mut latest_config: Option<WalConfigSnapshot> = None;
        let mut last_checkpoint_turn: u64 = 0;
        let mut max_turn_id: u64 = 0;
        let mut found_section_header = false;

        while let Some(raw_line) = rev_reader.next_line().map_err(|source| WalError::Io {
            path: path_str.clone(),
            source,
        })? {
            if raw_line.is_empty() && is_first_nonempty {
                // Normal trailing `\n` at EOF.
                continue;
            }
            if raw_line.iter().all(|b| b.is_ascii_whitespace()) {
                continue;
            }

            let line_str = match std::str::from_utf8(&raw_line) {
                Ok(s) => s,
                Err(err) => {
                    return if is_first_nonempty {
                        Err(WalError::CorruptedTail {
                            path: path_str,
                            reason: format!("invalid UTF-8 on last line: {err}"),
                        })
                    } else {
                        Err(WalError::MalformedEntry {
                            path: path_str,
                            reason: format!("invalid UTF-8: {err}"),
                        })
                    };
                }
            };

            let event: WalEvent = match serde_json::from_str(line_str) {
                Ok(ev) => ev,
                Err(err) => {
                    return if is_first_nonempty {
                        Err(WalError::CorruptedTail {
                            path: path_str,
                            reason: format!("invalid JSON on last line: {err}"),
                        })
                    } else {
                        Err(WalError::MalformedEntry {
                            path: path_str,
                            reason: format!("invalid JSON: {err}"),
                        })
                    };
                }
            };
            is_first_nonempty = false;

            match event {
                WalEvent::SessionInit {
                    ir_version, config, ..
                } => {
                    if ir_version != IR_VERSION {
                        return Err(WalError::IncompatibleVersion(ir_version));
                    }
                    if latest_config.is_none() {
                        latest_config = Some(config);
                    }
                    last_checkpoint_turn = 0;
                    found_section_header = true;
                    break;
                }
                WalEvent::Checkpoint {
                    ir_version,
                    turn_id,
                    config,
                    cumulative_usage,
                    ..
                } => {
                    if ir_version != IR_VERSION {
                        return Err(WalError::IncompatibleVersion(ir_version));
                    }
                    if latest_config.is_none() {
                        latest_config = Some(config);
                    }
                    if let Some(cum) = cumulative_usage {
                        checkpoint_base_usage = cum;
                    }
                    last_checkpoint_turn = turn_id;
                    max_turn_id = max_turn_id.max(turn_id);
                    found_section_header = true;
                    break;
                }
                WalEvent::Fold {
                    from_turn,
                    to_turn,
                    summary,
                    fold_usage,
                    ..
                } => {
                    max_turn_id = max_turn_id.max(to_turn).max(from_turn);
                    if let Some(ref fu) = fold_usage {
                        window_usage.add_assign(fu);
                    }
                    if to_turn < skip_ge {
                        collected_rev.push(IrMessage::user(
                            from_turn,
                            format!("<folded turns=\"{from_turn}..{to_turn}\">{summary}</folded>"),
                        ));
                    }
                    skip_ge = skip_ge.min(from_turn);
                }
                WalEvent::RuntimeUpdate {
                    turn_id,
                    content,
                    config,
                    ..
                } => {
                    max_turn_id = max_turn_id.max(turn_id);
                    if latest_config.is_none() {
                        latest_config = Some(config);
                    }
                    if turn_id < skip_ge {
                        collected_rev.push(IrMessage::system(
                            turn_id,
                            format!("<runtime_update>{content}</runtime_update>"),
                        ));
                    }
                }
                WalEvent::Message {
                    turn_id,
                    role,
                    text,
                    reasoning,
                    tool_calls,
                    tool_call_id,
                    is_error,
                    media,
                    provider_opaque,
                    usage,
                    ..
                } => {
                    max_turn_id = max_turn_id.max(turn_id);

                    // Section 6.2.4 Rule 2: accumulate EVERY scanned `assistant.usage` in this
                    // active window into `window_usage`, EVEN IF pruned by `fold` (`turn_id >= skip_ge`).
                    if role == WalRole::Assistant
                        && let Some(ref u) = usage
                    {
                        window_usage.add_assign(u);
                    }

                    if turn_id >= skip_ge {
                        continue;
                    }

                    match role {
                        WalRole::System => {
                            collected_rev
                                .push(IrMessage::system(turn_id, text.unwrap_or_default()));
                        }
                        WalRole::User => {
                            collected_rev.push(IrMessage::User {
                                turn_id,
                                text: text.unwrap_or_default(),
                                media,
                                extra: BTreeMap::new(),
                            });
                        }
                        WalRole::Tool => {
                            collected_rev.push(IrMessage::Tool {
                                turn_id,
                                tool_call_id: tool_call_id.unwrap_or_default(),
                                is_error,
                                text: text.unwrap_or_default(),
                                media,
                                extra: BTreeMap::new(),
                            });
                        }
                        WalRole::Assistant => {
                            // Self-heal any legacy/corrupted empty assistant turn (neither text
                            // nor tool_calls) so historical WAL files can never violate Section 9.1
                            // on replay; its `usage` was already accumulated into `window_usage`.
                            let has_text = text.as_deref().is_some_and(|t| !t.is_empty());
                            if !has_text && tool_calls.is_empty() {
                                continue;
                            }

                            // Rule 3 (Tool-Pair Closure during reverse-read):
                            // If this assistant carries `tool_calls`, verify EVERY tool_call ID
                            // is matched by contiguous surviving `Tool` messages immediately
                            // preceding it in `collected_rev` (which are temporally after it).
                            if !tool_calls.is_empty()
                                && !assistant_tool_calls_are_closed(&collected_rev, &tool_calls)
                            {
                                // Drop any partial tool messages belonging to this unclosed assistant
                                // and raise `skip_ge` to eliminate the orphaned tool_calls.
                                let call_ids: HashSet<&str> =
                                    tool_calls.iter().map(|tc| tc.id.as_str()).collect();
                                while let Some(IrMessage::Tool { tool_call_id, .. }) =
                                    collected_rev.last()
                                {
                                    if call_ids.contains(tool_call_id.as_str()) {
                                        collected_rev.pop();
                                    } else {
                                        break;
                                    }
                                }
                                skip_ge = skip_ge.min(turn_id);
                                continue;
                            }
                            if let Some(u) = usage {
                                surviving_assistant_usages.insert(turn_id, u);
                            }
                            collected_rev.push(IrMessage::Assistant {
                                turn_id,
                                text,
                                reasoning,
                                tool_calls,
                                provider_opaque,
                                extra: BTreeMap::new(),
                            });
                        }
                    }
                }
            }
        }

        if !found_section_header {
            return Err(WalError::MalformedEntry {
                path: path_str,
                reason: "missing `session_init` or `checkpoint` section header".into(),
            });
        }

        collected_rev.reverse();

        // Final safeguard: drop any leading orphaned tool message (if a fold started mid-batch).
        let messages = drop_orphan_tools(collected_rev);

        let surviving_ids: HashSet<u64> = messages
            .iter()
            .filter_map(|m| match m {
                IrMessage::Assistant { turn_id, .. } => Some(*turn_id),
                _ => None,
            })
            .collect();
        surviving_assistant_usages.retain(|tid, _| surviving_ids.contains(tid));

        let mut session_usage = checkpoint_base_usage;
        session_usage.add_assign(&window_usage);

        Ok(ReplayedSession {
            messages,
            assistant_usages: surviving_assistant_usages,
            session_usage,
            latest_config,
            last_checkpoint_turn,
            next_turn_id: max_turn_id.saturating_add(1).max(1),
            is_fresh: false,
        })
    }

    /// Append a single WAL event line atomically and flush to disk.
    pub fn append(&mut self, event: &WalEvent) -> Result<(), WalError> {
        let Some(ref mut file) = self.file else {
            return Ok(());
        };
        let path_str = self
            .path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default();

        let mut line = serde_json::to_string(event).map_err(|e| WalError::MalformedEntry {
            path: path_str.clone(),
            reason: e.to_string(),
        })?;
        line.push('\n');

        file.seek(SeekFrom::End(0)).map_err(|source| WalError::Io {
            path: path_str.clone(),
            source,
        })?;
        file.write_all(line.as_bytes())
            .and_then(|_| file.flush())
            .map_err(|source| WalError::Io {
                path: path_str,
                source,
            })?;
        Ok(())
    }

    pub fn append_message(&mut self, msg: &IrMessage) -> Result<(), WalError> {
        self.append_message_with_usage(msg, None)
    }

    pub fn append_message_with_usage(
        &mut self,
        msg: &IrMessage,
        usage: Option<TokenUsage>,
    ) -> Result<(), WalError> {
        let ts = posix::timestamp_rfc3339_utc();
        let event = ir_message_to_wal_event(msg, ts, usage);
        self.append(&event)
    }
}

/// Convert an [`IrMessage`] into its corresponding [`WalEvent::Message`].
pub fn ir_message_to_wal_event(msg: &IrMessage, ts: String, usage: Option<TokenUsage>) -> WalEvent {
    match msg {
        IrMessage::System { turn_id, text, .. } => WalEvent::Message {
            turn_id: *turn_id,
            role: WalRole::System,
            ts,
            text: Some(text.clone()),
            reasoning: None,
            tool_calls: Vec::new(),
            tool_call_id: None,
            is_error: false,
            media: Vec::new(),
            provider_opaque: None,
            usage: None,
        },
        IrMessage::User {
            turn_id,
            text,
            media,
            ..
        } => WalEvent::Message {
            turn_id: *turn_id,
            role: WalRole::User,
            ts,
            text: Some(text.clone()),
            reasoning: None,
            tool_calls: Vec::new(),
            tool_call_id: None,
            is_error: false,
            media: media.clone(),
            provider_opaque: None,
            usage: None,
        },
        IrMessage::Assistant {
            turn_id,
            text,
            reasoning,
            tool_calls,
            provider_opaque,
            ..
        } => WalEvent::Message {
            turn_id: *turn_id,
            role: WalRole::Assistant,
            ts,
            text: text.clone(),
            reasoning: reasoning.clone(),
            tool_calls: tool_calls.clone(),
            tool_call_id: None,
            is_error: false,
            media: Vec::new(),
            provider_opaque: provider_opaque.clone(),
            usage,
        },
        IrMessage::Tool {
            turn_id,
            tool_call_id,
            is_error,
            text,
            media,
            ..
        } => WalEvent::Message {
            turn_id: *turn_id,
            role: WalRole::Tool,
            ts,
            text: Some(text.clone()),
            reasoning: None,
            tool_calls: Vec::new(),
            tool_call_id: Some(tool_call_id.clone()),
            is_error: *is_error,
            media: media.clone(),
            provider_opaque: None,
            usage: None,
        },
    }
}

/// Align `requested_from_turn` upward to satisfy Tool-Pair Closure (Section 6.1.3):
/// If `requested_from_turn` lands on a `role: "tool"` message, walk back to the paired
/// `assistant(tool_call)`'s `turn_id` so the assistant + all its tool results fold together.
pub fn align_fold_from_turn(messages: &[IrMessage], requested_from_turn: u64) -> u64 {
    let mut aligned = requested_from_turn;

    // Locate the first message with `turn_id >= aligned`.
    if let Some(idx) = messages.iter().position(|m| m.turn_id() >= aligned) {
        if matches!(messages[idx], IrMessage::Tool { .. }) {
            // Walk backwards across any preceding contiguous `Tool` messages to their `Assistant`.
            let mut j = idx;
            while j > 0 {
                j -= 1;
                match &messages[j] {
                    IrMessage::Tool { .. } => continue,
                    IrMessage::Assistant {
                        turn_id,
                        tool_calls,
                        ..
                    } if !tool_calls.is_empty() => {
                        aligned = aligned.min(*turn_id);
                        break;
                    }
                    _ => break,
                }
            }
        }
    }

    // Also verify the last message remaining BEFORE `aligned` isn't an assistant with tool_calls
    // whose tools were all folded away.
    if let Some(last_kept) = messages.iter().rev().find(|m| m.turn_id() < aligned)
        && let IrMessage::Assistant {
            turn_id,
            tool_calls,
            ..
        } = last_kept
        && !tool_calls.is_empty()
    {
        aligned = aligned.min(*turn_id);
    }

    aligned
}

fn assistant_tool_calls_are_closed(collected_rev: &[IrMessage], tool_calls: &[ToolCall]) -> bool {
    let expected: HashSet<&str> = tool_calls.iter().map(|tc| tc.id.as_str()).collect();
    let mut seen: HashSet<&str> = HashSet::with_capacity(expected.len());

    // Remember `collected_rev` is in reverse chronological order, so the messages
    // immediately following this assistant in time are at the END of `collected_rev`.
    for msg in collected_rev.iter().rev() {
        match msg {
            IrMessage::Tool { tool_call_id, .. } => {
                if expected.contains(tool_call_id.as_str()) {
                    seen.insert(tool_call_id.as_str());
                    if seen.len() == expected.len() {
                        return true;
                    }
                } else {
                    return false;
                }
            }
            _ => break,
        }
    }
    seen.len() == expected.len()
}

fn drop_orphan_tools(messages: Vec<IrMessage>) -> Vec<IrMessage> {
    let mut out = Vec::with_capacity(messages.len());
    let mut pending_calls: HashSet<String> = HashSet::new();
    for msg in messages {
        match &msg {
            IrMessage::Assistant { tool_calls, .. } => {
                pending_calls.clear();
                for tc in tool_calls {
                    pending_calls.insert(tc.id.clone());
                }
                out.push(msg);
            }
            IrMessage::Tool { tool_call_id, .. } => {
                if pending_calls.remove(tool_call_id) {
                    out.push(msg);
                }
            }
            _ => {
                pending_calls.clear();
                out.push(msg);
            }
        }
    }
    out
}

/// Chunked reverse line reader reading backwards from EOF without loading archived history.
struct ReverseLineReader<'a> {
    file: &'a mut File,
    pos: u64,
    buffer: Vec<u8>,
}

impl<'a> ReverseLineReader<'a> {
    fn new(file: &'a mut File, file_len: u64) -> Self {
        Self {
            file,
            pos: file_len,
            buffer: Vec::new(),
        }
    }

    fn next_line(&mut self) -> io::Result<Option<Vec<u8>>> {
        loop {
            if let Some(nl_idx) = self.buffer.iter().rposition(|&b| b == b'\n') {
                let mut line = self.buffer.split_off(nl_idx + 1);
                self.buffer.pop(); // remove the `\n`
                if line.last() == Some(&b'\r') {
                    line.pop();
                }
                return Ok(Some(line));
            }

            if self.pos == 0 {
                if self.buffer.is_empty() {
                    return Ok(None);
                }
                let mut line = std::mem::take(&mut self.buffer);
                if line.last() == Some(&b'\r') {
                    line.pop();
                }
                return Ok(Some(line));
            }

            let read_len = self.pos.min(REVERSE_CHUNK_SIZE as u64) as usize;
            self.pos -= read_len as u64;
            self.file.seek(SeekFrom::Start(self.pos))?;
            let mut chunk = vec![0u8; read_len];
            self.file.read_exact(&mut chunk)?;
            chunk.extend_from_slice(&self.buffer);
            self.buffer = chunk;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use os_agent_proto::ir::{HeaderTag, IrHeader, IrMeta, IrStream, StderrMode};

    fn dummy_header() -> IrHeader {
        IrHeader {
            ir_version: IR_VERSION,
            record_type: HeaderTag::Header,
            tools: Vec::new(),
            max_output_tokens: None,
            params: BTreeMap::new(),
            meta: IrMeta {
                stderr_mode: StderrMode::Auto,
                cwd: "/tmp".into(),
                turn_budget: None,
                extra: BTreeMap::new(),
            },
            extra: BTreeMap::new(),
        }
    }

    #[test]
    fn replay_skips_empty_assistant_and_prunes_unclosed_tool_calls_preserving_usage() {
        let tmp =
            std::env::temp_dir().join(format!("os-agent-wal-test-{}.jsonl", uuid::Uuid::new_v4()));
        {
            let mut store = WalStore::open(&tmp).expect("open wal");
            store
                .append(&WalEvent::SessionInit {
                    ir_version: IR_VERSION,
                    ts: "2026-11-25T00:00:00Z".into(),
                    config: WalConfigSnapshot {
                        stderr: None,
                        approval: None,
                        vision: None,
                    },
                })
                .unwrap();
            store.append_message(&IrMessage::system(0, "sys")).unwrap();
            store.append_message(&IrMessage::user(1, "hello")).unwrap();
            // Legacy/empty assistant turn (neither text nor tool_calls) with real usage:
            store
                .append_message_with_usage(
                    &IrMessage::Assistant {
                        turn_id: 2,
                        text: Some(String::new()),
                        reasoning: Some("thinking...".into()),
                        tool_calls: Vec::new(),
                        provider_opaque: None,
                        extra: BTreeMap::new(),
                    },
                    Some(TokenUsage {
                        input_tokens: 100,
                        output_tokens: 20,
                        cached_tokens: None,
                        cache_write_tokens: None,
                        reasoning_tokens: Some(20),
                    }),
                )
                .unwrap();
            // Unclosed tool_calls turn interrupted mid-execution:
            store
                .append_message_with_usage(
                    &IrMessage::Assistant {
                        turn_id: 3,
                        text: None,
                        reasoning: None,
                        tool_calls: vec![
                            ToolCall {
                                id: "c1".into(),
                                name: "msh".into(),
                                arguments: serde_json::json!({"cmd": "true"}),
                            },
                            ToolCall {
                                id: "c2".into(),
                                name: "msh".into(),
                                arguments: serde_json::json!({"cmd": "sleep 99"}),
                            },
                        ],
                        provider_opaque: None,
                        extra: BTreeMap::new(),
                    },
                    Some(TokenUsage {
                        input_tokens: 200,
                        output_tokens: 40,
                        cached_tokens: None,
                        cache_write_tokens: None,
                        reasoning_tokens: None,
                    }),
                )
                .unwrap();
            store
                .append_message(&IrMessage::Tool {
                    turn_id: 4,
                    tool_call_id: "c1".into(),
                    is_error: false,
                    text: "[t4] exit=0\n".into(),
                    media: Vec::new(),
                    extra: BTreeMap::new(),
                })
                .unwrap();
        }

        let mut store = WalStore::open(&tmp).expect("reopen wal");
        let replayed = store.load_active_window().expect("replay wal");
        let _ = std::fs::remove_file(&tmp);

        // Both the empty assistant and the unclosed tool_calls batch (plus its partial tool `c1`)
        // must be dropped, leaving a strictly valid Canonical IR stream.
        assert_eq!(replayed.messages.len(), 2);
        assert!(matches!(
            replayed.messages[0],
            IrMessage::System { turn_id: 0, .. }
        ));
        assert!(matches!(
            replayed.messages[1],
            IrMessage::User { turn_id: 1, .. }
        ));
        assert!(replayed.assistant_usages.is_empty());

        // Billing ledger must still retain usage from both dropped turns!
        assert_eq!(replayed.session_usage.input_tokens, 300);
        assert_eq!(replayed.session_usage.output_tokens, 60);
        assert_eq!(replayed.session_usage.reasoning(), 20);

        let stream = IrStream {
            header: dummy_header(),
            messages: replayed.messages,
        };
        stream
            .validate_invariants()
            .expect("replayed stream must satisfy invariants");
    }
}
