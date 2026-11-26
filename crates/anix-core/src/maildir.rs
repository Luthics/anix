//! Unified Maildir IPC, `anix send [--wait]`, `--notify`, and `--inbox` event loop
//! with debounce batching, full rescan fallback, 60s safety scan, and crash recovery (RFC §4.3).

use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::RawFd;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anix_proto::exit_code::ExitCode;
use anix_proto::maildir::{MaildirEnvelope, MaildirHeaders};
use uuid::Uuid;

use crate::fd;
use crate::platform::{DirWatcher, posix};
use crate::signal::SignalState;

/// Periodic safety rescan interval guarding against container overlayfs / `IN_Q_OVERFLOW` drops.
const SAFETY_RESCAN_INTERVAL: Duration = Duration::from_secs(60);
/// Safety rescan interval for short-lived synchronous `anix send --wait` reply queues.
const REPLY_RESCAN_INTERVAL: Duration = Duration::from_secs(2);

/// Ensure `<dir>` and `{tmp,new,cur,done}` subdirectories exist with mode `0700`.
pub fn ensure_maildir(dir: &Path) -> io::Result<()> {
    posix::ensure_dir_mode(dir, 0o700)?;
    for sub in ["tmp", "new", "cur", "done"] {
        posix::ensure_dir_mode(&dir.join(sub), 0o700)?;
    }
    Ok(())
}

/// Atomically deliver a [`MaildirEnvelope`] into `<dir>/new/` (`tmp` -> `fsync` -> `rename`).
/// Returns the generated filename.
pub fn deliver_maildir(dir: &Path, envelope: &MaildirEnvelope) -> io::Result<String> {
    ensure_maildir(dir)?;

    let ts_ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pid = std::process::id();
    let rand = &Uuid::new_v4().simple().to_string()[..12];
    let filename = format!("{ts_ns:020}-{pid}-{rand}.msg");
    let tmp_path = dir.join("tmp").join(&filename);
    let new_path = dir.join("new").join(&filename);

    let encoded = envelope.format();
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&tmp_path)?;
    f.write_all(encoded.as_bytes())?;
    f.sync_data()?;
    drop(f);

    fs::rename(&tmp_path, &new_path).inspect_err(|_| {
        let _ = fs::remove_file(&tmp_path);
    })?;
    Ok(filename)
}

/// Deliver a reply message to an `Anix-Reply-To` Maildir (only if the temp dir still exists).
pub fn deliver_reply(
    reply_to_dir: &Path,
    in_reply_to: &str,
    from: &str,
    exit_code: i32,
    body: &str,
) -> io::Result<bool> {
    if !reply_to_dir.is_dir() {
        return Ok(false);
    }
    let env = MaildirEnvelope {
        headers: MaildirHeaders {
            message_id: Some(format!("reply_{}", Uuid::new_v4().simple())),
            reply_to: None,
            in_reply_to: Some(in_reply_to.to_owned()),
            from: Some(from.to_owned()),
            control: None,
            exit_code: Some(exit_code),
        },
        body: body.to_owned(),
    };
    deliver_maildir(reply_to_dir, &env)?;
    Ok(true)
}

/// Deliver an exit notification message for `--notify <MAILDIR>` (RFC §4.3.4).
pub fn deliver_notify(
    notify_dir: &Path,
    notify_id: Option<&str>,
    agent_name: &str,
    exit_code: i32,
    body: &str,
) -> io::Result<()> {
    let env = MaildirEnvelope {
        headers: MaildirHeaders {
            message_id: Some(format!("notify_{}", Uuid::new_v4().simple())),
            reply_to: None,
            in_reply_to: notify_id.map(str::to_owned),
            from: Some(agent_name.to_owned()),
            control: None,
            exit_code: Some(exit_code),
        },
        body: body.to_owned(),
    };
    deliver_maildir(notify_dir, &env)?;
    Ok(())
}

/// Implement `anix send <MAILDIR> [--wait [--timeout <SEC>]] ...` (RFC §4.3.1 & §4.3.2).
#[allow(clippy::too_many_arguments)]
pub fn run_send(
    maildir: &Path,
    wait: bool,
    timeout_sec: Option<u64>,
    in_reply_to: Option<&str>,
    control: Option<&str>,
    from: Option<&str>,
    message_args: &[String],
) -> ExitCode {
    if timeout_sec.is_some() && !wait {
        eprintln!("anix send: --timeout requires --wait");
        return ExitCode::UsageError;
    }
    if let Some(op) = control
        && op != "compact"
    {
        eprintln!("anix send: unsupported --control `{op}` (supported: `compact`)");
        return ExitCode::UsageError;
    }

    let signals = match SignalState::install() {
        Ok(s) => s,
        Err(err) => {
            eprintln!("anix send: failed to install signal handlers: {err}");
            return ExitCode::RuntimeError;
        }
    };

    let body = if !message_args.is_empty() {
        message_args.join(" ")
    } else if !posix::isatty(0) {
        match fd::read_fd_interruptible(0, &signals) {
            Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(err) => {
                if let Some(code) = signals.termination_exit_code() {
                    return code;
                }
                eprintln!("anix send: failed reading stdin: {err}");
                return ExitCode::RuntimeError;
            }
        }
    } else {
        String::new()
    };

    if body.is_empty() && control.is_none() && in_reply_to.is_none() {
        eprintln!("anix send: message body is empty (pass MSG..., pipe stdin, or set --control)");
        return ExitCode::UsageError;
    }

    let msg_id = format!("msg_{}", Uuid::new_v4().simple());

    let reply_dir_guard = if wait {
        match create_temp_reply_maildir() {
            Ok(dir) => Some(TempDirGuard(dir)),
            Err(err) => {
                eprintln!("anix send: failed creating temporary reply Maildir: {err}");
                return ExitCode::RuntimeError;
            }
        }
    } else {
        None
    };

    let reply_watcher = reply_dir_guard
        .as_ref()
        .and_then(|g| DirWatcher::watch(&g.0.join("new"), Some(signals.wakeup_fd())).ok());

    let envelope = MaildirEnvelope {
        headers: MaildirHeaders {
            message_id: Some(msg_id),
            reply_to: reply_dir_guard.as_ref().map(|g| g.0.display().to_string()),
            in_reply_to: in_reply_to.map(str::to_owned),
            from: from.map(str::to_owned),
            control: control.map(str::to_owned),
            exit_code: None,
        },
        body,
    };

    if let Err(err) = deliver_maildir(maildir, &envelope) {
        eprintln!(
            "anix send: failed delivering message to `{}`: {err}",
            maildir.display()
        );
        return ExitCode::RuntimeError;
    }

    let Some(ref reply_guard) = reply_dir_guard else {
        return ExitCode::Success;
    };

    let reply_new_dir = reply_guard.0.join("new");
    let deadline = timeout_sec.map(|s| Instant::now() + Duration::from_secs(s));

    loop {
        if let Some(code) = signals.termination_exit_code() {
            return code;
        }

        if let Ok(Some(reply_env)) = try_read_first_reply(&reply_new_dir) {
            let mut out = io::stdout().lock();
            let _ = out.write_all(reply_env.body.as_bytes());
            let _ = out.flush();
            return map_i32_to_exit_code(reply_env.headers.exit_code.unwrap_or(0));
        }

        if let Some(dl) = deadline
            && Instant::now() >= dl
        {
            eprintln!(
                "anix send: timed out after {}s waiting for reply",
                timeout_sec.unwrap_or(0)
            );
            return ExitCode::TimeoutOrBudget;
        }

        let wait_dur = match deadline {
            Some(dl) => dl
                .saturating_duration_since(Instant::now())
                .min(REPLY_RESCAN_INTERVAL),
            None => REPLY_RESCAN_INTERVAL,
        };
        if wait_dur.is_zero() {
            continue;
        }

        if let Some(ref w) = reply_watcher {
            let _ = w.wait(Some(wait_dur));
            signals.drain_wakeup();
        } else {
            let _ = posix::sleep_or_signal(wait_dur, signals.wakeup_fd());
            signals.drain_wakeup();
        }
    }
}

fn create_temp_reply_maildir() -> io::Result<PathBuf> {
    let base = match env::var_os("XDG_RUNTIME_DIR").filter(|v| !v.is_empty()) {
        Some(dir) => PathBuf::from(dir).join("anix"),
        None => PathBuf::from(format!("/tmp/anix-{}", posix::current_uid())),
    };
    posix::ensure_dir_mode(&base, 0o700)?;
    let reply_dir = base.join(format!("reply-{}", Uuid::new_v4().simple()));
    ensure_maildir(&reply_dir)?;
    Ok(reply_dir)
}

fn try_read_first_reply(new_dir: &Path) -> io::Result<Option<MaildirEnvelope>> {
    let mut names = list_sorted_entries(new_dir)?;
    if names.is_empty() {
        return Ok(None);
    }
    let name = names.remove(0);
    let path = new_dir.join(&name);
    let bytes = fs::read(&path)?;
    let raw = String::from_utf8_lossy(&bytes);
    Ok(Some(MaildirEnvelope::parse(&raw, &name)))
}

fn map_i32_to_exit_code(code: i32) -> ExitCode {
    match code {
        0 => ExitCode::Success,
        2 => ExitCode::UsageError,
        75 => ExitCode::TempFail,
        124 => ExitCode::TimeoutOrBudget,
        130 => ExitCode::SigInt,
        143 => ExitCode::SigTerm,
        _ => ExitCode::RuntimeError,
    }
}

struct TempDirGuard(PathBuf);

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// An atomic message claimed into `<inbox>/cur/<filename>` (RFC §4.3.3).
#[derive(Debug, Clone)]
pub struct InboxMessage {
    pub filename: String,
    pub cur_path: PathBuf,
    pub envelope: MaildirEnvelope,
}

impl InboxMessage {
    pub fn message_id(&self) -> &str {
        self.envelope.message_id()
    }

    pub fn format_for_user_turn(&self) -> String {
        let mut header_attrs = format!("source=\"{}\"", self.filename);
        if let Some(ref from) = self.envelope.headers.from {
            header_attrs.push_str(&format!(" from=\"{from}\""));
        }
        if let Some(ref in_reply) = self.envelope.headers.in_reply_to {
            header_attrs.push_str(&format!(" in_reply_to=\"{in_reply}\""));
        }
        if let Some(code) = self.envelope.headers.exit_code {
            header_attrs.push_str(&format!(" exit_code=\"{code}\""));
        }
        format!("<input {header_attrs}>\n{}\n</input>", self.envelope.body)
    }
}

/// Outcome of waiting for the next event on [`InboxQueue`].
#[derive(Debug)]
pub enum InboxWaitOutcome {
    Batch(Vec<InboxMessage>),
    HoldExpired,
    IdleTimeout,
    Interrupted(ExitCode),
}

/// Maildir-style atomic queue (`tmp/`, `new/`, `cur/`, `done/`, mode `0700`)
/// watched via `inotify`/`kqueue` with full rescan on every wakeup, 60s safety rescan,
/// debounce coalescing, batch drain, and `hold` lease deadline awareness (RFC §4.3.3).
#[derive(Debug)]
pub struct InboxQueue {
    new_dir: PathBuf,
    cur_dir: PathBuf,
    done_dir: PathBuf,
    watcher: Option<DirWatcher>,
}

impl InboxQueue {
    pub fn open(dir: &Path, sig_fd: Option<RawFd>) -> io::Result<Self> {
        ensure_maildir(dir)?;
        let new_dir = dir.join("new");
        let cur_dir = dir.join("cur");
        let done_dir = dir.join("done");
        let watcher = DirWatcher::watch(&new_dir, sig_fd).ok();
        Ok(Self {
            new_dir,
            cur_dir,
            done_dir,
            watcher,
        })
    }

    /// Scan `cur/` on startup for crash recovery & WAL deduplication (RFC §4.3.3).
    pub fn scan_cur_for_recovery(&self) -> io::Result<Vec<InboxMessage>> {
        let names = list_sorted_entries(&self.cur_dir)?;
        let mut msgs = Vec::with_capacity(names.len());
        for name in names {
            let cur_path = self.cur_dir.join(&name);
            let bytes = match fs::read(&cur_path) {
                Ok(b) => b,
                Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
                Err(err) => return Err(err),
            };
            let raw = String::from_utf8_lossy(&bytes);
            let envelope = MaildirEnvelope::parse(&raw, &name);
            msgs.push(InboxMessage {
                filename: name,
                cur_path,
                envelope,
            });
        }
        Ok(msgs)
    }

    /// Wait for the next batch of Maildir messages, a `hold` expiry, idle timeout, or signal.
    ///
    /// - When `next_hold_deadline` is `Some(dl)`: `idle_timeout` does NOT count down; returns
    ///   `HoldExpired` as soon as `Instant::now() >= dl` while `new/` is empty.
    /// - When `next_hold_deadline` is `None`: starts `idle_timeout` once `new/` is empty and
    ///   emits `on_idle()` once.
    /// - Whenever `new/` becomes non-empty: waits `debounce` (if > 0) to coalesce bursts, then
    ///   drains ALL files from `new/` into `cur/` in ASCII order as a single batch.
    #[allow(clippy::too_many_arguments)]
    pub fn next_batch(
        &mut self,
        idle_timeout: Option<Duration>,
        next_hold_deadline: Option<Instant>,
        debounce: Duration,
        signals: &SignalState,
        on_snapshot: &mut impl FnMut(),
        on_idle: &mut impl FnMut(),
    ) -> io::Result<InboxWaitOutcome> {
        let mut idle_start: Option<Instant> = None;
        let mut emitted_idle = false;

        loop {
            if let Some(code) = signals.termination_exit_code() {
                return Ok(InboxWaitOutcome::Interrupted(code));
            }
            if signals.take_snapshot_request() {
                on_snapshot();
            }

            if self.has_new_files()? {
                // Coalesce burst arrivals over `debounce` window (interruptible by signals).
                if !debounce.is_zero() {
                    let debounce_end = Instant::now() + debounce;
                    while Instant::now() < debounce_end {
                        if let Some(code) = signals.termination_exit_code() {
                            return Ok(InboxWaitOutcome::Interrupted(code));
                        }
                        let rem = debounce_end.saturating_duration_since(Instant::now());
                        if rem.is_zero() {
                            break;
                        }
                        if let Some(ref watcher) = self.watcher {
                            let _ = watcher.wait(Some(rem));
                            signals.drain_wakeup();
                        } else {
                            let _ = posix::sleep_or_signal(rem, signals.wakeup_fd());
                            signals.drain_wakeup();
                        }
                    }
                }
                let batch = self.claim_all_new()?;
                if !batch.is_empty() {
                    return Ok(InboxWaitOutcome::Batch(batch));
                }
            }

            // Active holds prevent idle-timeout countdown; wake immediately on earliest hold expiry.
            let wait_dur = if let Some(hold_dl) = next_hold_deadline {
                let rem = hold_dl.saturating_duration_since(Instant::now());
                if rem.is_zero() {
                    return Ok(InboxWaitOutcome::HoldExpired);
                }
                if !emitted_idle {
                    on_idle();
                    emitted_idle = true;
                }
                rem.min(SAFETY_RESCAN_INTERVAL)
            } else {
                let start = *idle_start.get_or_insert_with(Instant::now);
                if let Some(limit) = idle_timeout
                    && start.elapsed() >= limit
                {
                    return Ok(InboxWaitOutcome::IdleTimeout);
                }
                if !emitted_idle {
                    on_idle();
                    emitted_idle = true;
                }
                match idle_timeout {
                    Some(limit) => limit
                        .saturating_sub(start.elapsed())
                        .min(SAFETY_RESCAN_INTERVAL),
                    None => SAFETY_RESCAN_INTERVAL,
                }
            };

            if wait_dur.is_zero() {
                continue;
            }

            if let Some(ref watcher) = self.watcher {
                let _ = watcher.wait(Some(wait_dur));
                signals.drain_wakeup();
            } else {
                let _ = posix::sleep_or_signal(wait_dur, signals.wakeup_fd());
                signals.drain_wakeup();
            }
        }
    }

    /// Move a completed message atomically from `cur/<filename>` to `done/<filename>`.
    pub fn mark_done(&self, msg: &InboxMessage) -> io::Result<()> {
        let dest = self.done_dir.join(&msg.filename);
        fs::rename(&msg.cur_path, dest)
    }

    fn has_new_files(&self) -> io::Result<bool> {
        for entry in fs::read_dir(&self.new_dir)? {
            let Ok(entry) = entry else { continue };
            let Ok(ft) = entry.file_type() else { continue };
            if !ft.is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.starts_with('.') {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn claim_all_new(&self) -> io::Result<Vec<InboxMessage>> {
        let names = list_sorted_entries(&self.new_dir)?;
        let mut claimed = Vec::with_capacity(names.len());
        for name in names {
            let src = self.new_dir.join(&name);
            let cur_path = self.cur_dir.join(&name);
            match fs::rename(&src, &cur_path) {
                Ok(()) => {
                    let bytes = fs::read(&cur_path)?;
                    let raw = String::from_utf8_lossy(&bytes);
                    let envelope = MaildirEnvelope::parse(&raw, &name);
                    claimed.push(InboxMessage {
                        filename: name,
                        cur_path,
                        envelope,
                    });
                }
                Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
                Err(err) => return Err(err),
            }
        }
        Ok(claimed)
    }
}

fn list_sorted_entries(dir: &Path) -> io::Result<Vec<String>> {
    let mut names = Vec::new();
    for entry in fs::read_dir(dir)? {
        let Ok(entry) = entry else { continue };
        let Ok(ft) = entry.file_type() else { continue };
        if !ft.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        names.push(name);
    }
    names.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
    Ok(names)
}
