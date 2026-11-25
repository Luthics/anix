//! Project-level RCE gate (`agent trust <PATH>`) with `realpath` + SHA-256 verification
//! persisted in `$XDG_STATE_HOME/agent/trusted.json` (Section 8.3).

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use os_agent_proto::exit_code::ExitCode;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::config::{AgentPaths, load_config_file};
use crate::platform::posix;

const TRUST_SCHEMA_VERSION: u32 = 1;
const HOOK_STAGES: &[&str] = &["pre-context.d", "post-tool.d", "on-exit.d"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustedRegistry {
    pub version: u32,
    #[serde(default)]
    pub hooks: BTreeMap<String, TrustedEntry>,
}

impl Default for TrustedRegistry {
    fn default() -> Self {
        Self {
            version: TRUST_SCHEMA_VERSION,
            hooks: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustedEntry {
    pub sha256: String,
    pub trusted_at: String,
}

#[derive(Debug, Error)]
pub enum TrustError {
    #[error("untrusted project executable `{0}` (run `agent trust <project_dir>` first)")]
    Untrusted(String),
    #[error("SHA-256 mismatch for trusted executable `{0}` (file changed; re-run `agent trust`)")]
    HashMismatch(String),
    #[error("trust registry I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid trusted.json format: {0}")]
    Json(#[from] serde_json::Error),
}

/// CLI handler for `agent trust <PATH>`.
pub fn trust_path(path: &Path) -> ExitCode {
    let cwd = match std::env::current_dir() {
        Ok(c) => c,
        Err(err) => {
            eprintln!("agent trust: cannot determine cwd: {err}");
            return ExitCode::RuntimeError;
        }
    };
    let paths = match AgentPaths::discover(&cwd) {
        Ok(p) => p,
        Err(err) => {
            eprintln!("agent trust: {err}");
            return ExitCode::RuntimeError;
        }
    };

    let targets = match collect_trust_targets(path, &paths) {
        Ok(t) => t,
        Err(msg) => {
            eprintln!("agent trust: {msg}");
            return ExitCode::RuntimeError;
        }
    };

    if targets.is_empty() {
        eprintln!(
            "agent trust: no executable project hooks or custom bins found under `{}`",
            path.display()
        );
        return ExitCode::Success;
    }

    let reg_path = paths.trusted_registry_file();
    let mut reg = load_registry(&reg_path).unwrap_or_default();
    let now = posix::timestamp_rfc3339_utc();

    for target in &targets {
        let canonical = match fs::canonicalize(target) {
            Ok(p) => p,
            Err(err) => {
                eprintln!("agent trust: cannot resolve `{}`: {err}", target.display());
                return ExitCode::RuntimeError;
            }
        };
        let digest = match sha256_file(&canonical) {
            Ok(d) => d,
            Err(err) => {
                eprintln!("agent trust: cannot hash `{}`: {err}", canonical.display());
                return ExitCode::RuntimeError;
            }
        };
        let key = canonical.display().to_string();
        reg.hooks.insert(
            key.clone(),
            TrustedEntry {
                sha256: digest.clone(),
                trusted_at: now.clone(),
            },
        );
        println!("trusted {key} (sha256={digest})");
    }

    if let Err(err) = save_registry(&reg_path, &reg) {
        eprintln!(
            "agent trust: failed to save `{}`: {err}",
            reg_path.display()
        );
        return ExitCode::RuntimeError;
    }

    ExitCode::Success
}

/// Verify that `executable` is recorded in `trusted.json` with matching `realpath` & SHA-256.
pub fn verify_trusted(executable: &Path, paths: &AgentPaths) -> Result<(), TrustError> {
    let canonical = fs::canonicalize(executable)?;
    let key = canonical.display().to_string();
    let reg = load_registry(&paths.trusted_registry_file())?;

    let Some(entry) = reg.hooks.get(&key) else {
        return Err(TrustError::Untrusted(key));
    };
    let actual_sha = sha256_file(&canonical)?;
    if !actual_sha.eq_ignore_ascii_case(&entry.sha256) {
        return Err(TrustError::HashMismatch(key));
    }
    Ok(())
}

/// Check if a file is a systemd-style mask (0 bytes or symlink pointing to `/dev/null`).
pub fn is_hook_mask(path: &Path) -> bool {
    if let Ok(target) = fs::read_link(path)
        && target == Path::new("/dev/null")
    {
        return true;
    }
    fs::metadata(path)
        .map(|m| m.is_file() && m.len() == 0)
        .unwrap_or(false)
}

fn collect_trust_targets(input: &Path, paths: &AgentPaths) -> Result<Vec<PathBuf>, String> {
    let resolved = if input.is_absolute() {
        input.to_path_buf()
    } else {
        paths.cwd.join(input)
    };
    if !resolved.exists() {
        return Err(format!("path `{}` does not exist", resolved.display()));
    }

    // Single file trust support.
    if resolved.is_file() {
        return Ok(vec![resolved]);
    }

    // Determine `.agent` directory (user may pass project root OR `.agent` directly).
    let agent_dir = if resolved.file_name().is_some_and(|n| n == ".agent") {
        resolved
    } else if resolved.join(".agent").is_dir() {
        resolved.join(".agent")
    } else {
        return Err(format!(
            "no `.agent/` directory found under `{}`",
            resolved.display()
        ));
    };

    let project_root = agent_dir.parent().unwrap_or(&paths.cwd);
    let mut targets = Vec::new();

    // 1. All non-masked executable scripts in `.agent/hooks/<stage>.d/`.
    let hooks_root = agent_dir.join("hooks");
    for stage in HOOK_STAGES {
        let stage_dir = hooks_root.join(stage);
        let Ok(entries) = fs::read_dir(&stage_dir) else {
            continue;
        };
        let mut stage_files: Vec<PathBuf> =
            entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
        stage_files.sort();
        for file in stage_files {
            if is_hook_mask(&file) {
                continue;
            }
            if posix::is_executable_file(&file) {
                targets.push(file);
            }
        }
    }

    // 2. Project-configured `[approval].bin`, `[vision].bin`, `[compact].bin`, and
    //    `[providers.<name>].bin` in `.agent/config.toml` (v1.1 §8.3).
    let cfg_path = agent_dir.join("config.toml");
    if let Ok(Some(cfg)) = load_config_file(&cfg_path) {
        for bin in [
            cfg.approval.bin.as_deref(),
            cfg.vision.bin.as_deref(),
            cfg.compact.bin.as_deref(),
        ]
        .into_iter()
        .flatten()
        .filter(|s| !s.trim().is_empty())
        {
            let resolved_bin = paths.resolve_component_bin(bin, Some(project_root));
            if resolved_bin.is_file() {
                targets.push(resolved_bin);
            }
        }
        for raw_prov in cfg.providers.values() {
            if let Some(bin) = raw_prov.bin.as_deref().filter(|s| !s.trim().is_empty()) {
                let resolved_bin = paths.resolve_component_bin(bin, Some(project_root));
                if resolved_bin.is_file() {
                    targets.push(resolved_bin);
                }
            }
        }
    }

    Ok(targets)
}

fn load_registry(path: &Path) -> Result<TrustedRegistry, TrustError> {
    if !path.exists() {
        return Ok(TrustedRegistry::default());
    }
    let raw = fs::read_to_string(path)?;
    if raw.trim().is_empty() {
        return Ok(TrustedRegistry::default());
    }
    let reg: TrustedRegistry = serde_json::from_str(&raw)?;
    Ok(reg)
}

fn save_registry(path: &Path, reg: &TrustedRegistry) -> Result<(), TrustError> {
    let mut json = serde_json::to_string_pretty(reg)?;
    json.push('\n');
    posix::write_atomic_0600(path, json.as_bytes())?;
    Ok(())
}

pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut f = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for b in digest {
        use std::fmt::Write;
        let _ = write!(&mut hex, "{b:02x}");
    }
    Ok(hex)
}
