//! Structured JSONL Trace sink (`--trace-fd=<N>` / `ANIX_TRACE_FD=<N>` /
//! `$XDG_RUNTIME_DIR/anix/journal.sock`, v1.1 §§4.1 & 9.5).
//!
//! - Explicit activation: NEVER probes `fd 3` implicitly; only writes to an explicitly configured
//!   `--trace-fd` / `ANIX_TRACE_FD`, or connects to `$XDG_RUNTIME_DIR/anix/journal.sock` if present.
//! - EPIPE / broken pipe silently closes the trace sink without blocking or changing exit codes.
//! - `wire` level is strictly forbidden on shared `journal.sock`.

use std::env;
use std::fs::File;
use std::io::Write;
use std::os::unix::io::{FromRawFd, RawFd};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use anix_proto::TRACE_VERSION;
use anix_proto::trace::{TraceEnvelope, TraceLevel};
use serde_json::{Map, Value};

use crate::platform::posix;

#[derive(Debug)]
enum TraceSink {
    Disabled,
    Fd(File),
    JournalSock(UnixStream),
}

#[derive(Debug)]
pub struct Tracer {
    agent_name: String,
    pid: u32,
    ppid: u32,
    level: TraceLevel,
    sink: TraceSink,
}

impl Tracer {
    /// Claim an explicitly configured writable trace FD (`--trace-fd=<N>` / `ANIX_TRACE_FD=<N>`)
    /// immediately BEFORE opening any internal file descriptors (v1.1 §4.1).
    ///
    /// Returns `Err` if `fd` is invalid (`< 3`) or not open and writable.
    pub fn take_configured_fd(fd: RawFd) -> Result<File, String> {
        if fd < 3 {
            return Err(format!("invalid trace fd `{fd}` (must be >= 3)"));
        }
        if !posix::fd_is_writable(fd) {
            return Err(format!("trace fd `{fd}` is not open for writing"));
        }
        let _ = posix::set_cloexec(fd);
        // SAFETY: `fd` was explicitly specified by the caller, verified open & writable, and claimed once.
        Ok(unsafe { File::from_raw_fd(fd) })
    }

    /// Initialize Tracer from an explicitly claimed trace FD or fallback to `$XDG_RUNTIME_DIR/anix/journal.sock`.
    pub fn init(
        agent_name: impl Into<String>,
        trace_file: Option<File>,
        journal_sock: &Path,
    ) -> Self {
        let agent_name = agent_name.into();
        let pid = std::process::id();
        let ppid = posix::current_ppid();
        let env_level = parse_trace_level_env();

        if let Some(file) = trace_file {
            let level = env_level.unwrap_or(TraceLevel::Deltas);
            return Self {
                agent_name,
                pid,
                ppid,
                level,
                sink: TraceSink::Fd(file),
            };
        }

        if journal_sock.exists()
            && let Ok(stream) = UnixStream::connect(journal_sock)
        {
            let _ = stream.set_write_timeout(Some(Duration::from_millis(50)));
            let mut level = env_level.unwrap_or(TraceLevel::Events);
            // Section 9.5: `wire` MUST NEVER be sent to shared `journal.sock`.
            if level == TraceLevel::Wire {
                eprintln!(
                    "anix: warning: ANIX_TRACE_LEVEL=wire is forbidden on shared journal.sock; downgrading to `deltas`"
                );
                level = TraceLevel::Deltas;
            }
            return Self {
                agent_name,
                pid,
                ppid,
                level,
                sink: TraceSink::JournalSock(stream),
            };
        }

        Self {
            agent_name,
            pid,
            ppid,
            level: TraceLevel::Events,
            sink: TraceSink::Disabled,
        }
    }

    pub fn is_enabled(&self) -> bool {
        !matches!(self.sink, TraceSink::Disabled)
    }

    pub fn level(&self) -> TraceLevel {
        self.level
    }

    pub fn emit_event(&mut self, event_type: &str, fields: Map<String, Value>) {
        self.emit(TraceLevel::Events, event_type, fields);
    }

    pub fn emit_delta(&mut self, event_type: &str, text: &str) {
        if self.level < TraceLevel::Deltas {
            return;
        }
        let mut fields = Map::new();
        fields.insert("text".into(), Value::String(text.to_owned()));
        self.emit(TraceLevel::Deltas, event_type, fields);
    }

    pub fn emit_wire(&mut self, event_type: &str, fields: Map<String, Value>) {
        // Double-guard: never emit `wire` frames to shared `journal.sock`.
        if self.level < TraceLevel::Wire || matches!(self.sink, TraceSink::JournalSock(_)) {
            return;
        }
        self.emit(TraceLevel::Wire, event_type, fields);
    }

    fn emit(&mut self, min_level: TraceLevel, event_type: &str, fields: Map<String, Value>) {
        if matches!(self.sink, TraceSink::Disabled) || self.level < min_level {
            return;
        }

        let envelope = TraceEnvelope {
            v: TRACE_VERSION,
            ts: posix::timestamp_rfc3339_utc(),
            pid: self.pid,
            ppid: self.ppid,
            agent: self.agent_name.clone(),
            event_type: event_type.to_owned(),
            fields,
        };

        let Ok(mut line) = serde_json::to_string(&envelope) else {
            return;
        };
        line.push('\n');

        let write_result = match &mut self.sink {
            TraceSink::Disabled => return,
            TraceSink::Fd(f) => f.write_all(line.as_bytes()).and_then(|_| f.flush()),
            TraceSink::JournalSock(s) => s.write_all(line.as_bytes()),
        };

        if let Err(err) = write_result {
            // Section 9.5: On EPIPE / closed peer, silently disable trace writer.
            if matches!(
                err.kind(),
                std::io::ErrorKind::BrokenPipe
                    | std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::NotConnected
            ) {
                self.sink = TraceSink::Disabled;
            }
        }
    }
}

fn parse_trace_level_env() -> Option<TraceLevel> {
    let raw = env::var("ANIX_TRACE_LEVEL").ok()?;
    match raw.trim().to_ascii_lowercase().as_str() {
        "events" => Some(TraceLevel::Events),
        "deltas" => Some(TraceLevel::Deltas),
        "wire" => Some(TraceLevel::Wire),
        _ => None,
    }
}
