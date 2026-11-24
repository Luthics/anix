//! Structured JSONL Trace sink (`fd 3` / `$XDG_RUNTIME_DIR/agent/journal.sock`, Section 9.5).
//!
//! - Lazy activation: disabled unless `fd 3` is writable or `journal.sock` exists.
//! - EPIPE / broken pipe silently closes the trace sink without blocking or changing exit codes.
//! - `wire` level is strictly forbidden on shared `journal.sock`.

use std::env;
use std::fs::File;
use std::io::Write;
use std::os::unix::io::FromRawFd;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use os_agent_proto::TRACE_VERSION;
use os_agent_proto::trace::{TraceEnvelope, TraceLevel};
use serde_json::{Map, Value};

use crate::platform::posix;

#[derive(Debug)]
enum TraceSink {
    Disabled,
    Fd3(File),
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
    /// Claim inherited writable `fd 3` immediately before opening any internal file descriptors.
    pub fn take_inherited_fd3() -> Option<File> {
        if posix::fd_is_writable(3) {
            let _ = posix::set_cloexec(3);
            // SAFETY: `fd 3` was explicitly opened writable by the caller and is claimed once.
            Some(unsafe { File::from_raw_fd(3) })
        } else {
            None
        }
    }

    /// Initialize Tracer from a pre-claimed `fd 3` or fallback to `$XDG_RUNTIME_DIR/agent/journal.sock`.
    pub fn init(agent_name: impl Into<String>, fd3: Option<File>, journal_sock: &Path) -> Self {
        let agent_name = agent_name.into();
        let pid = std::process::id();
        let ppid = posix::current_ppid();
        let env_level = parse_trace_level_env();

        if let Some(file) = fd3 {
            let level = env_level.unwrap_or(TraceLevel::Deltas);
            return Self {
                agent_name,
                pid,
                ppid,
                level,
                sink: TraceSink::Fd3(file),
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
                    "agent: warning: AGENT_TRACE_LEVEL=wire is forbidden on shared journal.sock; downgrading to `deltas`"
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
            TraceSink::Fd3(f) => f.write_all(line.as_bytes()).and_then(|_| f.flush()),
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
    let raw = env::var("AGENT_TRACE_LEVEL").ok()?;
    match raw.trim().to_ascii_lowercase().as_str() {
        "events" => Some(TraceLevel::Events),
        "deltas" => Some(TraceLevel::Deltas),
        "wire" => Some(TraceLevel::Wire),
        _ => None,
    }
}
