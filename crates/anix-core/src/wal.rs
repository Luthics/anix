//! Append-Only Tree WAL JSONL persistence with `flock(LOCK_EX)` / `--lock-wait`,
//! `fsync` durability, single-pass reverse-read lineage replay (`id` / `parent_id` + `--leaf <id>`),
//! cross-file read-only `branch_base` (`anix fork`), `anix fsck`, `anix log`,
//! tail corruption Fail-Closed, Tool-Pair Closure, and legacy pre-v0.3 linear WAL fallback (RFC §5.2).

use std::collections::{BTreeMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anix_proto::IR_VERSION;
use anix_proto::exit_code::ExitCode;
use anix_proto::ir::{IrMessage, ToolCall};
use anix_proto::provider::TokenUsage;
use anix_proto::wal::{WalConfigSnapshot, WalEvent, WalRole};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::platform::posix;
use crate::signal::SignalState;

const REVERSE_CHUNK_SIZE: usize = 64 * 1024;
const LOCK_WAIT_POLL_SLICE: Duration = Duration::from_millis(25);

#[derive(Debug, Error)]
pub enum WalError {
    #[error("WAL context file `{0}` is locked by another running agent process")]
    Locked(String),
    #[error("timed out after {timeout_sec}s waiting for WAL lock on `{path}`")]
    LockTimeout { path: String, timeout_sec: u64 },
    #[error("interrupted while waiting for WAL lock")]
    Interrupted(ExitCode),
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
            Self::LockTimeout { .. } => ExitCode::TimeoutOrBudget,
            Self::Interrupted(code) => *code,
            Self::IncompatibleVersion(_) | Self::LeafNotFound { .. } | Self::InvalidLeaf { .. } => {
                ExitCode::UsageError
            }
            _ => ExitCode::RuntimeError,
        }
    }
}

/// Single surviving entry in the active conversation window.
#[derive(Debug, Clone)]
pub struct ActiveEntry {
    pub node_id: Option<u64>,
    pub message: IrMessage,
    pub entry_usage: TokenUsage,
}

/// Active conversation window reconstructed from the target branch lineage (including across
/// read-only `branch_base` parent files) or legacy linear WAL section.
#[derive(Debug, Clone)]
pub struct ReplayedSession {
    pub entries: Vec<ActiveEntry>,
    pub assistant_usages: BTreeMap<u64, TokenUsage>,
    pub session_usage: TokenUsage,
    pub session_id: Option<String>,
    pub latest_config: Option<WalConfigSnapshot>,
    pub last_checkpoint_turn: u64,
    pub next_turn_id: u64,
    pub tree_mode: bool,
    pub head_id: Option<u64>,
    pub next_node_id: u64,
    /// Maildir `message_id`s committed on this lineage (for startup `cur/` deduplication).
    pub committed_message_ids: HashSet<String>,
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
            committed_message_ids: HashSet::new(),
            is_fresh: true,
        }
    }
}

/// Derive a deterministic UUID-formatted fallback `session_id` for legacy WAL files.
pub fn derive_legacy_session_id(wal_path: Option<&Path>) -> String {
    let Some(path) = wal_path else {
        return uuid::Uuid::new_v4().to_string();
    };
    let canonical = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let mut hasher = Sha256::new();
    hasher.update(b"anix-wal-session:");
    hasher.update(canonical.to_string_lossy().as_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
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
    pub fn stateless() -> Self {
        Self {
            path: None,
            file: None,
        }
    }

    /// Open (or create with mode `0600`) a WAL file with non-blocking `flock(LOCK_EX | LOCK_NB)`.
    pub fn open(path: &Path) -> Result<Self, WalError> {
        let file = open_wal_file_rw(path)?;
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

    /// Open a WAL file honoring optional `--lock-wait[=<SEC>]` (interruptible by signals; exit 124 on timeout).
    pub fn open_with_lock_wait(
        path: &Path,
        lock_wait: Option<Option<u64>>,
        signals: &SignalState,
    ) -> Result<Self, WalError> {
        let Some(timeout_opt) = lock_wait else {
            return Self::open(path);
        };

        let file = open_wal_file_rw(path)?;
        let deadline = timeout_opt.map(|sec| Instant::now() + Duration::from_secs(sec));

        loop {
            if let Some(code) = signals.termination_exit_code() {
                return Err(WalError::Interrupted(code));
            }
            let acquired =
                posix::flock_exclusive_nonblocking(file.as_raw_fd()).map_err(|source| {
                    WalError::Io {
                        path: path.display().to_string(),
                        source,
                    }
                })?;
            if acquired {
                return Ok(Self {
                    path: Some(path.to_path_buf()),
                    file: Some(file),
                });
            }

            if let Some(dl) = deadline
                && Instant::now() >= dl
            {
                return Err(WalError::LockTimeout {
                    path: path.display().to_string(),
                    timeout_sec: timeout_opt.unwrap_or(0),
                });
            }

            let wait_dur = match deadline {
                Some(dl) => dl
                    .saturating_duration_since(Instant::now())
                    .min(LOCK_WAIT_POLL_SLICE),
                None => LOCK_WAIT_POLL_SLICE,
            };
            if wait_dur.is_zero() {
                continue;
            }
            if posix::sleep_or_signal(wait_dur, signals.wakeup_fd()) {
                signals.drain_wakeup();
            }
        }
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Reconstruct active context via single-pass reverse-read from EOF (including across `branch_base`).
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
        let path = self.path.as_deref().unwrap_or_else(|| Path::new("<wal>"));
        let mut visited = HashSet::new();
        replay_open_file(file, path, requested_leaf, &mut visited)
    }

    /// Append a single WAL event line atomically, flush, and `fsync` (`sync_data`) for crash durability.
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
            .and_then(|_| file.sync_data())
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
        self.append_message_with_meta(id, parent_id, msg, None, Vec::new())
    }

    pub fn append_message_with_usage(
        &mut self,
        id: Option<u64>,
        parent_id: Option<u64>,
        msg: &IrMessage,
        usage: Option<TokenUsage>,
    ) -> Result<(), WalError> {
        self.append_message_with_meta(id, parent_id, msg, usage, Vec::new())
    }

    pub fn append_message_with_meta(
        &mut self,
        id: Option<u64>,
        parent_id: Option<u64>,
        msg: &IrMessage,
        usage: Option<TokenUsage>,
        message_ids: Vec<String>,
    ) -> Result<(), WalError> {
        let ts = posix::timestamp_rfc3339_utc();
        let event = ir_message_to_wal_event(id, parent_id, msg, ts, usage, message_ids);
        self.append(&event)
    }
}

fn open_wal_file_rw(path: &Path) -> Result<File, WalError> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
        && !parent.exists()
    {
        fs::create_dir_all(parent).map_err(|source| WalError::Io {
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
    Ok(file)
}

/// Read-only replay of a WAL file (no write `flock`, used by `branch_base`, `fork`, `fsck`, `log`).
pub fn replay_file_readonly(
    path: &Path,
    requested_leaf: Option<u64>,
    visited: &mut HashSet<(PathBuf, u64)>,
) -> Result<ReplayedSession, WalError> {
    let mut file = File::open(path).map_err(|source| WalError::Io {
        path: path.display().to_string(),
        source,
    })?;
    replay_open_file(&mut file, path, requested_leaf, visited)
}

fn replay_open_file(
    file: &mut File,
    path: &Path,
    requested_leaf: Option<u64>,
    visited: &mut HashSet<(PathBuf, u64)>,
) -> Result<ReplayedSession, WalError> {
    let path_str = path.display().to_string();
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
        let target_leaf = requested_leaf.unwrap_or(eof_id);
        let canon = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        if !visited.insert((canon, target_leaf)) {
            return Err(WalError::MalformedEntry {
                path: path_str,
                reason: format!("cycle detected in branch_base chain at leaf {target_leaf}"),
            });
        }
        replay_tree_wal(
            first_event,
            eof_id,
            &mut rev_reader,
            &path_str,
            requested_leaf,
            visited,
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

fn parse_branch_base_ref(base: &str, path_str: &str) -> Result<(PathBuf, u64), WalError> {
    let Some((parent_path_str, leaf_str)) = base.rsplit_once('#') else {
        return Err(WalError::MalformedEntry {
            path: path_str.to_owned(),
            reason: format!("invalid branch_base `{base}` (expected `<abs_path>#<leaf_id>`)"),
        });
    };
    if parent_path_str.is_empty() {
        return Err(WalError::MalformedEntry {
            path: path_str.to_owned(),
            reason: format!("invalid branch_base `{base}` (empty parent path)"),
        });
    }
    let leaf_id = leaf_str
        .parse::<u64>()
        .map_err(|_| WalError::MalformedEntry {
            path: path_str.to_owned(),
            reason: format!("invalid branch_base leaf ID in `{base}`"),
        })?;
    if leaf_id == 0 {
        return Err(WalError::MalformedEntry {
            path: path_str.to_owned(),
            reason: format!("invalid branch_base leaf ID 0 in `{base}`"),
        });
    }
    Ok((PathBuf::from(parent_path_str), leaf_id))
}

/// Single-pass reverse-read replay for Tree WAL (`id` / `parent_id`), transparently
/// following cross-file `branch_base` references read-only.
fn replay_tree_wal(
    first_event: WalEvent,
    eof_id: u64,
    rev_reader: &mut ReverseLineReader<'_>,
    path_str: &str,
    requested_leaf: Option<u64>,
    visited: &mut HashSet<(PathBuf, u64)>,
) -> Result<ReplayedSession, WalError> {
    let explicit_leaf = requested_leaf.is_some();
    let target_leaf = requested_leaf.unwrap_or(eof_id);
    let mut want_id: Option<u64> = Some(target_leaf);
    let mut matched_target_leaf = false;

    let mut max_node_id: u64 = eof_id;
    let mut collected_rev: Vec<ActiveEntry> = Vec::new();
    let mut surviving_assistant_usages: BTreeMap<u64, TokenUsage> = BTreeMap::new();
    let mut committed_message_ids: HashSet<String> = HashSet::new();
    let mut window_usage = TokenUsage::default();
    let mut checkpoint_base_usage = TokenUsage::default();
    let mut session_id: Option<String> = None;
    let mut latest_config: Option<WalConfigSnapshot> = None;
    let mut last_checkpoint_turn: u64 = 0;
    let mut max_turn_id: u64 = 0;
    let mut found_section_header = false;
    let mut branch_base_parent: Option<(u64, ReplayedSession)> = None;

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
            continue;
        }

        let ev_parent = event.parent_id();
        if let Some(pid) = ev_parent
            && (pid == 0 || pid >= ev_id)
        {
            return Err(WalError::MalformedEntry {
                path: path_str.to_owned(),
                reason: format!(
                    "WAL node {ev_id} has non-ancestor parent_id {pid} (must satisfy 1 <= parent_id < id)"
                ),
            });
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
                reason: "cannot branch from a section header (`session_init` or `checkpoint`); select a message, runtime_update, fold, or branch_base node".into(),
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
            WalEvent::BranchBase {
                ir_version,
                base,
                session_id: sid,
                config,
                ..
            } => {
                if ir_version != IR_VERSION {
                    return Err(WalError::IncompatibleVersion(ir_version));
                }
                let (parent_path, parent_leaf_id) = parse_branch_base_ref(&base, path_str)?;
                let parent_session =
                    replay_file_readonly(&parent_path, Some(parent_leaf_id), visited)?;
                session_id = sid
                    .filter(|s| !s.trim().is_empty())
                    .or_else(|| parent_session.session_id.clone());
                if latest_config.is_none() {
                    let has_any = config.output.is_some()
                        || config.shell.is_some()
                        || config.stderr.is_some()
                        || config.tool_name.is_some();
                    latest_config = if has_any {
                        Some(config)
                    } else {
                        parent_session.latest_config.clone()
                    };
                }
                branch_base_parent = Some((ev_id, parent_session));
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
                message_ids,
                ..
            } => {
                max_turn_id = max_turn_id.max(turn_id);
                for mid in message_ids {
                    if !mid.is_empty() {
                        committed_message_ids.insert(mid);
                    }
                }
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
            reason: "broken parent_id chain: missing `session_init`, `branch_base`, or `checkpoint` ancestor".into(),
        });
    }

    collected_rev.reverse();

    let (mut combined_entries, branch_base_id_opt) =
        if let Some((bb_id, parent)) = branch_base_parent {
            for (tid, u) in parent.assistant_usages {
                surviving_assistant_usages.entry(tid).or_insert(u);
            }
            committed_message_ids.extend(parent.committed_message_ids);
            checkpoint_base_usage.add_assign(&parent.session_usage);
            let parent_max_turn = parent
                .entries
                .iter()
                .map(|e| e.message.turn_id())
                .max()
                .unwrap_or(0);
            // Prevent local `aclr -t` folds from slicing inside the immutable parent file.
            last_checkpoint_turn = parent.last_checkpoint_turn.max(parent_max_turn);
            max_turn_id = max_turn_id.max(parent.next_turn_id.saturating_sub(1));

            let mut merged: Vec<ActiveEntry> = parent
                .entries
                .into_iter()
                .map(|mut e| {
                    // Localize inherited entries' anchor ID to `branch_base` so child folds stay valid.
                    e.node_id = Some(bb_id);
                    e
                })
                .collect();
            merged.extend(collected_rev);
            (merged, Some(bb_id))
        } else {
            (collected_rev, None)
        };

    let before_orphan_len = combined_entries.len();
    combined_entries = drop_orphan_tool_entries(combined_entries);
    if explicit_leaf && combined_entries.len() != before_orphan_len {
        return Err(WalError::InvalidLeaf {
            path: path_str.to_owned(),
            leaf_id: target_leaf,
            reason: "branch contains orphaned tool message without matching assistant tool_calls"
                .into(),
        });
    }
    if explicit_leaf && combined_entries.is_empty() {
        return Err(WalError::InvalidLeaf {
            path: path_str.to_owned(),
            leaf_id: target_leaf,
            reason: "selected leaf yields an empty active context".into(),
        });
    }

    let surviving_ids: HashSet<u64> = combined_entries
        .iter()
        .filter_map(|e| match &e.message {
            IrMessage::Assistant { turn_id, .. } => Some(*turn_id),
            _ => None,
        })
        .collect();
    surviving_assistant_usages.retain(|tid, _| surviving_ids.contains(tid));

    let mut session_usage = checkpoint_base_usage;
    session_usage.add_assign(&window_usage);

    let head_id = combined_entries
        .last()
        .and_then(|e| e.node_id)
        .or(branch_base_id_opt);

    Ok(ReplayedSession {
        entries: combined_entries,
        assistant_usages: surviving_assistant_usages,
        session_usage,
        session_id,
        latest_config,
        last_checkpoint_turn,
        next_turn_id: max_turn_id.saturating_add(1).max(1),
        tree_mode: true,
        head_id,
        next_node_id: max_node_id.saturating_add(1).max(1),
        committed_message_ids,
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
    let mut committed_message_ids: HashSet<String> = HashSet::new();
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
            WalEvent::BranchBase { .. } => {
                return Err(WalError::MalformedEntry {
                    path: path_str.to_owned(),
                    reason: "`branch_base` is invalid in a legacy linear WAL without node IDs"
                        .into(),
                });
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
                message_ids,
                ..
            } => {
                max_turn_id = max_turn_id.max(turn_id);
                for mid in message_ids {
                    if !mid.is_empty() {
                        committed_message_ids.insert(mid);
                    }
                }
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
        committed_message_ids,
        is_fresh: false,
    })
}

/// Implement `anix fork <SRC_WAL> [--leaf <ID>] -o <DST_WAL>` (RFC §5.2).
pub fn fork_wal(src: &Path, leaf: Option<u64>, dst: &Path) -> ExitCode {
    if let Some(leaf_id) = leaf
        && leaf_id == 0
    {
        eprintln!("anix fork: --leaf must be >= 1");
        return ExitCode::UsageError;
    }

    let abs_src = match fs::canonicalize(src) {
        Ok(p) => p,
        Err(err) => {
            eprintln!(
                "anix fork: cannot resolve source `{}`: {err}",
                src.display()
            );
            return ExitCode::RuntimeError;
        }
    };

    let mut visited = HashSet::new();
    let replayed = match replay_file_readonly(&abs_src, leaf, &mut visited) {
        Ok(r) => r,
        Err(err) => {
            eprintln!("anix fork: {err}");
            return err.exit_code();
        }
    };

    if replayed.is_fresh || !replayed.tree_mode {
        eprintln!(
            "anix fork: source `{}` must be a non-empty Tree WAL",
            abs_src.display()
        );
        return ExitCode::UsageError;
    }

    let Some(resolved_leaf) = replayed.head_id else {
        eprintln!(
            "anix fork: source `{}` has no valid active leaf node",
            abs_src.display()
        );
        return ExitCode::UsageError;
    };

    if let Some(parent) = dst.parent()
        && !parent.as_os_str().is_empty()
        && !parent.exists()
        && let Err(err) = fs::create_dir_all(parent)
    {
        eprintln!(
            "anix fork: failed creating parent dir `{}`: {err}",
            parent.display()
        );
        return ExitCode::RuntimeError;
    }

    let mut out_file = match OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(dst)
    {
        Ok(f) => f,
        Err(err) => {
            eprintln!(
                "anix fork: cannot create destination `{}`: {err}",
                dst.display()
            );
            return ExitCode::RuntimeError;
        }
    };

    let base_ref = format!("{}#{}", abs_src.display(), resolved_leaf);
    let event = WalEvent::BranchBase {
        id: Some(1),
        ir_version: IR_VERSION,
        base: base_ref.clone(),
        ts: posix::timestamp_rfc3339_utc(),
        session_id: Some(uuid::Uuid::new_v4().to_string()),
        config: replayed.latest_config.unwrap_or_default(),
    };

    let mut line = match serde_json::to_string(&event) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("anix fork: failed serializing branch_base: {err}");
            let _ = fs::remove_file(dst);
            return ExitCode::RuntimeError;
        }
    };
    line.push('\n');

    if let Err(err) = out_file
        .write_all(line.as_bytes())
        .and_then(|_| out_file.flush())
        .and_then(|_| out_file.sync_data())
    {
        eprintln!("anix fork: failed writing `{}`: {err}", dst.display());
        let _ = fs::remove_file(dst);
        return ExitCode::RuntimeError;
    }

    println!("forked {} from {}", dst.display(), base_ref);
    ExitCode::Success
}

/// Implement `anix fsck <WAL>` (RFC §5.2).
pub fn fsck_wal(path: &Path) -> ExitCode {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(err) => {
            eprintln!("anix fsck: cannot open `{}`: {err}", path.display());
            return ExitCode::RuntimeError;
        }
    };

    let mut reader = BufReader::new(file);
    let mut line_num: usize = 0;
    let mut event_count: usize = 0;
    let mut seen_ids: HashSet<u64> = HashSet::new();
    let mut last_id: u64 = 0;
    let mut tree_mode: Option<bool> = None;
    let mut eof_event: Option<WalEvent> = None;
    let mut errors: Vec<String> = Vec::new();
    let mut buf = Vec::new();

    loop {
        buf.clear();
        let n = match reader.read_until(b'\n', &mut buf) {
            Ok(n) => n,
            Err(err) => {
                eprintln!("anix fsck: I/O error reading `{}`: {err}", path.display());
                return ExitCode::RuntimeError;
            }
        };
        if n == 0 {
            break;
        }
        line_num += 1;
        while matches!(buf.last(), Some(b'\n' | b'\r')) {
            buf.pop();
        }
        if buf.iter().all(|b| b.is_ascii_whitespace()) {
            continue;
        }

        let line_str = match std::str::from_utf8(&buf) {
            Ok(s) => s,
            Err(err) => {
                errors.push(format!("line {line_num}: invalid UTF-8 ({err})"));
                continue;
            }
        };

        let event: WalEvent = match serde_json::from_str(line_str) {
            Ok(ev) => ev,
            Err(err) => {
                errors.push(format!("line {line_num}: invalid JSON ({err})"));
                continue;
            }
        };
        event_count += 1;

        match &event {
            WalEvent::SessionInit { ir_version, .. }
            | WalEvent::BranchBase { ir_version, .. }
            | WalEvent::Checkpoint { ir_version, .. }
                if *ir_version != IR_VERSION =>
            {
                errors.push(format!(
                    "line {line_num}: incompatible ir_version {ir_version} (expected {IR_VERSION})"
                ));
            }
            _ => {}
        }

        let is_tree = event.id().is_some();
        match tree_mode {
            None => tree_mode = Some(is_tree),
            Some(prev) if prev != is_tree => {
                errors.push(format!(
                    "line {line_num}: mixed tree/legacy WAL events (missing or unexpected `id`)"
                ));
            }
            _ => {}
        }

        if let Some(id) = event.id() {
            if id == 0 || id <= last_id {
                errors.push(format!(
                    "line {line_num}: non-monotonic node id {id} (previous {last_id})"
                ));
            }
            match &event {
                WalEvent::SessionInit { .. } | WalEvent::BranchBase { .. } => {
                    if let Some(pid) = event.parent_id() {
                        errors.push(format!(
                            "line {line_num}: root node {id} must not have parent_id {pid}"
                        ));
                    }
                }
                _ => match event.parent_id() {
                    Some(pid) if pid >= 1 && pid < id && seen_ids.contains(&pid) => {}
                    Some(pid) => {
                        errors.push(format!(
                            "line {line_num}: node {id} references invalid or missing parent_id {pid}"
                        ));
                    }
                    None => {
                        errors.push(format!(
                            "line {line_num}: non-root node {id} is missing parent_id"
                        ));
                    }
                },
            }
            seen_ids.insert(id);
            last_id = last_id.max(id);
        }

        if let WalEvent::BranchBase { ref base, .. } = event {
            let path_str = path.display().to_string();
            match parse_branch_base_ref(base, &path_str) {
                Ok((parent_path, parent_leaf)) => {
                    let mut visited = HashSet::new();
                    if let Err(err) =
                        replay_file_readonly(&parent_path, Some(parent_leaf), &mut visited)
                    {
                        errors.push(format!(
                            "line {line_num}: unreachable or invalid branch_base `{base}`: {err}"
                        ));
                    }
                }
                Err(err) => errors.push(format!("line {line_num}: {err}")),
            }
        }

        eof_event = Some(event);
    }

    if event_count == 0 {
        println!("[OK] {}: empty WAL (0 events)", path.display());
        return ExitCode::Success;
    }

    // Verify full replay and strict tool-call closure at the active EOF tip.
    if errors.is_empty() {
        let mut visited = HashSet::new();
        let strict_leaf = eof_event.as_ref().and_then(|ev| match ev {
            WalEvent::SessionInit { .. } | WalEvent::Checkpoint { .. } => None,
            other => other.id(),
        });
        if let Err(err) = replay_file_readonly(path, strict_leaf, &mut visited) {
            errors.push(format!("active lineage check failed: {err}"));
        }
    }

    if !errors.is_empty() {
        eprintln!("[FAIL] {} ({} issue(s)):", path.display(), errors.len());
        for err in &errors {
            eprintln!("  - {err}");
        }
        return ExitCode::RuntimeError;
    }

    let mode_label = if tree_mode == Some(true) {
        "tree"
    } else {
        "legacy"
    };
    if let Some(id) = eof_event.as_ref().and_then(WalEvent::id) {
        println!(
            "[OK] {}: {event_count} events valid ({mode_label}, head={id})",
            path.display()
        );
    } else {
        println!(
            "[OK] {}: {event_count} events valid ({mode_label})",
            path.display()
        );
    }
    ExitCode::Success
}

/// Implement `anix log <WAL> [--leaf <ID>]` (RFC §5.2).
pub fn log_wal(path: &Path, leaf: Option<u64>) -> ExitCode {
    if let Some(leaf_id) = leaf {
        if leaf_id == 0 {
            eprintln!("anix log: --leaf must be >= 1");
            return ExitCode::UsageError;
        }
        let mut visited = HashSet::new();
        let replayed = match replay_file_readonly(path, Some(leaf_id), &mut visited) {
            Ok(r) => r,
            Err(err) => {
                eprintln!("anix log: {err}");
                return err.exit_code();
            }
        };
        println!(
            "Lineage for {} (leaf={}, entries={}):",
            path.display(),
            leaf_id,
            replayed.entries.len()
        );
        for entry in &replayed.entries {
            let id_str = entry
                .node_id
                .map(|id| id.to_string())
                .unwrap_or_else(|| "-".into());
            let (role, tid, preview) = summarize_ir_message(&entry.message);
            println!("  node={id_str:<4} turn={tid:<4} role={role:<10} | {preview}");
        }
        return ExitCode::Success;
    }

    let file = match File::open(path) {
        Ok(f) => f,
        Err(err) => {
            eprintln!("anix log: cannot open `{}`: {err}", path.display());
            return ExitCode::RuntimeError;
        }
    };
    let reader = BufReader::new(file);
    let mut events = Vec::new();
    for (idx, line_res) in reader.lines().enumerate() {
        let line = match line_res {
            Ok(l) => l,
            Err(err) => {
                eprintln!("anix log: read error on line {}: {err}", idx + 1);
                return ExitCode::RuntimeError;
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<WalEvent>(&line) {
            Ok(ev) => events.push(ev),
            Err(err) => {
                eprintln!("anix log: invalid JSON on line {}: {err}", idx + 1);
                return ExitCode::RuntimeError;
            }
        }
    }

    let head_id = events.last().and_then(WalEvent::id);
    for ev in &events {
        let id_str = ev
            .id()
            .map(|id| id.to_string())
            .unwrap_or_else(|| "-".into());
        let parent_str = ev
            .parent_id()
            .map(|pid| pid.to_string())
            .unwrap_or_else(|| "-".into());
        let marker = if head_id.is_some() && ev.id() == head_id {
            "*"
        } else {
            " "
        };
        let (kind, turn_str, ts, summary) = summarize_wal_event(ev);
        println!(
            "{marker} id={id_str:<4} parent={parent_str:<4} turn={turn_str:<4} {kind:<15} {ts} | {summary}"
        );
    }

    ExitCode::Success
}

fn summarize_ir_message(msg: &IrMessage) -> (&'static str, u64, String) {
    match msg {
        IrMessage::System { turn_id, text, .. } => ("system", *turn_id, one_line_preview(text, 80)),
        IrMessage::User { turn_id, text, .. } => ("user", *turn_id, one_line_preview(text, 80)),
        IrMessage::Assistant {
            turn_id,
            text,
            tool_calls,
            ..
        } => {
            let preview = if !tool_calls.is_empty() {
                let names: Vec<&str> = tool_calls.iter().map(|tc| tc.name.as_str()).collect();
                format!(
                    "[tool_calls: {}] {}",
                    names.join(","),
                    one_line_preview(text.as_deref().unwrap_or(""), 50)
                )
            } else {
                one_line_preview(text.as_deref().unwrap_or(""), 80)
            };
            ("assistant", *turn_id, preview)
        }
        IrMessage::Tool {
            turn_id,
            tool_call_id,
            text,
            ..
        } => (
            "tool",
            *turn_id,
            format!("[{tool_call_id}] {}", one_line_preview(text, 60)),
        ),
    }
}

fn summarize_wal_event(ev: &WalEvent) -> (String, String, String, String) {
    match ev {
        WalEvent::SessionInit { ts, session_id, .. } => (
            "session_init".into(),
            "-".into(),
            ts.clone(),
            format!("session_id={}", session_id.as_deref().unwrap_or("-")),
        ),
        WalEvent::BranchBase { ts, base, .. } => (
            "branch_base".into(),
            "-".into(),
            ts.clone(),
            format!("base={base}"),
        ),
        WalEvent::Checkpoint { ts, turn_id, .. } => (
            "checkpoint".into(),
            turn_id.to_string(),
            ts.clone(),
            "compaction checkpoint".into(),
        ),
        WalEvent::RuntimeUpdate {
            ts,
            turn_id,
            content,
            ..
        } => (
            "runtime_update".into(),
            turn_id.to_string(),
            ts.clone(),
            one_line_preview(content, 80),
        ),
        WalEvent::Fold {
            ts,
            from_turn,
            to_turn,
            summary,
            ..
        } => (
            "fold".into(),
            format!("{from_turn}..{to_turn}"),
            ts.clone(),
            one_line_preview(summary, 80),
        ),
        WalEvent::Message {
            ts,
            turn_id,
            role,
            text,
            tool_calls,
            ..
        } => {
            let role_s = match role {
                WalRole::System => "msg:system",
                WalRole::User => "msg:user",
                WalRole::Assistant => "msg:assistant",
                WalRole::Tool => "msg:tool",
            };
            let preview = if !tool_calls.is_empty() {
                format!(
                    "[{} tool_call(s)] {}",
                    tool_calls.len(),
                    one_line_preview(text.as_deref().unwrap_or(""), 60)
                )
            } else {
                one_line_preview(text.as_deref().unwrap_or(""), 80)
            };
            (role_s.into(), turn_id.to_string(), ts.clone(), preview)
        }
    }
}

fn one_line_preview(raw: &str, max_chars: usize) -> String {
    let single = raw.replace(['\r', '\n'], " ");
    let trimmed = single.trim();
    let mut out = String::new();
    for (idx, ch) in trimmed.chars().enumerate() {
        if idx >= max_chars {
            out.push_str("...");
            break;
        }
        out.push(ch);
    }
    out
}

/// Convert an [`IrMessage`] into its corresponding [`WalEvent::Message`].
pub fn ir_message_to_wal_event(
    id: Option<u64>,
    parent_id: Option<u64>,
    msg: &IrMessage,
    ts: String,
    usage: Option<TokenUsage>,
    message_ids: Vec<String>,
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
            message_ids: Vec::new(),
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
            message_ids,
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
            message_ids: Vec::new(),
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
            message_ids: Vec::new(),
        },
    }
}

/// Align `requested_from_turn` upward to satisfy Tool-Pair Closure (RFC §5.1).
pub fn align_fold_from_turn(messages: &[IrMessage], requested_from_turn: u64) -> u64 {
    let mut aligned = requested_from_turn;

    if let Some(idx) = messages.iter().position(|m| m.turn_id() >= aligned)
        && matches!(messages[idx], IrMessage::Tool { .. })
    {
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
                self.buffer.pop();
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
                                name: "shell".into(),
                                arguments: serde_json::json!({"cmd": "true"}),
                            },
                            ToolCall {
                                id: "c2".into(),
                                name: "shell".into(),
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
        let leaf_err = store
            .load_active_window(Some(1))
            .expect_err("legacy WAL must reject --leaf");
        assert_eq!(leaf_err.exit_code(), ExitCode::UsageError);

        let replayed = store.load_active_window(None).expect("replay legacy wal");
        let _ = fs::remove_file(&tmp);

        assert!(!replayed.tree_mode);
        assert_eq!(replayed.head_id, None);
        assert_eq!(replayed.entries.len(), 2);
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
    fn fork_chain_fsck_and_tree_replay_work_across_files_without_parent_lock() {
        let dir = std::env::temp_dir().join(format!("anix-fork-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let parent_wal = dir.join("parent.jsonl");
        let child_wal = dir.join("child.jsonl");

        // Keep an exclusive lock on parent_wal to verify fork & read-only parent replay never block!
        let mut parent_store = WalStore::open(&parent_wal).unwrap();
        parent_store
            .append(&WalEvent::SessionInit {
                id: Some(1),
                ir_version: IR_VERSION,
                ts: "2026-11-27T00:00:00Z".into(),
                session_id: Some("sid-parent".into()),
                config: WalConfigSnapshot::default(),
            })
            .unwrap();
        parent_store
            .append_message(Some(2), Some(1), &IrMessage::system(0, "sys"))
            .unwrap();
        parent_store
            .append_message_with_meta(
                Some(3),
                Some(2),
                &IrMessage::user(1, "root prompt"),
                None,
                vec!["msg-root".into()],
            )
            .unwrap();
        parent_store
            .append_message_with_usage(
                Some(4),
                Some(3),
                &IrMessage::Assistant {
                    turn_id: 2,
                    text: Some("parent answer".into()),
                    reasoning: None,
                    tool_calls: Vec::new(),
                    provider_opaque: None,
                    extra: BTreeMap::new(),
                },
                Some(TokenUsage {
                    input_tokens: 100,
                    output_tokens: 20,
                    cached_tokens: Some(0),
                    cache_write_tokens: Some(0),
                    reasoning_tokens: Some(0),
                }),
            )
            .unwrap();

        assert_eq!(
            fork_wal(&parent_wal, Some(4), &child_wal),
            ExitCode::Success
        );
        assert_eq!(fsck_wal(&parent_wal), ExitCode::Success);
        assert_eq!(fsck_wal(&child_wal), ExitCode::Success);

        let mut child_store = WalStore::open(&child_wal).unwrap();
        let replayed = child_store.load_active_window(None).unwrap();
        assert_eq!(replayed.entries.len(), 3);
        assert_eq!(replayed.head_id, Some(1));
        assert_eq!(replayed.next_node_id, 2);
        assert_eq!(replayed.next_turn_id, 3);
        assert_eq!(replayed.session_usage.input_tokens, 100);
        assert!(replayed.committed_message_ids.contains("msg-root"));

        // Append child message anchored on branch_base id=1
        child_store
            .append_message(Some(2), Some(1), &IrMessage::user(3, "branch prompt"))
            .unwrap();
        let replayed2 = child_store.load_active_window(None).unwrap();
        assert_eq!(replayed2.entries.len(), 4);
        assert_eq!(replayed2.head_id, Some(2));
        assert_eq!(fsck_wal(&child_wal), ExitCode::Success);

        drop(child_store);
        drop(parent_store);
        let _ = fs::remove_dir_all(&dir);
    }
}
