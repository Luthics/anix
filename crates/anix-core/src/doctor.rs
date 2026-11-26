//! Deterministic 4-step environment self-check for `anix --doctor` (`docs/RUNTIME.md` §8).

use std::env;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::os::unix::io::{AsRawFd, FromRawFd};
use std::path::Path;

use anix_proto::exit_code::ExitCode;
use uuid::Uuid;

use crate::config::{
    AgentPaths, ApiKeySource, ProviderConfig, default_providers, load_config_file, merge_providers,
    split_shell_words,
};
use crate::platform::{self, posix};
use crate::strict_modes;

pub fn run(cli_config: Option<&Path>) -> ExitCode {
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

    let (config_file, must_exist) = paths.resolve_config_file(cli_config);
    let mut all_ok = true;

    let (default_provider, default_shell) = check_config(&config_file, must_exist);
    let default_provider_bin = default_provider
        .as_ref()
        .map_or("anix-provider-openai-responses", |p| p.bin.as_str());

    // Step 1: Verify configured shell, default provider, and StrictModes.
    if !check_binaries(
        &paths,
        default_shell.as_deref().unwrap_or("msh"),
        default_provider_bin,
    ) {
        all_ok = false;
    }

    // Step 2: Config & default provider API key check.
    if let Some(ref prov) = default_provider {
        match prov.resolve_api_key().1 {
            ApiKeySource::Env(env_name) => {
                println!(
                    "[OK]   2. config `{}` valid and `{env_name}` is set",
                    config_file.display()
                );
            }
            ApiKeySource::Config => {
                println!(
                    "[OK]   2. config `{}` valid and `api_key` is configured",
                    config_file.display()
                );
            }
            ApiKeySource::NoneRequired => {
                println!(
                    "[OK]   2. config `{}` valid (no api_key required)",
                    config_file.display()
                );
            }
            ApiKeySource::Missing(env_name) => {
                eprintln!(
                    "[FAIL] 2. default provider API key is missing (`{env_name}` is unset and `api_key` is not set)"
                );
                eprintln!(
                    "       fix: export {env_name}=\"<your-api-key>\" or set `api_key` in `{}`",
                    config_file.display()
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

    // Step 4: Verify `$XDG_RUNTIME_DIR/anix` (`0700`) and `flock` exclusive lock semantics.
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

fn check_binaries(paths: &AgentPaths, default_shell: &str, default_provider_bin: &str) -> bool {
    let mut issues = Vec::new();

    if !default_shell.trim().eq_ignore_ascii_case("none") {
        let parts = split_shell_words(default_shell);
        if let Some(prog) = parts.first() {
            let resolved = paths.resolve_component_bin(prog);
            if !posix::is_executable_file(&resolved) {
                issues.push(format!(
                    "configured shell `{prog}` not found or not executable (resolved `{}`)",
                    resolved.display()
                ));
            }
        }
    }

    let prov_resolved = paths.resolve_component_bin(default_provider_bin);
    if !posix::is_executable_file(&prov_resolved) {
        issues.push(format!(
            "default provider `{default_provider_bin}` not found or not executable (resolved `{}`)",
            prov_resolved.display()
        ));
    } else if let Err(err) = strict_modes::verify_file(&prov_resolved) {
        issues.push(err.to_string());
    }

    if issues.is_empty() {
        println!(
            "[OK]   1. configured shell and provider binaries present, executable, and StrictModes-clean"
        );
        true
    } else {
        eprintln!("[FAIL] 1. component binary check failed:");
        for msg in &issues {
            eprintln!("       - {msg}");
        }
        eprintln!(
            "       fix: run `./scripts/install-local.sh` or check permissions (`chmod go-w`)"
        );
        false
    }
}

fn check_config(config_file: &Path, must_exist: bool) -> (Option<ProviderConfig>, Option<String>) {
    let file_cfg = match load_config_file(config_file, must_exist) {
        Ok(c) => c.unwrap_or_default(),
        Err(err) => {
            eprintln!("[FAIL] 2. config invalid: {err}");
            eprintln!("       fix: fix TOML syntax in `{}`", config_file.display());
            return (None, None);
        }
    };

    let default_shell = file_cfg.defaults.shell.clone();
    let default_prov_name = file_cfg
        .defaults
        .provider
        .unwrap_or_else(|| "openai-responses".into());

    let mut provs = default_providers();
    if let Err(err) = merge_providers(&mut provs, &file_cfg.providers) {
        eprintln!("[FAIL] 2. provider config invalid: {err}");
        return (None, default_shell);
    }

    let Some(prov) = provs.get(&default_prov_name) else {
        eprintln!("[FAIL] 2. default provider `{default_prov_name}` not found in [providers.*]");
        eprintln!(
            "       fix: define [providers.{default_prov_name}] in `{}`",
            config_file.display()
        );
        return (None, default_shell);
    };

    (Some(prov.clone()), default_shell)
}

fn check_dev_fd_pipe() -> bool {
    if !posix::dev_fd_available() {
        return false;
    }
    let Ok((r_fd, w_fd)) = posix::create_pipe_cloexec() else {
        return false;
    };

    let dev_fd_path = format!("/dev/fd/{w_fd}");
    let write_ok = OpenOptions::new()
        .write(true)
        .open(&dev_fd_path)
        .and_then(|mut f| f.write_all(b"anix-doctor-ok"))
        .is_ok();

    // SAFETY: Close original pipe write end so reader sees EOF.
    unsafe {
        libc::close(w_fd);
    }

    // SAFETY: `r_fd` is exclusively owned by this check.
    let mut reader = unsafe { fs::File::from_raw_fd(r_fd) };
    let mut buf = Vec::new();
    let read_ok = reader.read_to_end(&mut buf).is_ok();

    write_ok && read_ok && buf == b"anix-doctor-ok"
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
