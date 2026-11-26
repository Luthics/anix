//! Append-Only Tree WAL JSONL persistence with `flock(LOCK_EX | LOCK_NB)`, single-pass
//! reverse-read lineage replay (`id` / `parent_id` + `--leaf <id>`), tail corruption Fail-Closed,
//! Tool-Pair Closure, and legacy pre-v0.3 linear WAL fallback (Section 6.2).

use std::collections::{BTreeMap, HashSet};
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};

use anix_proto::IR_VERSION;
use anix_proto::exit_code::ExitCode;
use anix_proto::ir::{IrMessage, ToolCall};
use anix_proto::provider::TokenUsage;
use anix_proto::wal::{WalConfigSnapshot, WalEvent, WalRole};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::platform::posix;

const REVERSE_CHUNK_SIZE: usize = 64 * 1024;

#[derive(Debug, Error)]
pub enum WalError {
    #[error("WAL context file `{0}` is locked by another running agent process")]
    Locked(String),
    #[error("incompatible WAL ir_version {0} (expected {IR_VERSION})")]
    IncompatibleVersion(u32),
    #[error("WAL node id {leaf_id} not found in `{path}`")]
    LeafNotFound { path: String, leaf_id: u64 },
    #[error("invalid --leaf {leaf_id} in `{path}`: {reason}")]
    InvalidLeaf {
        path: String,
        leaf_id: u64,
        reason: String,
    },
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
            Self::IncompatibleVersion(_) | Self::LeafNotFound { .. } | Self::InvalidLeaf { .. } => {
                ExitCode::UsageError
            }
            _ => ExitCode::RuntimeError,
        }
    }
}

/// Single surviving entry in the active conversation window, pairing its WAL tree `node_id`
/// and billing usage contribution (used to compute exact `fold.pruned_usage` in tree mode).
#[derive(Debug, Clone)]
pub struct ActiveEntry {
    pub node_id: Option<u64>,
    pub message: IrMessage,
    pub entry_usage: TokenUsage,
}

/// Active conversation window reconstructed from the target branch lineage (or legacy WAL section).
#[derive(Debug, Clone)]
pub struct ReplayedSession {
    pub entries: Vec<ActiveEntry>,
    /// Recorded `usage` for surviving `role: "assistant"` turns in `entries` (keyed by `turn_id`).
    pub assistant_usages: BTreeMap<u64, TokenUsage>,
    /// Exact cross-compaction 5-field cumulative usage along this branch lineage.
    pub session_usage: TokenUsage,
    /// Stable routing affinity identifier persisted on `session_init` / `checkpoint` (v0.1.1).
    pub session_id: Option<String>,
    pub latest_config: Option<WalConfigSnapshot>,
    pub last_checkpoint_turn: u64,
    pub next_turn_id: u64,
    /// True for fresh sessions and v0.3 tree WALs (`id`/`parent_id`); false for legacy pre-v0.3 WALs.
    pub tree_mode: bool,
    /// Active branch tip node ID (`Some` after replay/init in tree mode; `None` if fresh or legacy).
    pub head_id: Option<u64>,
    /// Next file-wide monotonic node ID to allocate (`>= 1`).
    pub next_node_id: u64,
    pub is_fresh: bool,
}

impl ReplayedSession {
    fn fresh() -> Self {
        Self {
            entries: Vec::new(),
            assistant_usages: BTreeMap::new(),
            session_usage: TokenUsage::default(),
            session_id: None,
            latest_config: None,
            last_checkpoint_turn: 0,
            next_turn_id: 1,
            tree_mode: true,
            head_id: None,
            next_node_id: 1,
            is_fresh: true,
        }
    }
}

/// Derive a deterministic UUID-formatted fallback `session_id` for legacy WAL files
/// created before v0.1.1 (when `session_init` / `checkpoint` omitted `session_id`).
pub fn derive_legacy_session_id(wal_path: Option<&Path>) -> String {
    let Some(path) = wal_path else {
        return uuid::Uuid::new_v4().to_string();
    };
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let mut hasher = Sha256::new();
    hasher.update(b"anix-wal-session:");
    hasher.update(canonical.to_string_lossy().as_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    // Set RFC 4122 version 5 & variant bits so strict UUID validators accept it uniformly.
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    uuid::Uuid::from_bytes(bytes).to_string()
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

    /// Reconstruct active context via single-pass reverse-read from EOF (Section 6.2.4).
    ///
    /// - Tree WAL (EOF event has `id`): follows `parent_id` from `requested_leaf.unwrap_or(eof_id)`
    ///   back to the branch's nearest `checkpoint` or `session_init`.
    /// - Legacy pre-v0.3 WAL (EOF event lacks `id`): rejects explicit `--leaf` with exit `2`,
    ///   otherwise replays linearly using `skip_ge` fold pruning.
    pub fn load_active_window(
        &mut self,
        requested_leaf: Option<u64>,
    ) -> Result<ReplayedSession, WalError> {
        let Some(ref mut file) = self.file else {
            if let Some(leaf_id) = requested_leaf {
                return Err(WalError::InvalidLeaf {
                    path: "<stateless>".into(),
                    leaf_id,
                    reason: "--leaf requires a persistent WAL context file (-c / --context)".into(),
                });
            }
            return Ok(ReplayedSession::fresh());
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
            if let Some(leaf_id) = requested_leaf {
                return Err(WalError::LeafNotFound {
                    path: path_str,
                    leaf_id,
                });
            }
            return Ok(ReplayedSession::fresh());
        }

        let mut rev_reader = ReverseLineReader::new(file, file_len);
        let first_event = match next_parsed_wal_event(&mut rev_reader, &path_str, true)? {
            Some(ev) => ev,
            None => {
                if let Some(leaf_id) = requested_leaf {
                    return Err(WalError::LeafNotFound {
                        path: path_str,
                        leaf_id,
                    });
                }
                return Ok(ReplayedSession::fresh());
            }
        };

        if let Some(eof_id) = first_event.id() {
            if eof_id == 0 {
                return Err(WalError::MalformedEntry {
                    path: path_str,
                    reason: "WAL node id must be >= 1".into(),
                });
            }
            replay_tree_wal(
                first_event,
                eof_id,
                &mut rev_reader,
                &path_str,
                requested_leaf,
            )
        } else {
            if let Some(leaf_id) = requested_leaf {
                return Err(WalError::InvalidLeaf {
                    path: path_str,
                    leaf_id,
                    reason: "WAL file uses legacy linear format without tree node IDs".into(),
                });
            }
            replay_legacy_wal(first_event, &mut rev_reader, &path_str)
        }
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

    pub fn append_message(
        &mut self,
        id: Option<u64>,
        parent_id: Option<u64>,
        msg: &IrMessage,
    ) -> Result<(), WalError> {
        self.append_message_with_usage(id, parent_id, msg, None)
    }

    pub fn append_message_with_usage(
        &mut self,
        id: Option<u64>,
        parent_id: Option<u64>,
        msg: &IrMessage,
        usage: Option<TokenUsage>,
    ) -> Result<(), WalError> {
        let ts = posix::timestamp_rfc3339_utc();
        let event = ir_message_to_wal_event(id, parent_id, msg, ts, usage);
        self.append(&event)
    }
}

fn next_parsed_wal_event(
    rev_reader: &mut ReverseLineReader<'_>,
    path_str: &str,
    is_tail: bool,
) -> Result<Option<WalEvent>, WalError> {
    let mut saw_trailing_empty = false;
    while let Some(raw_line) = rev_reader.next_line().map_err(|source| WalError::Io {
        path: path_str.to_owned(),
        source,
    })? {
        if raw_line.is_empty() && is_tail && !saw_trailing_empty {
            // Normal trailing `\n` at EOF.
            saw_trailing_empty = true;
            continue;
        }
        if raw_line.iter().all(|b| b.is_ascii_whitespace()) {
            continue;
        }

        let line_str = match std::str::from_utf8(&raw_line) {
            Ok(s) => s,
            Err(err) => {
                return if is_tail {
                    Err(WalError::CorruptedTail {
                        path: path_str.to_owned(),
                        reason: format!("invalid UTF-8 on last line: {err}"),
                    })
                } else {
                    Err(WalError::MalformedEntry {
                        path: path_str.to_owned(),
                        reason: format!("invalid UTF-8: {err}"),
                    })
                };
            }
        };

        let event: WalEvent = match serde_json::from_str(line_str) {
            Ok(ev) => ev,
            Err(err) => {
                return if is_tail {
                    Err(WalError::CorruptedTail {
                        path: path_str.to_owned(),
                        reason: format!("invalid JSON on last line: {err}"),
                    })
                } else {
                    Err(WalError::MalformedEntry {
                        path: path_str.to_owned(),
                        reason: format!("invalid JSON: {err}"),
                    })
                };
            }
        };

        return Ok(Some(event));
    }

    Ok(None)
}

/// Single-pass reverse-read replay for v0.3 Tree WAL (`id` / `parent_id`).
fn replay_tree_wal(
    first_event: WalEvent,
    eof_id: u64,
    rev_reader: &mut ReverseLineReader<'_>,
    path_str: &str,
    requested_leaf: Option<u64>,
) -> Result<ReplayedSession, WalError> {
    let explicit_leaf = requested_leaf.is_some();
    let target_leaf = requested_leaf.unwrap_or(eof_id);
    let mut want_id: Option<u64> = Some(target_leaf);
    let mut matched_target_leaf = false;

    let mut max_node_id: u64 = eof_id;
    let mut collected_rev: Vec<ActiveEntry> = Vec::new();
    let mut surviving_assistant_usages: BTreeMap<u64, TokenUsage> = BTreeMap::new();
    let mut window_usage = TokenUsage::default();
    let mut checkpoint_base_usage = TokenUsage::default();
    let mut session_id: Option<String> = None;
    let mut latest_config: Option<WalConfigSnapshot> = None;
    let mut last_checkpoint_turn: u64 = 0;
    let mut max_turn_id: u64 = 0;
    let mut found_section_header = false;

    let mut pending_event = Some(first_event);
    while let Some(event) = match pending_event.take() {
        Some(ev) => Some(ev),
        None => next_parsed_wal_event(rev_reader, path_str, false)?,
    } {
        let Some(ev_id) = event.id() else {
            return Err(WalError::MalformedEntry {
                path: path_str.to_owned(),
                reason: "tree WAL contains a line missing `id`".into(),
            });
        };
        if ev_id == 0 {
            return Err(WalError::MalformedEntry {
                path: path_str.to_owned(),
                reason: "WAL node id must be >= 1".into(),
            });
        }
        max_node_id = max_node_id.max(ev_id);

        if Some(ev_id) != want_id {
            // Sibling branch or unrelated descendant: skip in O(1), even across checkpoints.
            continue;
        }

        let ev_parent = event.parent_id();
        if let Some(pid) = ev_parent {
            if pid == 0 || pid >= ev_id {
                return Err(WalError::MalformedEntry {
                    path: path_str.to_owned(),
                    reason: format!(
                        "WAL node {ev_id} has non-ancestor parent_id {pid} (must satisfy 1 <= parent_id < id)"
                    ),
                });
            }
        }

        let is_target_node = !matched_target_leaf;
        matched_target_leaf = true;

        if explicit_leaf
            && is_target_node
            && matches!(
                event,
                WalEvent::SessionInit { .. } | WalEvent::Checkpoint { .. }
            )
        {
            return Err(WalError::InvalidLeaf {
                path: path_str.to_owned(),
                leaf_id: target_leaf,
                reason: "cannot branch from a section header (`session_init` or `checkpoint`); select a message, runtime_update, or fold node".into(),
            });
        }

        match event {
            WalEvent::SessionInit {
                ir_version,
                session_id: sid,
                config,
                ..
            } => {
                if ir_version != IR_VERSION {
                    return Err(WalError::IncompatibleVersion(ir_version));
                }
                session_id = sid.filter(|s| !s.trim().is_empty());
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
                session_id: sid,
                config,
                cumulative_usage,
                ..
            } => {
                if ir_version != IR_VERSION {
                    return Err(WalError::IncompatibleVersion(ir_version));
                }
                session_id = sid.filter(|s| !s.trim().is_empty());
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
                pruned_usage,
                ..
            } => {
                max_turn_id = max_turn_id.max(to_turn).max(from_turn);
                let mut entry_usage = TokenUsage::default();
                if let Some(ref fu) = fold_usage {
                    window_usage.add_assign(fu);
                    entry_usage.add_assign(fu);
                }
                if let Some(ref pu) = pruned_usage {
                    window_usage.add_assign(pu);
                    entry_usage.add_assign(pu);
                }
                collected_rev.push(ActiveEntry {
                    node_id: Some(ev_id),
                    message: IrMessage::user(
                        from_turn,
                        format!("<folded turns=\"{from_turn}..{to_turn}\">{summary}</folded>"),
                    ),
                    entry_usage,
                });
                want_id = ev_parent;
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
                collected_rev.push(ActiveEntry {
                    node_id: Some(ev_id),
                    message: IrMessage::system(
                        turn_id,
                        format!("<runtime_update>{content}</runtime_update>"),
                    ),
                    entry_usage: TokenUsage::default(),
                });
                want_id = ev_parent;
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
                if role == WalRole::Assistant
                    && let Some(ref u) = usage
                {
                    window_usage.add_assign(u);
                }

                match role {
                    WalRole::System => {
                        collected_rev.push(ActiveEntry {
                            node_id: Some(ev_id),
                            message: IrMessage::system(turn_id, text.unwrap_or_default()),
                            entry_usage: TokenUsage::default(),
                        });
                    }
                    WalRole::User => {
                        collected_rev.push(ActiveEntry {
                            node_id: Some(ev_id),
                            message: IrMessage::User {
                                turn_id,
                                text: text.unwrap_or_default(),
                                media,
                                extra: BTreeMap::new(),
                            },
                            entry_usage: TokenUsage::default(),
                        });
                    }
                    WalRole::Tool => {
                        collected_rev.push(ActiveEntry {
                            node_id: Some(ev_id),
                            message: IrMessage::Tool {
                                turn_id,
                                tool_call_id: tool_call_id.unwrap_or_default(),
                                is_error,
                                text: text.unwrap_or_default(),
                                media,
                                extra: BTreeMap::new(),
                            },
                            entry_usage: TokenUsage::default(),
                        });
                    }
                    WalRole::Assistant => {
                        let has_text = text.as_deref().is_some_and(|t| !t.is_empty());
                        if !has_text && tool_calls.is_empty() {
                            if explicit_leaf && is_target_node {
                                return Err(WalError::InvalidLeaf {
                                    path: path_str.to_owned(),
                                    leaf_id: target_leaf,
                                    reason: "node is an empty assistant turn with neither text nor tool_calls".into(),
                                });
                            }
                            want_id = ev_parent;
                            continue;
                        }

                        if !tool_calls.is_empty()
                            && !assistant_tool_calls_are_closed_entries(&collected_rev, &tool_calls)
                        {
                            if explicit_leaf {
                                return Err(WalError::InvalidLeaf {
                                    path: path_str.to_owned(),
                                    leaf_id: target_leaf,
                                    reason: format!(
                                        "breaks tool-call closure (assistant node {ev_id} / turn {turn_id} has unclosed tool_calls)"
                                    ),
                                });
                            }
                            let call_ids: HashSet<&str> =
                                tool_calls.iter().map(|tc| tc.id.as_str()).collect();
                            while let Some(ActiveEntry {
                                message: IrMessage::Tool { tool_call_id, .. },
                                ..
                            }) = collected_rev.last()
                            {
                                if call_ids.contains(tool_call_id.as_str()) {
                                    collected_rev.pop();
                                } else {
                                    break;
                                }
                            }
                            want_id = ev_parent;
                            continue;
                        }

                        if let Some(u) = usage {
                            surviving_assistant_usages.insert(turn_id, u);
                        }
                        collected_rev.push(ActiveEntry {
                            node_id: Some(ev_id),
                            message: IrMessage::Assistant {
                                turn_id,
                                text,
                                reasoning,
                                tool_calls,
                                provider_opaque,
                                extra: BTreeMap::new(),
                            },
                            entry_usage: usage.unwrap_or_default(),
                        });
                    }
                }
                want_id = ev_parent;
            }
        }
    }

    if !matched_target_leaf {
        return Err(WalError::LeafNotFound {
            path: path_str.to_owned(),
            leaf_id: target_leaf,
        });
    }

    if !found_section_header {
        return Err(WalError::MalformedEntry {
            path: path_str.to_owned(),
            reason: "broken parent_id chain: missing `session_init` or `checkpoint` ancestor"
                .into(),
        });
    }

    collected_rev.reverse();

    let before_orphan_len = collected_rev.len();
    let entries = drop_orphan_tool_entries(collected_rev);
    if explicit_leaf && entries.len() != before_orphan_len {
        return Err(WalError::InvalidLeaf {
            path: path_str.to_owned(),
            leaf_id: target_leaf,
            reason: "branch contains orphaned tool message without matching assistant tool_calls"
                .into(),
        });
    }
    if explicit_leaf && entries.is_empty() {
        return Err(WalError::InvalidLeaf {
            path: path_str.to_owned(),
            leaf_id: target_leaf,
            reason: "selected leaf yields an empty active context".into(),
        });
    }

    let surviving_ids: HashSet<u64> = entries
        .iter()
        .filter_map(|e| match &e.message {
            IrMessage::Assistant { turn_id, .. } => Some(*turn_id),
            _ => None,
        })
        .collect();
    surviving_assistant_usages.retain(|tid, _| surviving_ids.contains(tid));

    let mut session_usage = checkpoint_base_usage;
    session_usage.add_assign(&window_usage);

    let head_id = entries.last().and_then(|e| e.node_id);

    Ok(ReplayedSession {
        entries,
        assistant_usages: surviving_assistant_usages,
        session_usage,
        session_id,
        latest_config,
        last_checkpoint_turn,
        next_turn_id: max_turn_id.saturating_add(1).max(1),
        tree_mode: true,
        head_id,
        next_node_id: max_node_id.saturating_add(1).max(1),
        is_fresh: false,
    })
}

/// Legacy linear single-pass reverse-read replay for pre-v0.3 WAL files without `id` / `parent_id`.
fn replay_legacy_wal(
    first_event: WalEvent,
    rev_reader: &mut ReverseLineReader<'_>,
    path_str: &str,
) -> Result<ReplayedSession, WalError> {
    let mut skip_ge: u64 = u64::MAX;
    let mut collected_rev: Vec<ActiveEntry> = Vec::new();
    let mut surviving_assistant_usages: BTreeMap<u64, TokenUsage> = BTreeMap::new();
    let mut window_usage = TokenUsage::default();
    let mut checkpoint_base_usage = TokenUsage::default();
    let mut session_id: Option<String> = None;
    let mut latest_config: Option<WalConfigSnapshot> = None;
    let mut last_checkpoint_turn: u64 = 0;
    let mut max_turn_id: u64 = 0;
    let mut found_section_header = false;

    let mut pending_event = Some(first_event);
    while let Some(event) = match pending_event.take() {
        Some(ev) => Some(ev),
        None => next_parsed_wal_event(rev_reader, path_str, false)?,
    } {
        match event {
            WalEvent::SessionInit {
                ir_version,
                session_id: sid,
                config,
                ..
            } => {
                if ir_version != IR_VERSION {
                    return Err(WalError::IncompatibleVersion(ir_version));
                }
                session_id = sid.filter(|s| !s.trim().is_empty());
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
                session_id: sid,
                config,
                cumulative_usage,
                ..
            } => {
                if ir_version != IR_VERSION {
                    return Err(WalError::IncompatibleVersion(ir_version));
                }
                session_id = sid.filter(|s| !s.trim().is_empty());
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
                    collected_rev.push(ActiveEntry {
                        node_id: None,
                        message: IrMessage::user(
                            from_turn,
                            format!("<folded turns=\"{from_turn}..{to_turn}\">{summary}</folded>"),
                        ),
                        entry_usage: fold_usage.unwrap_or_default(),
                    });
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
                    collected_rev.push(ActiveEntry {
                        node_id: None,
                        message: IrMessage::system(
                            turn_id,
                            format!("<runtime_update>{content}</runtime_update>"),
                        ),
                        entry_usage: TokenUsage::default(),
                    });
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
                        collected_rev.push(ActiveEntry {
                            node_id: None,
                            message: IrMessage::system(turn_id, text.unwrap_or_default()),
                            entry_usage: TokenUsage::default(),
                        });
                    }
                    WalRole::User => {
                        collected_rev.push(ActiveEntry {
                            node_id: None,
                            message: IrMessage::User {
                                turn_id,
                                text: text.unwrap_or_default(),
                                media,
                                extra: BTreeMap::new(),
                            },
                            entry_usage: TokenUsage::default(),
                        });
                    }
                    WalRole::Tool => {
                        collected_rev.push(ActiveEntry {
                            node_id: None,
                            message: IrMessage::Tool {
                                turn_id,
                                tool_call_id: tool_call_id.unwrap_or_default(),
                                is_error,
                                text: text.unwrap_or_default(),
                                media,
                                extra: BTreeMap::new(),
                            },
                            entry_usage: TokenUsage::default(),
                        });
                    }
                    WalRole::Assistant => {
                        let has_text = text.as_deref().is_some_and(|t| !t.is_empty());
                        if !has_text && tool_calls.is_empty() {
                            continue;
                        }

                        if !tool_calls.is_empty()
                            && !assistant_tool_calls_are_closed_entries(&collected_rev, &tool_calls)
                        {
                            let call_ids: HashSet<&str> =
                                tool_calls.iter().map(|tc| tc.id.as_str()).collect();
                            while let Some(ActiveEntry {
                                message: IrMessage::Tool { tool_call_id, .. },
                                ..
                            }) = collected_rev.last()
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
                        collected_rev.push(ActiveEntry {
                            node_id: None,
                            message: IrMessage::Assistant {
                                turn_id,
                                text,
                                reasoning,
                                tool_calls,
                                provider_opaque,
                                extra: BTreeMap::new(),
                            },
                            entry_usage: usage.unwrap_or_default(),
                        });
                    }
                }
            }
        }
    }

    if !found_section_header {
        return Err(WalError::MalformedEntry {
            path: path_str.to_owned(),
            reason: "missing `session_init` or `checkpoint` section header".into(),
        });
    }

    collected_rev.reverse();

    let entries = drop_orphan_tool_entries(collected_rev);

    let surviving_ids: HashSet<u64> = entries
        .iter()
        .filter_map(|e| match &e.message {
            IrMessage::Assistant { turn_id, .. } => Some(*turn_id),
            _ => None,
        })
        .collect();
    surviving_assistant_usages.retain(|tid, _| surviving_ids.contains(tid));

    let mut session_usage = checkpoint_base_usage;
    session_usage.add_assign(&window_usage);

    Ok(ReplayedSession {
        entries,
        assistant_usages: surviving_assistant_usages,
        session_usage,
        session_id,
        latest_config,
        last_checkpoint_turn,
        next_turn_id: max_turn_id.saturating_add(1).max(1),
        tree_mode: false,
        head_id: None,
        next_node_id: 1,
        is_fresh: false,
    })
}

/// Convert an [`IrMessage`] into its corresponding [`WalEvent::Message`].
pub fn ir_message_to_wal_event(
    id: Option<u64>,
    parent_id: Option<u64>,
    msg: &IrMessage,
    ts: String,
    usage: Option<TokenUsage>,
) -> WalEvent {
    match msg {
        IrMessage::System { turn_id, text, .. } => WalEvent::Message {
            id,
            parent_id,
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
            id,
            parent_id,
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
            id,
            parent_id,
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
            id,
            parent_id,
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

    if let Some(idx) = messages.iter().position(|m| m.turn_id() >= aligned) {
        if matches!(messages[idx], IrMessage::Tool { .. }) {
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

fn assistant_tool_calls_are_closed_entries(
    collected_rev: &[ActiveEntry],
    tool_calls: &[ToolCall],
) -> bool {
    let expected: HashSet<&str> = tool_calls.iter().map(|tc| tc.id.as_str()).collect();
    let mut seen: HashSet<&str> = HashSet::with_capacity(expected.len());

    for entry in collected_rev.iter().rev() {
        match &entry.message {
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

fn drop_orphan_tool_entries(entries: Vec<ActiveEntry>) -> Vec<ActiveEntry> {
    let mut out = Vec::with_capacity(entries.len());
    let mut pending_calls: HashSet<String> = HashSet::new();
    for entry in entries {
        match &entry.message {
            IrMessage::Assistant { tool_calls, .. } => {
                pending_calls.clear();
                for tc in tool_calls {
                    pending_calls.insert(tc.id.clone());
                }
                out.push(entry);
            }
            IrMessage::Tool { tool_call_id, .. } => {
                if pending_calls.remove(tool_call_id) {
                    out.push(entry);
                }
            }
            _ => {
                pending_calls.clear();
                out.push(entry);
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
    use anix_proto::ir::{HeaderTag, IrHeader, IrMeta, IrStream, StderrMode};

    fn dummy_header() -> IrHeader {
        IrHeader {
            ir_version: IR_VERSION,
            record_type: HeaderTag::Header,
            tools: Vec::new(),
            max_output_tokens: None,
            params: BTreeMap::new(),
            meta: IrMeta {
                output_mode: None,
                stderr_mode: StderrMode::Auto,
                cwd: "/tmp".into(),
                turn_budget: None,
                extra: BTreeMap::new(),
            },
            extra: BTreeMap::new(),
        }
    }

    #[test]
    fn replay_legacy_skips_empty_assistant_and_prunes_unclosed_tool_calls_preserving_usage() {
        let tmp =
            std::env::temp_dir().join(format!("anix-wal-test-{}.jsonl", uuid::Uuid::new_v4()));
        {
            let mut store = WalStore::open(&tmp).expect("open wal");
            store
                .append(&WalEvent::SessionInit {
                    id: None,
                    ir_version: IR_VERSION,
                    ts: "2026-11-25T00:00:00Z".into(),
                    session_id: None,
                    config: WalConfigSnapshot::default(),
                })
                .unwrap();
            store
                .append_message(None, None, &IrMessage::system(0, "sys"))
                .unwrap();
            store
                .append_message(None, None, &IrMessage::user(1, "hello"))
                .unwrap();
            store
                .append_message_with_usage(
                    None,
                    None,
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
            store
                .append_message_with_usage(
                    None,
                    None,
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
                .append_message(
                    None,
                    None,
                    &IrMessage::Tool {
                        turn_id: 4,
                        tool_call_id: "c1".into(),
                        is_error: false,
                        text: "[t4] exit=0\n".into(),
                        media: Vec::new(),
                        extra: BTreeMap::new(),
                    },
                )
                .unwrap();
        }

        let mut store = WalStore::open(&tmp).expect("reopen wal");
        // Explicit `--leaf` on legacy WAL must fail with UsageError (exit 2).
        let leaf_err = store
            .load_active_window(Some(1))
            .expect_err("legacy WAL must reject --leaf");
        assert_eq!(leaf_err.exit_code(), ExitCode::UsageError);

        let replayed = store.load_active_window(None).expect("replay legacy wal");
        let _ = std::fs::remove_file(&tmp);

        assert!(!replayed.tree_mode);
        assert_eq!(replayed.head_id, None);
        assert_eq!(replayed.entries.len(), 2);
        assert!(matches!(
            replayed.entries[0].message,
            IrMessage::System { turn_id: 0, .. }
        ));
        assert!(matches!(
            replayed.entries[1].message,
            IrMessage::User { turn_id: 1, .. }
        ));
        assert!(replayed.assistant_usages.is_empty());

        assert_eq!(replayed.session_usage.input_tokens, 300);
        assert_eq!(replayed.session_usage.output_tokens, 60);
        assert_eq!(replayed.session_usage.reasoning(), 20);

        let stream = IrStream {
            header: dummy_header(),
            messages: replayed.entries.into_iter().map(|e| e.message).collect(),
        };
        stream
            .validate_invariants()
            .expect("replayed stream must satisfy invariants");
    }

    #[test]
    fn replay_tree_branches_folds_checkpoints_and_enforces_strict_leaf_validation() {
        let tmp =
            std::env::temp_dir().join(format!("anix-tree-wal-{}.jsonl", uuid::Uuid::new_v4()));
        {
            let mut store = WalStore::open(&tmp).expect("open wal");
            // 1: session_init -> 2: system -> 3: user("root")
            store
                .append(&WalEvent::SessionInit {
                    id: Some(1),
                    ir_version: IR_VERSION,
                    ts: "2026-11-26T00:00:00Z".into(),
                    session_id: Some("sid-tree-001".into()),
                    config: WalConfigSnapshot::default(),
                })
                .unwrap();
            store
                .append_message(Some(2), Some(1), &IrMessage::system(0, "sys"))
                .unwrap();
            store
                .append_message(Some(3), Some(2), &IrMessage::user(1, "root prompt"))
                .unwrap();

            // Branch A: 4: assistant([c1, c2]) -> 5: tool(c1) -> 6: tool(c2) -> 7: fold(parent=3)
            store
                .append_message_with_usage(
                    Some(4),
                    Some(3),
                    &IrMessage::Assistant {
                        turn_id: 2,
                        text: None,
                        reasoning: None,
                        tool_calls: vec![
                            ToolCall {
                                id: "c1".into(),
                                name: "msh".into(),
                                arguments: serde_json::json!({"cmd": "echo 1"}),
                            },
                            ToolCall {
                                id: "c2".into(),
                                name: "msh".into(),
                                arguments: serde_json::json!({"cmd": "echo 2"}),
                            },
                        ],
                        provider_opaque: None,
                        extra: BTreeMap::new(),
                    },
                    Some(TokenUsage {
                        input_tokens: 500,
                        output_tokens: 50,
                        cached_tokens: Some(0),
                        cache_write_tokens: Some(0),
                        reasoning_tokens: Some(0),
                    }),
                )
                .unwrap();
            store
                .append_message(
                    Some(5),
                    Some(4),
                    &IrMessage::Tool {
                        turn_id: 3,
                        tool_call_id: "c1".into(),
                        is_error: false,
                        text: "[t3] exit=0\n1".into(),
                        media: Vec::new(),
                        extra: BTreeMap::new(),
                    },
                )
                .unwrap();
            store
                .append_message(
                    Some(6),
                    Some(5),
                    &IrMessage::Tool {
                        turn_id: 4,
                        tool_call_id: "c2".into(),
                        is_error: false,
                        text: "[t4] exit=0\n2".into(),
                        media: Vec::new(),
                        extra: BTreeMap::new(),
                    },
                )
                .unwrap();
            store
                .append(&WalEvent::Fold {
                    id: Some(7),
                    parent_id: Some(3),
                    ts: "2026-11-26T00:01:00Z".into(),
                    from_turn: 2,
                    to_turn: 4,
                    summary: "branch A failed".into(),
                    fold_usage: Some(TokenUsage {
                        input_tokens: 60,
                        output_tokens: 15,
                        cached_tokens: Some(0),
                        cache_write_tokens: Some(0),
                        reasoning_tokens: Some(0),
                    }),
                    pruned_usage: Some(TokenUsage {
                        input_tokens: 500,
                        output_tokens: 50,
                        cached_tokens: Some(0),
                        cache_write_tokens: Some(0),
                        reasoning_tokens: Some(0),
                    }),
                })
                .unwrap();
            // Checkpoint on Branch A: 8: checkpoint(parent=7) -> 9: system -> 10: user(<checkpoint>)
            store
                .append(&WalEvent::Checkpoint {
                    id: Some(8),
                    parent_id: Some(7),
                    ir_version: IR_VERSION,
                    turn_id: 5,
                    ts: "2026-11-26T00:02:00Z".into(),
                    session_id: Some("sid-tree-001".into()),
                    config: WalConfigSnapshot::default(),
                    compact_usage: None,
                    cumulative_usage: Some(TokenUsage {
                        input_tokens: 600,
                        output_tokens: 80,
                        cached_tokens: Some(0),
                        cache_write_tokens: Some(0),
                        reasoning_tokens: Some(0),
                    }),
                })
                .unwrap();
            store
                .append_message(Some(9), Some(8), &IrMessage::system(5, "sys2"))
                .unwrap();
            store
                .append_message(
                    Some(10),
                    Some(9),
                    &IrMessage::user(6, "<checkpoint>\nsum\n</checkpoint>"),
                )
                .unwrap();
        }

        let mut store = WalStore::open(&tmp).expect("reopen wal");

        // 1. Default leaf (10, post-checkpoint): stops at checkpoint 8, preserves session_id & cumulative usage.
        let r_default = store.load_active_window(None).expect("default replay");
        assert!(r_default.tree_mode);
        assert_eq!(r_default.head_id, Some(10));
        assert_eq!(r_default.next_node_id, 11);
        assert_eq!(r_default.session_id.as_deref(), Some("sid-tree-001"));
        assert_eq!(r_default.entries.len(), 2);
        assert_eq!(r_default.session_usage.input_tokens, 600);

        // 2. `--leaf 7` (fold node BEFORE checkpoint 8): skips checkpoint 8 and pruned nodes 4..6, preserves 500+60 usage!
        let r_fold = store.load_active_window(Some(7)).expect("replay fold leaf");
        assert_eq!(r_fold.head_id, Some(7));
        assert_eq!(r_fold.next_node_id, 11);
        assert_eq!(r_fold.entries.len(), 3); // system(2), user(3), folded(7)
        assert_eq!(r_fold.session_usage.input_tokens, 560);
        assert_eq!(r_fold.session_usage.output_tokens, 65);

        // 3. `--leaf 6` (completed tool batch on abandoned branch BEFORE fold & checkpoint): valid!
        let r_tool_done = store
            .load_active_window(Some(6))
            .expect("replay completed tool batch");
        assert_eq!(r_tool_done.head_id, Some(6));
        assert_eq!(r_tool_done.entries.len(), 5); // sys(2), user(3), asst(4), tool(5), tool(6)
        assert_eq!(r_tool_done.session_usage.input_tokens, 500);

        // 4. `--leaf 3` (pre-branch user prompt): zero sibling usage leakage!
        let r_root_user = store.load_active_window(Some(3)).expect("replay user leaf");
        assert_eq!(r_root_user.head_id, Some(3));
        assert_eq!(r_root_user.entries.len(), 2); // sys(2), user(3)
        assert_eq!(r_root_user.session_usage.input_tokens, 0);

        // 5. Invalid explicit leaves MUST fail with UsageError (exit 2):
        for bad_leaf in [1, 4, 5, 8, 999] {
            let err = store
                .load_active_window(Some(bad_leaf))
                .expect_err(&format!("leaf {bad_leaf} must be rejected"));
            assert_eq!(
                err.exit_code(),
                ExitCode::UsageError,
                "leaf {bad_leaf} error={err}"
            );
        }

        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn replay_tree_implicit_crash_recovery_heals_head_id_to_last_surviving_node() {
        let tmp =
            std::env::temp_dir().join(format!("anix-tree-crash-{}.jsonl", uuid::Uuid::new_v4()));
        {
            let mut store = WalStore::open(&tmp).expect("open wal");
            store
                .append(&WalEvent::SessionInit {
                    id: Some(1),
                    ir_version: IR_VERSION,
                    ts: "2026-11-26T00:00:00Z".into(),
                    session_id: Some("sid-crash".into()),
                    config: WalConfigSnapshot::default(),
                })
                .unwrap();
            store
                .append_message(Some(2), Some(1), &IrMessage::system(0, "sys"))
                .unwrap();
            store
                .append_message(Some(3), Some(2), &IrMessage::user(1, "hello"))
                .unwrap();
            // Crashed mid-tool-batch (only c1 completed, c2 missing):
            store
                .append_message_with_usage(
                    Some(4),
                    Some(3),
                    &IrMessage::Assistant {
                        turn_id: 2,
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
                        input_tokens: 250,
                        output_tokens: 30,
                        cached_tokens: None,
                        cache_write_tokens: None,
                        reasoning_tokens: None,
                    }),
                )
                .unwrap();
            store
                .append_message(
                    Some(5),
                    Some(4),
                    &IrMessage::Tool {
                        turn_id: 3,
                        tool_call_id: "c1".into(),
                        is_error: false,
                        text: "[t3] exit=0\n".into(),
                        media: Vec::new(),
                        extra: BTreeMap::new(),
                    },
                )
                .unwrap();
        }

        let mut store = WalStore::open(&tmp).expect("reopen wal");
        let replayed = store
            .load_active_window(None)
            .expect("implicit crash recovery");
        let _ = std::fs::remove_file(&tmp);

        assert!(replayed.tree_mode);
        // `head_id` MUST heal back to node 3 so the next append forks cleanly past crashed 4 & 5!
        assert_eq!(replayed.head_id, Some(3));
        assert_eq!(replayed.next_node_id, 6);
        assert_eq!(replayed.entries.len(), 2);
        assert_eq!(replayed.session_usage.input_tokens, 250);
    }

    #[test]
    fn replay_restores_session_id_across_checkpoint_and_derives_stable_legacy_fallback() {
        let tmp = std::env::temp_dir().join(format!("anix-wal-sid-{}.jsonl", uuid::Uuid::new_v4()));
        {
            let mut store = WalStore::open(&tmp).expect("open wal");
            store
                .append(&WalEvent::SessionInit {
                    id: Some(1),
                    ir_version: IR_VERSION,
                    ts: "2026-11-26T00:00:00Z".into(),
                    session_id: Some("sid-persisted-001".into()),
                    config: WalConfigSnapshot::default(),
                })
                .unwrap();
            store
                .append_message(Some(2), Some(1), &IrMessage::system(0, "sys"))
                .unwrap();
            store
                .append(&WalEvent::Checkpoint {
                    id: Some(3),
                    parent_id: Some(2),
                    ir_version: IR_VERSION,
                    turn_id: 5,
                    ts: "2026-11-26T00:05:00Z".into(),
                    session_id: Some("sid-persisted-001".into()),
                    config: WalConfigSnapshot::default(),
                    compact_usage: None,
                    cumulative_usage: None,
                })
                .unwrap();
            store
                .append_message(Some(4), Some(3), &IrMessage::system(5, "sys2"))
                .unwrap();
            store
                .append_message(
                    Some(5),
                    Some(4),
                    &IrMessage::user(6, "<checkpoint>\nok\n</checkpoint>"),
                )
                .unwrap();
        }

        let mut store = WalStore::open(&tmp).expect("reopen wal");
        let replayed = store.load_active_window(None).expect("replay wal");
        assert_eq!(replayed.session_id.as_deref(), Some("sid-persisted-001"));
        assert_eq!(replayed.head_id, Some(5));
        assert_eq!(replayed.next_node_id, 6);

        let legacy1 = derive_legacy_session_id(Some(&tmp));
        let legacy2 = derive_legacy_session_id(Some(&tmp));
        assert_eq!(legacy1, legacy2);
        assert_eq!(legacy1.len(), 36);
        let _ = std::fs::remove_file(&tmp);
    }
}
