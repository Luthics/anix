//! Deterministic 4-step environment self-check for `agent --doctor` (Section 10.3).

use std::env;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::os::unix::io::{AsRawFd, FromRawFd};

use os_agent_proto::exit_code::ExitCode;
use uuid::Uuid;

use crate::config::{
    AgentPaths, ApiKeySource, ProviderConfig, default_providers, load_config_file, merge_providers,
};
use crate::platform::{self, posix};

pub fn run() -> ExitCode {
    let cwd = match env::current_dir() {
        Ok(c) => c,
        Err(err) => {
            eprintln!("[FAIL] cannot resolve cwd: {err}");
            return ExitCode::RuntimeError;
        }
    };
    let paths = match AgentPaths::discover(&cwd) {
        Ok(p) => p,
        Err(err) => {
            eprintln!("[FAIL] path discovery failed: {err}");
            return ExitCode::RuntimeError;
        }
    };

    let mut all_ok = true;

    // Step 1: Verify FHS binaries exist and have `+x` permissions.
    let default_provider = check_config(&paths);
    let default_provider_bin = default_provider
        .as_ref()
        .map_or("agent-provider-openai-responses", |p| p.bin.as_str());
    if !check_binaries(&paths, default_provider_bin) {
        all_ok = false;
    }

    // Step 2: Config & default provider API key check (`api_key_env` ENV takes precedence over `api_key`).
    if let Some(ref prov) = default_provider {
        match prov.resolve_api_key().1 {
            ApiKeySource::Env(env_name) => {
                println!("[OK]   2. config.toml valid and `{env_name}` is set");
            }
            ApiKeySource::Config => {
                println!("[OK]   2. config.toml valid and `api_key` is configured");
            }
            ApiKeySource::NoneRequired => {
                println!("[OK]   2. config.toml valid (no api_key required)");
            }
            ApiKeySource::Missing(env_name) => {
                eprintln!(
                    "[FAIL] 2. default provider API key is missing (`{env_name}` is unset and `api_key` is not set)"
                );
                eprintln!(
                    "       fix: export {env_name}=\"<your-api-key>\" or set `api_key` in `{}`",
                    paths.global_config_file().display()
                );
                all_ok = false;
            }
        }
    } else {
        all_ok = false;
    }

    // Step 3: Verify `/dev/fd` mount and anonymous pipe relay round-trip.
    if check_dev_fd_pipe() {
        println!("[OK]   3. /dev/fd mount and anonymous pipe relay operational");
    } else {
        eprintln!("[FAIL] 3. /dev/fd mount or anonymous pipe relay failed");
        eprintln!(
            "       fix: ensure fdescfs (/dev/fd) or procfs (/proc/self/fd -> /dev/fd) is mounted"
        );
        all_ok = false;
    }

    // Step 4: Verify `$XDG_RUNTIME_DIR/agent` (`0700`) and `flock` exclusive lock semantics.
    if check_runtime_dir_and_flock(&paths) {
        println!(
            "[OK]   4. runtime dir `{}` (0700) and flock(LOCK_EX|LOCK_NB) operational",
            paths.runtime_dir.display()
        );
    } else {
        all_ok = false;
    }

    let caps = platform::probe_capabilities();
    println!(
        "[INFO] platform={}-{} dev_fd={} pidfd={} landlock_abi={:?}",
        std::env::consts::OS,
        std::env::consts::ARCH,
        caps.dev_fd_available,
        caps.pidfd_supported,
        caps.landlock_abi,
    );

    if all_ok {
        ExitCode::Success
    } else {
        ExitCode::RuntimeError
    }
}

fn check_binaries(paths: &AgentPaths, default_provider_bin: &str) -> bool {
    let required = [
        paths.home_dir.join(".local/bin/agent"),
        paths.libexec_dir.join("msh"),
        paths.libexec_dir.join(default_provider_bin),
        paths.tools_dir.join("aview"),
        paths.tools_dir.join("aclr"),
    ];

    let mut missing = Vec::new();
    for p in &required {
        if !posix::is_executable_file(p) {
            missing.push(p.display().to_string());
        }
    }

    if missing.is_empty() {
        println!("[OK]   1. FHS binaries present and executable (+x)");
        true
    } else {
        eprintln!("[FAIL] 1. missing or non-executable FHS binaries:");
        for m in &missing {
            eprintln!("       - {m}");
        }
        eprintln!("       fix: run `./scripts/install-local.sh` (or `chmod +x <path>`)");
        false
    }
}

fn check_config(paths: &AgentPaths) -> Option<ProviderConfig> {
    let global_path = paths.global_config_file();
    let global_cfg = match load_config_file(&global_path) {
        Ok(c) => c.unwrap_or_default(),
        Err(err) => {
            eprintln!("[FAIL] 2. global config invalid: {err}");
            eprintln!("       fix: fix TOML syntax in `{}`", global_path.display());
            return None;
        }
    };

    let project_cfg = match paths.project_config_file() {
        Some(p) => match load_config_file(&p) {
            Ok(c) => c.unwrap_or_default(),
            Err(err) => {
                eprintln!("[FAIL] 2. project config invalid: {err}");
                eprintln!("       fix: fix TOML syntax in `{}`", p.display());
                return None;
            }
        },
        None => Default::default(),
    };

    let default_prov_name = project_cfg
        .defaults
        .provider
        .or(global_cfg.defaults.provider)
        .unwrap_or_else(|| "openai-responses".into());

    let mut provs = default_providers();
    merge_providers(&mut provs, &global_cfg.providers, false);
    merge_providers(&mut provs, &project_cfg.providers, true);

    let Some(prov) = provs.get(&default_prov_name) else {
        eprintln!("[FAIL] 2. default provider `{default_prov_name}` not found in [providers.*]");
        eprintln!(
            "       fix: define [providers.{default_prov_name}] in `{}`",
            global_path.display()
        );
        return None;
    };

    Some(prov.clone())
}

fn check_dev_fd_pipe() -> bool {
    if !posix::dev_fd_available() {
        return false;
    }
    let Ok((r_fd, w_fd)) = posix::create_pipe_cloexec() else {
        return false;
    };

    // Open `/dev/fd/<w_fd>` directly, write probe bytes, close both write ends, verify read.
    let dev_fd_path = format!("/dev/fd/{w_fd}");
    let write_ok = OpenOptions::new()
        .write(true)
        .open(&dev_fd_path)
        .and_then(|mut f| f.write_all(b"os-agent-doctor-ok"))
        .is_ok();

    // SAFETY: Close original pipe write end so reader sees EOF.
    unsafe {
        libc::close(w_fd);
    }

    // SAFETY: `r_fd` is exclusively owned by this check.
    let mut reader = unsafe { fs::File::from_raw_fd(r_fd) };
    let mut buf = Vec::new();
    let read_ok = reader.read_to_end(&mut buf).is_ok();

    write_ok && read_ok && buf == b"os-agent-doctor-ok"
}

fn check_runtime_dir_and_flock(paths: &AgentPaths) -> bool {
    if let Err(err) = posix::ensure_dir_mode(&paths.runtime_dir, 0o700) {
        eprintln!(
            "[FAIL] 4. cannot prepare runtime dir `{}` (0700): {err}",
            paths.runtime_dir.display()
        );
        eprintln!(
            "       fix: mkdir -p {} && chmod 0700 {}",
            paths.runtime_dir.display(),
            paths.runtime_dir.display()
        );
        return false;
    }

    let mode = fs::metadata(&paths.runtime_dir)
        .map(|m| m.permissions().mode() & 0o777)
        .unwrap_or(0);
    if mode != 0o700 {
        eprintln!(
            "[FAIL] 4. runtime dir `{}` has mode {mode:04o} (expected 0700)",
            paths.runtime_dir.display()
        );
        eprintln!("       fix: chmod 0700 {}", paths.runtime_dir.display());
        return false;
    }

    let lock_path = paths
        .runtime_dir
        .join(format!("doctor_flock_{}.lock", Uuid::new_v4()));
    let f1 = match OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&lock_path)
    {
        Ok(f) => f,
        Err(err) => {
            eprintln!("[FAIL] 4. cannot create flock probe file: {err}");
            return false;
        }
    };
    let f2 = match OpenOptions::new().read(true).write(true).open(&lock_path) {
        Ok(f) => f,
        Err(err) => {
            let _ = fs::remove_file(&lock_path);
            eprintln!("[FAIL] 4. cannot reopen flock probe file: {err}");
            return false;
        }
    };

    let first_ok = posix::flock_exclusive_nonblocking(f1.as_raw_fd()).unwrap_or(false);
    let second_blocked = matches!(
        posix::flock_exclusive_nonblocking(f2.as_raw_fd()),
        Ok(false)
    );

    drop(f2);
    drop(f1);
    let _ = fs::remove_file(&lock_path);

    if first_ok && second_blocked {
        true
    } else {
        eprintln!(
            "[FAIL] 4. flock(LOCK_EX|LOCK_NB) failed on `{}` (first={first_ok}, second_blocked={second_blocked})",
            paths.runtime_dir.display()
        );
        eprintln!("       fix: set XDG_RUNTIME_DIR to a local filesystem supporting flock(2)");
        false
    }
}
