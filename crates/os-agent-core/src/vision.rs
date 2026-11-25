//! Non-vision model protection and optional external `[vision].bin` image-to-text translator
//! (v1.1 Section 5.5).

use std::io::{Read, Write};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use os_agent_proto::exit_code::ExitCode;
use os_agent_proto::hooks::{PLUGIN_WIRE_VERSION, VisionBridgeInput};

use crate::config::ResolvedConfig;
use crate::platform::{self, ChildWaitOutcome};
use crate::signal::SignalState;
use crate::trust;

/// Deterministic error message returned to the model when `vision = false` and no `[vision].bin`
/// is configured (v1.1 §5.5).
pub const NO_NATIVE_VISION_ERROR: &str = "Error: current model does not support native vision (vision=false). Use text tools (e.g. OCR/pdftotext) or delegate to a vision-capable sub-agent.";

/// Result of invoking `[vision].bin` for a single `aview` media control frame.
#[derive(Debug)]
pub enum VisionBridgeOutcome {
    /// Successfully translated to `<vision_description source="...">...</vision_description>`.
    Translated(String),
    /// Bridge failed or timed out: discard media frame and report `is_error = true` to model
    /// without terminating `agent`.
    ToolError(String),
    /// Project-level `[vision].bin` failed `agent trust` SHA-256 verification (Fail-Closed `exit 75`).
    TrustViolation(String),
    /// Interrupted by `SIGINT` / `SIGTERM`.
    Interrupted(ExitCode),
}

/// Invoke `[vision].bin` to translate a local image produced by `aview` into plain text.
pub fn run_vision_bridge(
    cfg: &ResolvedConfig,
    mime: &str,
    path: &str,
    source: Option<&str>,
    signals: &SignalState,
) -> VisionBridgeOutcome {
    let Some(ref bin_spec) = cfg.vision_bridge.bin else {
        return VisionBridgeOutcome::ToolError(NO_NATIVE_VISION_ERROR.to_owned());
    };

    let base_dir = cfg
        .paths
        .project_agent_dir
        .as_deref()
        .and_then(|d| d.parent());
    let bin_path = cfg.paths.resolve_component_bin(bin_spec, base_dir);

    if cfg.vision_bridge.bin_from_project
        && let Err(err) = trust::verify_trusted(&bin_path, &cfg.paths)
    {
        return VisionBridgeOutcome::TrustViolation(err.to_string());
    }

    let payload = VisionBridgeInput {
        v: PLUGIN_WIRE_VERSION,
        mime: mime.to_owned(),
        path: path.to_owned(),
        source: source.map(ToOwned::to_owned),
        cwd: cfg.paths.cwd.display().to_string(),
    };
    let mut input_line = match serde_json::to_string(&payload) {
        Ok(s) => s,
        Err(err) => {
            return VisionBridgeOutcome::ToolError(format!(
                "Error: failed to serialize [vision].bin payload: {err}"
            ));
        }
    };
    input_line.push('\n');

    let parent_pid = unsafe { libc::getpid() };
    let mut cmd = Command::new(&bin_path);
    cmd.current_dir(&cfg.paths.cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // SAFETY: `pre_exec` invokes only async-signal-safe syscalls to isolate PGID and set PDEATHSIG.
    unsafe {
        cmd.pre_exec(move || {
            if libc::setpgid(0, 0) == -1 {
                return Err(std::io::Error::last_os_error());
            }
            #[cfg(target_os = "linux")]
            {
                crate::platform::linux::set_pdeathsig_sigkill(parent_pid)?;
            }
            #[cfg(not(target_os = "linux"))]
            let _ = parent_pid;
            Ok(())
        });
    }

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(err) => {
            return VisionBridgeOutcome::ToolError(format!(
                "Error: failed to execute [vision].bin `{}`: {err}",
                bin_path.display()
            ));
        }
    };

    let child_pgid = child.id() as libc::pid_t;
    signals.set_active_child_pgid(child_pgid);

    if let Some(mut stdin) = child.stdin.take() {
        let bytes = input_line.into_bytes();
        thread::spawn(move || {
            let _ = stdin.write_all(&bytes);
        });
    }

    let mut stdout_pipe = child.stdout.take();
    let mut stderr_pipe = child.stderr.take();
    let out_handle = thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(ref mut p) = stdout_pipe {
            let _ = p.read_to_end(&mut buf);
        }
        buf
    });
    let err_handle = thread::spawn(move || {
        let mut s = String::new();
        if let Some(ref mut p) = stderr_pipe {
            let _ = p.read_to_string(&mut s);
        }
        s
    });

    let timeout = Duration::from_secs(cfg.vision_bridge.timeout_sec.max(1));
    let deadline = Instant::now() + timeout;

    let status = loop {
        if let Some(code) = signals.termination_exit_code() {
            let _ = signals.terminate_active_child_and_wait(&mut child);
            signals.clear_active_child_pgid();
            return VisionBridgeOutcome::Interrupted(code);
        }
        match platform::wait_child_until(&mut child, Some(deadline), Some(signals.wakeup_fd())) {
            Ok(ChildWaitOutcome::Exited(st)) => break st,
            Ok(ChildWaitOutcome::SignalWakeup) => {
                signals.drain_wakeup();
                continue;
            }
            Ok(ChildWaitOutcome::TimedOut) => {
                let _ = signals.terminate_active_child_and_wait(&mut child);
                signals.clear_active_child_pgid();
                let stderr_str = err_handle.join().unwrap_or_default();
                let detail = stderr_str.trim();
                let msg = if detail.is_empty() {
                    format!(
                        "Error: [vision].bin timed out after {}s",
                        cfg.vision_bridge.timeout_sec
                    )
                } else {
                    format!(
                        "Error: [vision].bin timed out after {}s: {detail}",
                        cfg.vision_bridge.timeout_sec
                    )
                };
                return VisionBridgeOutcome::ToolError(msg);
            }
            Err(err) => {
                let _ = signals.terminate_active_child_and_wait(&mut child);
                signals.clear_active_child_pgid();
                return VisionBridgeOutcome::ToolError(format!(
                    "Error: waiting on [vision].bin failed: {err}"
                ));
            }
        }
    };

    signals.clear_active_child_pgid();
    let stdout_bytes = out_handle.join().unwrap_or_default();
    let stderr_str = err_handle.join().unwrap_or_default();

    if !status.success() {
        let code = status.code().unwrap_or(1);
        let detail = stderr_str.trim();
        let msg = if detail.is_empty() {
            format!("Error: [vision].bin exited with status {code}")
        } else {
            format!("Error: [vision].bin exited with status {code}: {detail}")
        };
        return VisionBridgeOutcome::ToolError(msg);
    }

    let Ok(text) = String::from_utf8(stdout_bytes) else {
        return VisionBridgeOutcome::ToolError(
            "Error: [vision].bin produced non-UTF-8 stdout".into(),
        );
    };
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return VisionBridgeOutcome::ToolError(
            "Error: [vision].bin produced empty description".into(),
        );
    }

    let src_attr = source
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(path)
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");

    VisionBridgeOutcome::Translated(format!(
        "<vision_description source=\"{src_attr}\">{trimmed}</vision_description>"
    ))
}
