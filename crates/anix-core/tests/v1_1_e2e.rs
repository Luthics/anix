//! End-to-end integration tests for Anix v1.1 features:
//! - `--trace-fd=<N>` / no implicit `fd 3` / `cycle_start`, `cycle_end`, `idle` Trace events
//! - Standardized exit code `75` (`EX_TEMPFAIL`) & external `[approval].bin` + `anix trust`
//! - `vision = false` WAL protection, `[vision].bin` text bridge, & `how-to-mcp.md` built-in tip
//! - `aclr -t <N>` (omitted summary) auto-summary tail fold + `fold_usage` accounting
//! - Persistent Provider `coprocess` reuse, pure event-driven `--inbox`, & `fd 2` newline contract

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::io::{AsRawFd, RawFd};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use serde_json::Value;

fn agent_bin() -> PathBuf {
    let compile_time = PathBuf::from(env!("CARGO_BIN_EXE_anix"));
    if compile_time.is_file() {
        return compile_time;
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        for cand in [
            dir.join("anix"),
            dir.parent().map(|p| p.join("bin/anix")).unwrap_or_default(),
        ] {
            if cand.is_file() {
                return cand;
            }
        }
    }
    compile_time
}

fn target_debug_dir() -> PathBuf {
    agent_bin()
        .parent()
        .expect("anix binary parent dir")
        .to_path_buf()
}

fn make_temp_env(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "anix-v11-{tag}-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    fs::create_dir_all(&dir).expect("create temp test dir");
    dir
}

fn write_exec(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent dir");
    }
    fs::write(path, content).expect("write executable script");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("chmod +x");
}

fn setup_fhs_home(root: &Path) -> PathBuf {
    let home = root.join("home");
    let libexec = home.join(".local/libexec/anix");
    let tools = libexec.join("tools");
    fs::create_dir_all(&tools).expect("create libexec/tools");

    let bin_dir = target_debug_dir();
    let msh_src = bin_dir.join("msh");
    let aclr_src = bin_dir.join("aclr");
    if msh_src.is_file() {
        let _ = std::os::unix::fs::symlink(&msh_src, libexec.join("msh"));
    } else {
        write_exec(
            &libexec.join("msh"),
            "#!/usr/bin/env sh\nexec /bin/sh \"$@\"\n",
        );
    }
    if aclr_src.is_file() {
        let _ = std::os::unix::fs::symlink(&aclr_src, tools.join("aclr"));
    } else {
        write_exec(
            &tools.join("aclr"),
            r#"#!/usr/bin/env sh
if [ "$1" = "-s" ]; then
  printf '{"op":"compact"}\n' > "$ANIX_CTL"
  exit 0
fi
if [ "$1" = "-t" ] && [ "$#" -eq 2 ]; then
  printf '{"op":"fold","from_turn":%s}\n' "$2" > "$ANIX_CTL"
  exit 0
fi
if [ "$1" = "-t" ] && [ "$#" -eq 3 ]; then
  printf '{"op":"fold","from_turn":%s,"summary":"%s"}\n' "$2" "$3" > "$ANIX_CTL"
  exit 0
fi
exit 2
"#,
        );
    }
    home
}

fn base_cmd(home: &Path, workdir: &Path) -> Command {
    let mut cmd = Command::new(agent_bin());
    cmd.current_dir(workdir)
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("XDG_STATE_HOME", home.join(".local/state"))
        .env("XDG_RUNTIME_DIR", home.join("runtime"))
        .env_remove("ANIX_TRACE_FD")
        .env_remove("ANIX_TRACE_LEVEL")
        .env_remove("ANIX_VISION")
        .env_remove("ANIX_APPROVAL")
        .env_remove("ANIX_OUTPUT")
        .env_remove("ANIX_SHELL")
        .env_remove("ANIX_STDERR")
        .env_remove("ANIX_YES")
        .env_remove("ANIX_CONTEXT")
        .env_remove("ANIX_LEAF")
        .env_remove("ANIX_INBOX");
    cmd
}

/// Isolate inherited FDs `3..=9` from the test runner, optionally attaching `file` onto `target_fd`.
fn configure_child_fds(cmd: &mut Command, attach: Option<(&File, RawFd)>) {
    let pair = attach.map(|(f, target)| (f.as_raw_fd(), target));
    // SAFETY: `pre_exec` uses only async-signal-safe `fcntl`/`dup2`/`close`.
    unsafe {
        cmd.pre_exec(move || {
            let keep_fd = if let Some((src_fd, target_fd)) = pair {
                let relocated = if (3..=9).contains(&src_fd) && src_fd != target_fd {
                    let dup = libc::fcntl(src_fd, libc::F_DUPFD_CLOEXEC, 20);
                    if dup < 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    dup
                } else {
                    src_fd
                };
                if relocated != target_fd {
                    if libc::dup2(relocated, target_fd) < 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                } else {
                    let flags = libc::fcntl(target_fd, libc::F_GETFD);
                    if flags < 0
                        || libc::fcntl(target_fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) < 0
                    {
                        return Err(std::io::Error::last_os_error());
                    }
                }
                Some(target_fd)
            } else {
                None
            };

            for fd in 3..=9 {
                if keep_fd != Some(fd) {
                    libc::close(fd);
                }
            }
            Ok(())
        });
    }
}

#[test]
fn trace_fd_explicit_only_no_implicit_fd3_and_cycle_events() {
    let root = make_temp_env("trace-fd");
    let home = setup_fhs_home(&root);
    let work = root.join("work");
    fs::create_dir_all(&work).unwrap();

    let mock_prov = root.join("mock-provider.sh");
    write_exec(
        &mock_prov,
        r#"#!/usr/bin/env sh
while IFS= read -r line; do
  [ -z "$line" ] && continue
  cat <<'EOF'
{"type":"text_delta","text":"hello-no-nl"}
{"type":"usage","input_tokens":100,"output_tokens":20,"cached_tokens":10}
{"type":"done","finish_reason":"stop"}
EOF
done
"#,
    );

    let cfg_dir = home.join(".config/anix");
    fs::create_dir_all(&cfg_dir).unwrap();
    fs::write(
        cfg_dir.join("config.toml"),
        format!(
            "[defaults]\nprovider = \"mock\"\nmodel = \"m1\"\n\n[providers.mock]\nbin = \"{}\"\nmode = \"coprocess\"\napi_key_env = \"\"\n",
            mock_prov.display()
        ),
    )
    .unwrap();

    let script = work.join("hello.agent");
    fs::write(
        &script,
        "---\nname: trace-demo\napproval: auto\noutput: explicit\n---\nSay hi.\n",
    )
    .unwrap();

    // 1. Open writable fd 3 WITHOUT `--trace-fd`: v1.1 MUST NOT touch fd 3!
    let fd3_path = root.join("fd3_unused.jsonl");
    let fd3_file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&fd3_path)
        .unwrap();
    let mut cmd1 = base_cmd(&home, &work);
    cmd1.arg(&script)
        .arg("hi")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_child_fds(&mut cmd1, Some((&fd3_file, 3)));
    let out1 = cmd1.output().expect("run without --trace-fd");
    drop(fd3_file);
    assert_eq!(out1.status.code(), Some(0));
    assert!(
        fs::read_to_string(&fd3_path).unwrap().is_empty(),
        "v1.1 must NOT implicitly write to fd 3 when --trace-fd is unset"
    );
    // Verify fd 2 newline contract: `hello-no-nl` followed by `\n` before `[usage]` and trailing `\n`.
    let stderr1 = String::from_utf8_lossy(&out1.stderr);
    assert!(
        stderr1.contains("hello-no-nl\n[usage] "),
        "expected newline before [usage], got: {stderr1:?}"
    );
    assert!(stderr1.ends_with('\n'));

    // 2. Closed/unwritable `--trace-fd=7` MUST fail with exit code 2 (`EX_USAGE`).
    let mut cmd_bad = base_cmd(&home, &work);
    cmd_bad
        .args(["--trace-fd", "7"])
        .arg(&script)
        .arg("hi")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_child_fds(&mut cmd_bad, None);
    let out_bad = cmd_bad.output().expect("run invalid trace-fd");
    assert_eq!(out_bad.status.code(), Some(2));

    // 3. Valid `--trace-fd=4` writes full cycle_start/cycle_end JSONL and does NOT treat fd 4 as input.
    let fd4_path = root.join("fd4_trace.jsonl");
    let fd4_file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&fd4_path)
        .unwrap();
    let mut cmd3 = base_cmd(&home, &work);
    cmd3.args(["--trace-fd", "4"])
        .arg(&script)
        .arg("hi")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_child_fds(&mut cmd3, Some((&fd4_file, 4)));
    let out3 = cmd3.output().expect("run with --trace-fd=4");
    drop(fd4_file);
    assert_eq!(out3.status.code(), Some(0));

    let events: Vec<Value> = fs::read_to_string(&fd4_path)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).expect("valid trace json"))
        .collect();
    let types: Vec<&str> = events
        .iter()
        .filter_map(|v| v.get("type").and_then(Value::as_str))
        .collect();
    assert!(types.contains(&"run_start"), "types={types:?}");
    assert!(types.contains(&"cycle_start"), "types={types:?}");
    assert!(types.contains(&"cycle_end"), "types={types:?}");
    assert!(types.contains(&"run_end"), "types={types:?}");

    let cycle_end = events
        .iter()
        .find(|v| v["type"] == "cycle_end")
        .expect("cycle_end event");
    assert_eq!(cycle_end["cycle_id"], 1);
    assert_eq!(cycle_end["source"], "initial");
    assert_eq!(cycle_end["reason"], "completed");
    assert_eq!(cycle_end["exit_code"], 0);
    assert_eq!(cycle_end["turns"], 1);
    assert_eq!(cycle_end["cycle_usage"]["input_tokens"], 100);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn external_approval_decider_trust_gate_and_exit_75() {
    let root = make_temp_env("approval-decider");
    let home = setup_fhs_home(&root);
    let work = root.join("proj");
    let proj_agent = work.join(".anix");
    fs::create_dir_all(&proj_agent).unwrap();

    let state_file = root.join("prov_turn");
    let mock_prov = root.join("mock-provider.sh");
    write_exec(
        &mock_prov,
        &format!(
            r#"#!/usr/bin/env sh
STATE="{}"
while IFS= read -r line; do
  [ -z "$line" ] && continue
  n=0
  [ -f "$STATE" ] && n=$(cat "$STATE")
  n=$((n + 1))
  echo "$n" > "$STATE"
  if [ "$n" -eq 1 ]; then
    cat <<'EOF'
{{"type":"tool_call","id":"c1","name":"msh","arguments":{{"cmd":"touch allowed.txt"}}}}
{{"type":"done","finish_reason":"tool_calls"}}
EOF
  elif [ "$n" -eq 2 ]; then
    cat <<'EOF'
{{"type":"tool_call","id":"c2","name":"msh","arguments":{{"cmd":"touch blocked.txt"}}}}
{{"type":"done","finish_reason":"tool_calls"}}
EOF
  else
    cat <<'EOF'
{{"type":"text_delta","text":"done"}}
{{"type":"done","finish_reason":"stop"}}
EOF
  fi
done
"#,
            state_file.display()
        ),
    );

    let decider = proj_agent.join("decider.sh");
    write_exec(
        &decider,
        r#"#!/usr/bin/env sh
input=$(cat)
case "$input" in
  *allowed.txt*) exit 0 ;;
  *blocked.txt*) echo "blocked by policy"; exit 1 ;;
  *crash.txt*) echo "decider crash" >&2; exit 9 ;;
  *) exit 2 ;;
esac
"#,
    );

    let cfg_dir = home.join(".config/anix");
    fs::create_dir_all(&cfg_dir).unwrap();
    fs::write(
        cfg_dir.join("config.toml"),
        format!(
            "[defaults]\nprovider = \"mock\"\nmodel = \"m1\"\n\n[providers.mock]\nbin = \"{}\"\nmode = \"coprocess\"\napi_key_env = \"\"\n",
            mock_prov.display()
        ),
    )
    .unwrap();

    fs::write(
        proj_agent.join("config.toml"),
        "[approval]\nbin = \".anix/decider.sh\"\ntimeout_sec = 10\n",
    )
    .unwrap();

    let script = work.join("a.agent");
    fs::write(
        &script,
        "---\nname: app-demo\napproval: ask\n---\nRun commands.\n",
    )
    .unwrap();

    // 1. Untrusted project `[approval].bin` MUST fail-closed with exit code 75 (`EX_TEMPFAIL`).
    let mut cmd_untrusted = base_cmd(&home, &work);
    cmd_untrusted
        .arg(&script)
        .arg("go")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_child_fds(&mut cmd_untrusted, None);
    let out_untrusted = cmd_untrusted.output().expect("run untrusted decider");
    assert_eq!(
        out_untrusted.status.code(),
        Some(75),
        "expected exit 75 (EX_TEMPFAIL) for untrusted [approval].bin, stderr={}",
        String::from_utf8_lossy(&out_untrusted.stderr)
    );

    // 2. Trust the project directory via `anix trust .`.
    let trust_out = base_cmd(&home, &work)
        .args(["trust", "."])
        .output()
        .expect("run anix trust");
    assert_eq!(trust_out.status.code(), Some(0));

    // 3. Re-run with `--trace-fd=5`: turn 1 allowed (exit 0), turn 2 denied to model (exit 1), turn 3 finishes!
    let _ = fs::remove_file(&state_file);
    let trace_path = root.join("approval_trace.jsonl");
    let trace_file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&trace_path)
        .unwrap();
    let mut cmd_trusted = base_cmd(&home, &work);
    cmd_trusted
        .args(["--trace-fd", "5"])
        .arg(&script)
        .arg("go")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_child_fds(&mut cmd_trusted, Some((&trace_file, 5)));
    let out_trusted = cmd_trusted.output().expect("run trusted decider");
    drop(trace_file);
    assert_eq!(
        out_trusted.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&out_trusted.stderr)
    );
    assert!(work.join("allowed.txt").is_file());
    assert!(!work.join("blocked.txt").exists());

    let events: Vec<Value> = fs::read_to_string(&trace_path)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let approvals: Vec<&Value> = events
        .iter()
        .filter(|v| v["type"] == "tool_approval")
        .collect();
    assert_eq!(approvals.len(), 2);
    assert_eq!(approvals[0]["decider"], "external");
    assert_eq!(approvals[0]["decider_exit_code"], 0);
    assert_eq!(approvals[0]["decision"], "allow");
    assert_eq!(approvals[1]["decider"], "external");
    assert_eq!(approvals[1]["decider_exit_code"], 1);
    assert_eq!(approvals[1]["decision"], "deny");

    // 4. `--approval deny --fail-on-deny` also exits 75 (`EX_TEMPFAIL`).
    let _ = fs::remove_file(&state_file);
    let mut cmd_deny = base_cmd(&home, &work);
    cmd_deny
        .args(["--approval", "deny", "--fail-on-deny"])
        .arg(&script)
        .arg("go")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_child_fds(&mut cmd_deny, None);
    let out_deny = cmd_deny.output().expect("run fail-on-deny");
    assert_eq!(out_deny.status.code(), Some(75));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn vision_false_blocks_wal_pollution_and_vision_bridge_translates_and_mcp_tip_present() {
    let root = make_temp_env("vision");
    let home = setup_fhs_home(&root);
    let work = root.join("work");
    fs::create_dir_all(&work).unwrap();

    // Helper script invoked inside `msh` to emit a clean media control frame on `$ANIX_CTL`.
    let emit_media = root.join("emit-media.sh");
    write_exec(
        &emit_media,
        r#"#!/usr/bin/env sh
printf '%s\n' '{"op":"media","mime":"image/png","path":"/tmp/fake.png","source":"demo.png"}' > "$ANIX_CTL"
"#,
    );

    let state_file = root.join("turn_state");
    let mock_prov = root.join("mock-provider.sh");
    write_exec(
        &mock_prov,
        &format!(
            r#"#!/usr/bin/env sh
STATE="{}"
EMIT="{}"
while IFS= read -r line; do
  [ -z "$line" ] && continue
  n=0
  [ -f "$STATE" ] && n=$(cat "$STATE")
  n=$((n + 1))
  echo "$n" > "$STATE"
  if [ "$n" -eq 1 ]; then
    printf '%s\n' "{{\"type\":\"tool_call\",\"id\":\"c1\",\"name\":\"msh\",\"arguments\":{{\"cmd\":\"$EMIT\"}}}}"
    printf '%s\n' '{{"type":"done","finish_reason":"tool_calls"}}'
  else
    printf '%s\n' '{{"type":"text_delta","text":"saw result"}}'
    printf '%s\n' '{{"type":"done","finish_reason":"stop"}}'
  fi
done
"#,
            state_file.display(),
            emit_media.display()
        ),
    );

    let cfg_dir = home.join(".config/anix");
    fs::create_dir_all(&cfg_dir).unwrap();
    fs::write(
        cfg_dir.join("config.toml"),
        format!(
            "[defaults]\nprovider = \"mock\"\nmodel = \"text-only\"\n\n[providers.mock]\nbin = \"{}\"\nmode = \"coprocess\"\napi_key_env = \"\"\n",
            mock_prov.display()
        ),
    )
    .unwrap();

    let script = work.join("v.agent");
    fs::write(
        &script,
        "---\nname: vision-demo\napproval: auto\n---\nInspect.\n",
    )
    .unwrap();

    // 1. `--no-vision` WITHOUT `[vision].bin`: blocks media frame, never writes media[] to WAL,
    //    injects non-vision contract + `how-to-mcp.md` in `<runtime_tips>`.
    let wal_path = root.join("novision.wal.jsonl");
    let mut cmd1 = base_cmd(&home, &work);
    cmd1.args(["--no-vision", "-c"])
        .arg(&wal_path)
        .arg(&script)
        .arg("inspect")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_child_fds(&mut cmd1, None);
    let out1 = cmd1.output().expect("run --no-vision");
    assert_eq!(out1.status.code(), Some(0));

    let wal_text = fs::read_to_string(&wal_path).unwrap();
    assert!(
        !wal_text.contains("\"media\":[{"),
        "media[] MUST NOT be persisted to WAL when vision=false: {wal_text}"
    );
    assert!(
        wal_text.contains("does not support native vision (vision=false)"),
        "expected deterministic vision error in tool turn: {wal_text}"
    );
    assert!(
        wal_text.contains("how-to-mcp.md"),
        "expected built-in how-to-mcp.md in <runtime_tips>: {wal_text}"
    );
    assert!(
        wal_text.contains("how-to-runtime.md"),
        "expected built-in how-to-runtime.md in <runtime_tips>: {wal_text}"
    );

    // 2. `--no-vision` WITH `[vision].bin`: translates image to `<vision_description>` text, zero media[].
    let bridge = root.join("vision-bridge.sh");
    write_exec(
        &bridge,
        "#!/usr/bin/env sh\ncat >/dev/null\necho 'A red error dialog showing HTTP 500'\n",
    );
    fs::write(
        cfg_dir.join("config.toml"),
        format!(
            "[defaults]\nprovider = \"mock\"\nmodel = \"text-only\"\n\n[vision]\nbin = \"{}\"\ntimeout_sec = 10\n\n[providers.mock]\nbin = \"{}\"\nmode = \"coprocess\"\napi_key_env = \"\"\n",
            bridge.display(),
            mock_prov.display()
        ),
    )
    .unwrap();

    let _ = fs::remove_file(&state_file);
    let wal_bridge = root.join("bridge.wal.jsonl");
    let mut cmd2 = base_cmd(&home, &work);
    cmd2.args(["--no-vision", "-c"])
        .arg(&wal_bridge)
        .arg(&script)
        .arg("inspect")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_child_fds(&mut cmd2, None);
    let out2 = cmd2.output().expect("run --no-vision with bridge");
    assert_eq!(out2.status.code(), Some(0));

    let wal2 = fs::read_to_string(&wal_bridge).unwrap();
    assert!(!wal2.contains("\"media\":[{"), "bridge must strip media[]");
    assert!(
        wal2.contains(
            "<vision_description source=\\\"demo.png\\\">A red error dialog showing HTTP 500</vision_description>"
        ),
        "expected vision_description in WAL: {wal2}"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn aclr_auto_summary_tail_fold_and_coprocess_inbox_lifecycle() {
    let root = make_temp_env("autofold-inbox");
    let home = setup_fhs_home(&root);
    let work = root.join("work");
    fs::create_dir_all(&work).unwrap();

    let emit_raw = root.join("emit-raw.sh");
    write_exec(
        &emit_raw,
        "#!/usr/bin/env sh\nprintf 'RAW-NO-NL' > \"$ANIX_OUT\"\n",
    );

    let pid_log = root.join("pids.log");
    let req_log = root.join("reqs.jsonl");
    let state_file = root.join("step");
    let mock_prov = root.join("mock-coproc.sh");
    write_exec(
        &mock_prov,
        &format!(
            r#"#!/usr/bin/env sh
PIDLOG="{}"
REQLOG="{}"
STATE="{}"
EMIT="{}"
while IFS= read -r line; do
  [ -z "$line" ] && continue
  printf '%s\n' "$$" >> "$PIDLOG"
  printf '%s\n' "$line" >> "$REQLOG"
  n=0
  [ -f "$STATE" ] && n=$(cat "$STATE")
  n=$((n + 1))
  echo "$n" > "$STATE"
  if [ "$n" -eq 1 ]; then
    cat <<'EOF'
{{"type":"tool_call","id":"c1","name":"msh","arguments":{{"cmd":"aclr -t 2"}}}}
{{"type":"usage","input_tokens":500,"output_tokens":50}}
{{"type":"done","finish_reason":"tool_calls"}}
EOF
  elif [ "$n" -eq 2 ]; then
    cat <<'EOF'
{{"type":"text_delta","text":"Auto folded dead-end attempt"}}
{{"type":"usage","input_tokens":60,"output_tokens":15}}
{{"type":"done","finish_reason":"stop"}}
EOF
  elif [ "$n" -eq 3 ]; then
    printf '%s\n' "{{\"type\":\"tool_call\",\"id\":\"c2\",\"name\":\"msh\",\"arguments\":{{\"cmd\":\"$EMIT\"}}}}"
    printf '%s\n' '{{"type":"usage","input_tokens":300,"output_tokens":30}}'
    printf '%s\n' '{{"type":"done","finish_reason":"tool_calls"}}'
  elif [ "$n" -eq 4 ]; then
    cat <<'EOF'
{{"type":"text_delta","text":"msg1-done"}}
{{"type":"usage","input_tokens":320,"output_tokens":20}}
{{"type":"done","finish_reason":"stop"}}
EOF
  else
    cat <<'EOF'
{{"type":"text_delta","text":"msg2-done"}}
{{"type":"usage","input_tokens":350,"output_tokens":25}}
{{"type":"done","finish_reason":"stop"}}
EOF
  fi
done
"#,
            pid_log.display(),
            req_log.display(),
            state_file.display(),
            emit_raw.display()
        ),
    );

    let cfg_dir = home.join(".config/anix");
    fs::create_dir_all(&cfg_dir).unwrap();
    fs::write(
        cfg_dir.join("config.toml"),
        format!(
            "[defaults]\nprovider = \"mock\"\nmodel = \"m1\"\n\n[providers.mock]\nbin = \"{}\"\nmode = \"coprocess\"\napi_key_env = \"\"\n",
            mock_prov.display()
        ),
    )
    .unwrap();

    let script = work.join("worker.agent");
    fs::write(
        &script,
        "---\nname: inbox-worker\napproval: auto\noutput: explicit\n---\nWork.\n",
    )
    .unwrap();

    let inbox_dir = root.join("inbox");
    let wal_path = root.join("inbox.wal.jsonl");
    let trace_path = root.join("inbox.trace.jsonl");
    let trace_file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&trace_path)
        .unwrap();

    fs::create_dir_all(inbox_dir.join("tmp")).unwrap();
    fs::create_dir_all(inbox_dir.join("new")).unwrap();
    fs::write(inbox_dir.join("new/001-first"), "first task").unwrap();

    let inbox_for_thread = inbox_dir.clone();
    let sender = thread::spawn(move || {
        thread::sleep(Duration::from_millis(250));
        let tmp_msg = inbox_for_thread.join("tmp/002-second");
        let mut f = File::create(&tmp_msg).unwrap();
        f.write_all(b"second task").unwrap();
        drop(f);
        fs::rename(&tmp_msg, inbox_for_thread.join("new/002-second")).unwrap();
    });

    let mut cmd = base_cmd(&home, &work);
    cmd.args(["--inbox"])
        .arg(&inbox_dir)
        .args(["--idle-timeout", "1", "--trace-fd", "6", "-c"])
        .arg(&wal_path)
        .arg(&script)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_child_fds(&mut cmd, Some((&trace_file, 6)));

    let out = cmd.output().expect("run inbox agent");
    sender.join().unwrap();
    drop(trace_file);

    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    // 1. Verify raw `fd 1` payload is completely untouched (no trailing `\n` injected!).
    assert_eq!(String::from_utf8_lossy(&out.stdout), "RAW-NO-NL");

    // 2. Verify ALL 5 provider requests ran inside ONE persistent coprocess PID!
    let pids: Vec<String> = fs::read_to_string(&pid_log)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect();
    assert_eq!(pids.len(), 5, "expected 5 provider requests, got {pids:?}");
    assert!(
        pids.iter().all(|p| p == &pids[0]),
        "expected all requests to share one persistent coprocess PID, got {pids:?}"
    );

    // 3. Verify req 2 (auto-fold summary) had `tools: []` and req 3 received `<folded ...>Auto folded dead-end attempt</folded>`.
    let reqs: Vec<Value> = fs::read_to_string(&req_log)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(reqs[1]["header"]["tools"].as_array().unwrap().len(), 0);
    let req3_str = reqs[2].to_string();
    assert!(
        req3_str.contains("Auto folded dead-end attempt"),
        "expected auto-folded summary in subsequent request: {req3_str}"
    );

    // 4. Verify both inbox messages moved to `done/`.
    assert!(inbox_dir.join("done/001-first").is_file());
    assert!(inbox_dir.join("done/002-second").is_file());

    // 5. Verify WAL contains `fold` event with `fold_usage`.
    let wal_text = fs::read_to_string(&wal_path).unwrap();
    assert!(wal_text.contains("\"type\":\"fold\""));
    assert!(wal_text.contains("\"fold_usage\""));

    // 6. Verify Trace contains nested `cycle_start` / `cycle_end` for both messages + `fold` + `idle` + `run_end`.
    let events: Vec<Value> = fs::read_to_string(&trace_path)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let fold_ev = events
        .iter()
        .find(|v| v["type"] == "fold")
        .expect("fold event");
    assert_eq!(fold_ev["fold_usage"]["input_tokens"], 60);

    let cycle_ends: Vec<&Value> = events.iter().filter(|v| v["type"] == "cycle_end").collect();
    assert_eq!(cycle_ends.len(), 2, "events={events:?}");
    assert_eq!(cycle_ends[0]["source"], "001-first");
    assert_eq!(cycle_ends[0]["reason"], "completed");
    assert_eq!(cycle_ends[1]["source"], "002-second");
    assert_eq!(cycle_ends[1]["reason"], "completed");
    assert!(
        events
            .iter()
            .any(|v| v["type"] == "idle" && v["idle_timeout_sec"] == 1),
        "expected idle event in inbox trace"
    );

    // 7. Verify `fd 2` newline contract on `--inbox` per-cycle `[usage]`.
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("msg1-done\n[usage] ") && stderr.contains("msg2-done\n[usage] "),
        "expected clean newline before each cycle [usage], got: {stderr:?}"
    );
    assert!(stderr.ends_with('\n'));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_v1_1_1_session_id_persists_across_wal_resume_checkpoint_and_isolates_msh() {
    let root = make_temp_env("session-id");
    let home = setup_fhs_home(&root);
    let workdir = root.join("work");
    fs::create_dir_all(&workdir).unwrap();

    let sid_log = root.join("provider_sids.jsonl");
    let state_file = root.join("turn_counter.txt");
    let wal_path = root.join("session.ctx.jsonl");
    let trace1_path = root.join("trace1.jsonl");
    let trace2_path = root.join("trace2.jsonl");

    // Persistent coprocess mock provider:
    // - Records ANIX_SESSION_ID env and each request's `envelope.session_id`
    // - Turn 1: calls `msh` to check that `ANIX_SESSION_ID` is unset inside msh AND triggers `aclr -s`
    // - Turn 2 (built-in compactor, `tools: []`): returns compact summary
    // - Turn 3: finishes Run 1
    // - Turn 4 (Run 2 after WAL resume): finishes Run 2
    let mock_provider = root.join("mock-session-provider");
    write_exec(
        &mock_provider,
        &format!(
            r#"#!/usr/bin/env python3
import json, os, sys

sid_log = {sid_log:?}
state_file = {state_file:?}
env_sid = os.environ.get("ANIX_SESSION_ID", "")

def next_count():
    n = int(open(state_file).read().strip()) if os.path.exists(state_file) else 0
    n += 1
    open(state_file, "w").write(str(n))
    return n

for raw in sys.stdin:
    line = raw.strip()
    if not line:
        continue
    req = json.loads(line)
    n = next_count()
    with open(sid_log, "a") as f:
        f.write(json.dumps({{
            "n": n,
            "env_sid": env_sid,
            "req_sid": req.get("session_id"),
            "tools_len": len(req["header"]["tools"]),
        }}) + "\n")
    if n == 1:
        # Verify ANIX_SESSION_ID is stripped from msh env and trigger full compaction via `aclr -s`.
        cmd = 'printf "msh_sid=%s\\n" "${{ANIX_SESSION_ID:-UNSET}}" > "$ANIX_OUT"; printf \'{{"op":"compact"}}\\n\' > "$ANIX_CTL"'
        print(json.dumps({{"type": "tool_call", "id": "call_s1", "name": "msh", "arguments": {{"cmd": cmd}}}}), flush=True)
        print(json.dumps({{"type": "usage", "input_tokens": 50, "output_tokens": 10}}), flush=True)
        print(json.dumps({{"type": "done", "finish_reason": "tool_calls"}}), flush=True)
    elif n == 2:
        print(json.dumps({{"type": "text_delta", "text": "Compacted checkpoint summary"}}), flush=True)
        print(json.dumps({{"type": "usage", "input_tokens": 80, "output_tokens": 15}}), flush=True)
        print(json.dumps({{"type": "done", "finish_reason": "stop"}}), flush=True)
    else:
        print(json.dumps({{"type": "text_delta", "text": f"reply-{{n}}"}}), flush=True)
        print(json.dumps({{"type": "usage", "input_tokens": 40, "output_tokens": 8}}), flush=True)
        print(json.dumps({{"type": "done", "finish_reason": "stop"}}), flush=True)
"#,
            sid_log = sid_log.display().to_string(),
            state_file = state_file.display().to_string(),
        ),
    );

    let global_cfg_dir = home.join(".config/anix");
    fs::create_dir_all(&global_cfg_dir).unwrap();
    fs::write(
        global_cfg_dir.join("config.toml"),
        format!(
            r#"[defaults]
provider = "mock"
model = "mock-model"
approval = "auto"

[providers.mock]
bin = "{}"
mode = "coprocess"
base_url = "http://127.0.0.1:1/v1"
api_key_env = ""
timeout_sec = 30
"#,
            mock_provider.display()
        ),
    )
    .unwrap();

    let script = workdir.join("sess.agent");
    write_exec(
        &script,
        "#!/usr/bin/env anix\n---\nname: sess-test\noutput: explicit\n---\nSession test prompt\n",
    );

    // Run 1: fresh WAL -> tool call (aclr -s) -> built-in compaction (writes checkpoint) -> final turn.
    let tf1 = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&trace1_path)
        .unwrap();
    let mut cmd1 = base_cmd(&home, &workdir);
    cmd1.args([
        "-c",
        &wal_path.display().to_string(),
        "--trace-fd=7",
        &script.display().to_string(),
        "turn 1",
    ])
    .env("ANIX_TRACE_LEVEL", "events")
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
    configure_child_fds(&mut cmd1, Some((&tf1, 7)));
    let out1 = cmd1.output().expect("run 1");
    drop(tf1);
    assert_eq!(
        out1.status.code(),
        Some(0),
        "run1 stderr={}",
        String::from_utf8_lossy(&out1.stderr)
    );
    // msh must NOT inherit ANIX_SESSION_ID.
    assert_eq!(String::from_utf8_lossy(&out1.stdout), "msh_sid=UNSET\n");

    // Run 2: resume the SAME WAL after checkpoint in a brand-new `anix` process.
    let tf2 = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&trace2_path)
        .unwrap();
    let mut cmd2 = base_cmd(&home, &workdir);
    cmd2.args([
        "-c",
        &wal_path.display().to_string(),
        "--trace-fd=7",
        &script.display().to_string(),
        "turn 2",
    ])
    .env("ANIX_TRACE_LEVEL", "events")
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
    configure_child_fds(&mut cmd2, Some((&tf2, 7)));
    let out2 = cmd2.output().expect("run 2");
    drop(tf2);
    assert_eq!(
        out2.status.code(),
        Some(0),
        "run2 stderr={}",
        String::from_utf8_lossy(&out2.stderr)
    );

    // 1. Verify Trace `run_start` carries the exact same non-empty `session_id` in both runs.
    let t1: Vec<Value> = fs::read_to_string(&trace1_path)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let t2: Vec<Value> = fs::read_to_string(&trace2_path)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let sid1 = t1
        .iter()
        .find(|v| v["type"] == "run_start")
        .and_then(|v| v["session_id"].as_str())
        .expect("run1 run_start.session_id")
        .to_owned();
    let sid2 = t2
        .iter()
        .find(|v| v["type"] == "run_start")
        .and_then(|v| v["session_id"].as_str())
        .expect("run2 run_start.session_id")
        .to_owned();
    assert!(!sid1.is_empty());
    assert_eq!(
        sid1, sid2,
        "WAL resume must preserve session_id across checkpoint"
    );

    // 2. Verify WAL `session_init` AND `checkpoint` both persisted `session_id == sid1`.
    let wal_entries: Vec<Value> = fs::read_to_string(&wal_path)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let init_sid = wal_entries
        .iter()
        .find(|v| v["type"] == "session_init")
        .and_then(|v| v["session_id"].as_str());
    let ckpt_sid = wal_entries
        .iter()
        .find(|v| v["type"] == "checkpoint")
        .and_then(|v| v["session_id"].as_str());
    assert_eq!(init_sid, Some(sid1.as_str()));
    assert_eq!(ckpt_sid, Some(sid1.as_str()));

    // 3. Verify ALL 4 provider calls (turn 1, built-in compactor, post-compact turn 3, resumed run 2)
    //    received the exact same `session_id` via BOTH `ANIX_SESSION_ID` env and coprocess envelope!
    let records: Vec<Value> = fs::read_to_string(&sid_log)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(records.len(), 4, "records={records:?}");
    assert_eq!(
        records[1]["tools_len"], 0,
        "req 2 must be built-in compactor"
    );
    for rec in &records {
        assert_eq!(rec["env_sid"].as_str(), Some(sid1.as_str()), "rec={rec:?}");
        assert_eq!(rec["req_sid"].as_str(), Some(sid1.as_str()), "rec={rec:?}");
    }

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_v1_1_2_msh_per_call_timeout_yield_bg_notification_and_kill_on_timeout() {
    let root = make_temp_env("msh-bg-v112");
    let home = setup_fhs_home(&root);
    let work = root.join("work");
    fs::create_dir_all(&work).unwrap();

    let req_log = root.join("reqs.jsonl");
    let trace_path = root.join("trace.jsonl");
    let wal_path = root.join("bg.ctx.jsonl");
    let provider_path = root.join("mock-provider-bg.sh");

    // 3-turn coprocess mock provider exercising:
    // Turn 1: `timeout_sec: 1` (default kill_on_timeout=false) on a 2s command -> auto-yields to background!
    // Turn 2: 3 calls in one batch:
    //   - `timeout_sec: 0` -> immediate background start (`IMM-BG-DONE`)
    //   - `timeout_sec: 1, kill_on_timeout: true` on `sleep 20` -> hard-killed, exit=124, is_error=true
    //   - `timeout_sec: 999` (clamped to max_msh_timeout_sec=5) on `sleep 2; echo EXTENDED-OK` -> completes!
    // Turn 3: receives `<runtime_update>` notifications for BOTH completed bg jobs (exit_code=0 + log tails!)
    //   and returns final text.
    write_exec(
        &provider_path,
        &format!(
            r#"#!/usr/bin/env sh
count=0
while IFS= read -r line; do
  [ -n "$line" ] || continue
  printf '%s\n' "$line" >> "{req_log}"
  count=$((count + 1))
  if [ "$count" -eq 1 ]; then
    printf '%s\n' '{{"type":"tool_call","id":"call_yield","name":"msh","arguments":{{"cmd":"echo BG-START; sleep 2; echo BG-FINISHED","timeout_sec":1}}}}'
    printf '%s\n' '{{"type":"usage","input_tokens":10,"output_tokens":5}}'
    printf '%s\n' '{{"type":"done","finish_reason":"tool_calls"}}'
  elif [ "$count" -eq 2 ]; then
    printf '%s\n' '{{"type":"tool_call","id":"call_imm","name":"msh","arguments":{{"cmd":"sleep 1; echo IMM-BG-DONE","timeout_sec":0}}}}'
    printf '%s\n' '{{"type":"tool_call","id":"call_kill","name":"msh","arguments":{{"cmd":"sleep 20","timeout_sec":1,"kill_on_timeout":true}}}}'
    printf '%s\n' '{{"type":"tool_call","id":"call_ext","name":"msh","arguments":{{"cmd":"sleep 2; echo EXTENDED-OK","timeout_sec":999}}}}'
    printf '%s\n' '{{"type":"usage","input_tokens":20,"output_tokens":10}}'
    printf '%s\n' '{{"type":"done","finish_reason":"tool_calls"}}'
  else
    printf '%s\n' '{{"type":"text_delta","text":"bg-v112-ok"}}'
    printf '%s\n' '{{"type":"usage","input_tokens":30,"output_tokens":5}}'
    printf '%s\n' '{{"type":"done","finish_reason":"stop"}}'
  fi
done
"#,
            req_log = req_log.display(),
        ),
    );

    let cfg_dir = home.join(".config/anix");
    fs::create_dir_all(&cfg_dir).unwrap();
    fs::write(
        cfg_dir.join("config.toml"),
        format!(
            "[defaults]\nprovider = \"mock\"\nmodel = \"mock-m\"\napproval = \"auto\"\nstderr = \"auto\"\nmsh_timeout_sec = 2\nmax_msh_timeout_sec = 5\n\n[providers.mock]\nbin = \"{}\"\nmode = \"coprocess\"\nbase_url = \"http://127.0.0.1:1\"\napi_key_env = \"\"\n",
            provider_path.display()
        ),
    )
    .unwrap();

    let script = work.join("bg.agent");
    fs::write(&script, "---\nname: bg-v112\n---\nTest msh v1.1.2\n").unwrap();

    let trace_file = File::create(&trace_path).unwrap();
    let mut cmd = base_cmd(&home, &work);
    cmd.env_remove("MSH_TIMEOUT_SEC")
        .env_remove("MSH_MAX_TIMEOUT_SEC")
        .env_remove("MSH_KILL_ON_TIMEOUT")
        .arg("--trace-fd=3")
        .arg("-c")
        .arg(&wal_path)
        .arg(&script)
        .arg("--")
        .arg("run bg test");
    configure_child_fds(&mut cmd, Some((&trace_file, 3)));

    let out = cmd.output().expect("run anix v1.1.2 bg test");
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let reqs: Vec<Value> = fs::read_to_string(&req_log)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(
        reqs.len(),
        3,
        "expected 3 provider turns, got {}",
        reqs.len()
    );

    // 1. Verify Turn 1 tool definition exposes `timeout_sec` & `kill_on_timeout`.
    let msh_props = &reqs[0]["header"]["tools"][0]["parameters"]["properties"];
    assert!(msh_props.get("timeout_sec").is_some());
    assert!(msh_props.get("kill_on_timeout").is_some());

    // 2. Verify Turn 2 received the yielded job handle (`exit=0`, `BG-START`, moved to background, NOT error).
    let req2_msgs = reqs[1]["messages"].as_array().unwrap();
    let yield_tool = req2_msgs
        .iter()
        .find(|m| m["tool_call_id"] == "call_yield")
        .expect("call_yield tool msg");
    assert_ne!(yield_tool["is_error"], Value::Bool(true));
    let yield_text = yield_tool["text"].as_str().unwrap();
    assert!(yield_text.contains("exit=0"), "yield_text={yield_text}");
    assert!(yield_text.contains("BG-START"), "yield_text={yield_text}");
    assert!(
        yield_text.contains("moved to background"),
        "yield_text={yield_text}"
    );

    // 3. Verify Turn 3 received:
    //    - `call_imm` started in background (`exit=0`, `started in background`)
    //    - `call_kill` killed on timeout (`exit=124`, `is_error=true`)
    //    - `call_ext` succeeded with `EXTENDED-OK` (clamped timeout 5s > default 2s!)
    //    - `<runtime_update>` notifications for BOTH background jobs with `exit_code=0` and log tails!
    let req3_msgs = reqs[2]["messages"].as_array().unwrap();
    let imm_tool = req3_msgs
        .iter()
        .find(|m| m["tool_call_id"] == "call_imm")
        .expect("call_imm");
    assert_ne!(imm_tool["is_error"], Value::Bool(true));
    assert!(
        imm_tool["text"]
            .as_str()
            .unwrap()
            .contains("started in background")
    );

    let kill_tool = req3_msgs
        .iter()
        .find(|m| m["tool_call_id"] == "call_kill")
        .expect("call_kill");
    assert_eq!(kill_tool["is_error"], Value::Bool(true));
    let kill_text = kill_tool["text"].as_str().unwrap();
    assert!(kill_text.contains("exit=124"), "kill_text={kill_text}");
    assert!(
        kill_text.contains("process group terminated"),
        "kill_text={kill_text}"
    );

    let ext_tool = req3_msgs
        .iter()
        .find(|m| m["tool_call_id"] == "call_ext")
        .expect("call_ext");
    assert_ne!(ext_tool["is_error"], Value::Bool(true));
    assert!(ext_tool["text"].as_str().unwrap().contains("EXTENDED-OK"));

    let runtime_updates: Vec<&str> = req3_msgs
        .iter()
        .filter(|m| m["role"] == "system")
        .filter_map(|m| m["text"].as_str())
        .filter(|t| t.contains("<runtime_update>"))
        .collect();
    let joined_updates = runtime_updates.join("\n");
    assert!(
        joined_updates.contains("BG-FINISHED") && joined_updates.contains("exit_code=0"),
        "expected BG-FINISHED runtime_update, got: {joined_updates}"
    );
    assert!(
        joined_updates.contains("IMM-BG-DONE"),
        "expected IMM-BG-DONE runtime_update, got: {joined_updates}"
    );

    // 4. Verify Trace events: clamping (`999 -> 5`), `bg_job_started` x2, `bg_job_finished` x2.
    let events: Vec<Value> = fs::read_to_string(&trace_path)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let ext_start = events
        .iter()
        .find(|e| e["type"] == "tool_start" && e["tool_call_id"] == "call_ext")
        .expect("call_ext tool_start");
    assert_eq!(
        ext_start["timeout_sec"], 5,
        "requested 999s must be clamped to max_msh_timeout_sec=5"
    );

    let bg_started: Vec<&Value> = events
        .iter()
        .filter(|e| e["type"] == "bg_job_started")
        .collect();
    let bg_finished: Vec<&Value> = events
        .iter()
        .filter(|e| e["type"] == "bg_job_finished")
        .collect();
    assert_eq!(bg_started.len(), 2, "bg_started={bg_started:?}");
    assert_eq!(bg_finished.len(), 2, "bg_finished={bg_finished:?}");
    assert!(bg_finished.iter().all(|e| e["exit_code"] == 0));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_v1_2_0_orthogonal_output_contracts_and_shell_none() {
    let root = make_temp_env("v120-output");
    let home = setup_fhs_home(&root);
    let work = root.join("work");
    fs::create_dir_all(&work).unwrap();

    // -------------------------------------------------------------------------
    // Case 1: `shell: none` + `output: reply` (pure-text filter + real 2-stage pipeline)
    // -------------------------------------------------------------------------
    let req_log_pure = root.join("reqs_pure.jsonl");
    let mock_pure = root.join("mock-pure.sh");
    write_exec(
        &mock_pure,
        &format!(
            r#"#!/usr/bin/env python3
import json, sys
for raw in sys.stdin:
    line = raw.strip()
    if not line:
        continue
    with open({req_log:?}, "a") as f:
        f.write(line + "\n")
    req = json.loads(line)
    last_user = ""
    for m in req["messages"]:
        if m["role"] == "user":
            last_user = m["text"]
    out = "EN:" + last_user.strip()
    print(json.dumps({{"type": "text_delta", "text": out}}), flush=True)
    print(json.dumps({{"type": "usage", "input_tokens": 12, "output_tokens": 6}}), flush=True)
    print(json.dumps({{"type": "done", "finish_reason": "stop"}}), flush=True)
"#,
            req_log = req_log_pure.display().to_string()
        ),
    );

    let cfg_dir = home.join(".config/anix");
    fs::create_dir_all(&cfg_dir).unwrap();
    fs::write(
        cfg_dir.join("config.toml"),
        format!(
            "[defaults]\nprovider = \"pure\"\nmodel = \"m1\"\n\n[providers.pure]\nbin = \"{}\"\nmode = \"coprocess\"\napi_key_env = \"\"\n",
            mock_pure.display()
        ),
    )
    .unwrap();

    let pure_script = work.join("to_en.agent");
    write_exec(
        &pure_script,
        "#!/usr/bin/env anix\n---\nname: to-en\nshell: none\noutput: reply\napproval: deny\n---\nTranslate input to English.\n",
    );

    let trace_pure = root.join("trace_pure.jsonl");
    let tf_pure = File::create(&trace_pure).unwrap();
    let mut cmd_pure = base_cmd(&home, &work);
    cmd_pure
        .args(["--trace-fd=3"])
        .arg(&pure_script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_child_fds(&mut cmd_pure, Some((&tf_pure, 3)));
    let mut child_pure = cmd_pure.spawn().expect("spawn pure filter");
    child_pure
        .stdin
        .take()
        .unwrap()
        .write_all(b"Ciallo")
        .unwrap();
    let out_pure = child_pure.wait_with_output().expect("wait pure filter");
    drop(tf_pure);
    assert_eq!(
        out_pure.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&out_pure.stderr)
    );
    // Non-TTY stdout must emit exact bytes without appending `\n`.
    assert_eq!(String::from_utf8_lossy(&out_pure.stdout), "EN:Ciallo");
    let pure_stderr = String::from_utf8_lossy(&out_pure.stderr);
    // Final reply must NOT be echoed to fd 2, and `approval: deny` with `shell: none` must emit no warning.
    assert!(
        !pure_stderr.contains("EN:Ciallo"),
        "stdout reply must not double-print to stderr: {pure_stderr:?}"
    );
    assert!(
        !pure_stderr.contains("warning:"),
        "shell=none + approval=deny should emit no warning: {pure_stderr:?}"
    );

    // Inspect IR sent to provider: `tools: []`, `output_mode: "reply"`, no `<runtime_tips>`/`<available_skills>`.
    let pure_req: Value = serde_json::from_str(
        fs::read_to_string(&req_log_pure)
            .unwrap()
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(pure_req["header"]["tools"].as_array().unwrap().len(), 0);
    assert_eq!(pure_req["header"]["meta"]["output_mode"], "reply");
    let sys_text = pure_req["messages"][0]["text"].as_str().unwrap();
    assert!(sys_text.contains("shell=none"), "sys={sys_text}");
    assert!(!sys_text.contains("<runtime_tips>"), "sys={sys_text}");
    assert!(!sys_text.contains("<available_skills>"), "sys={sys_text}");

    let pure_trace: Vec<Value> = fs::read_to_string(&trace_pure)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let rs = pure_trace
        .iter()
        .find(|e| e["type"] == "run_start")
        .unwrap();
    assert_eq!(rs["shell"], "none");
    assert_eq!(rs["output"], "reply");

    // -------------------------------------------------------------------------
    // Case 2: `shell: msh` + `output: reply` (default):
    //   - Turn 1 intermediate text goes to fd 2 ONLY (never fd 1)
    //   - Inside `msh`, `$ANIX_OUT` is UNSET and `fd 4` is CLOSED
    //   - Turn 2 final reply goes to fd 1 ONLY (exact bytes, no fd 2 echo)
    //   - Prompt mentioning `ANIX_OUT` emits non-blocking migration warning
    // -------------------------------------------------------------------------
    let probe_file = work.join("msh_probe.txt");
    let state_hybrid = root.join("hybrid_turn");
    let mock_hybrid = root.join("mock-hybrid.sh");
    write_exec(
        &mock_hybrid,
        &format!(
            r#"#!/usr/bin/env python3
import json, os, shlex, sys
state_file = {state_file:?}
probe_file = {probe_file:?}
for raw in sys.stdin:
    line = raw.strip()
    if not line:
        continue
    n = int(open(state_file).read().strip()) if os.path.exists(state_file) else 0
    n += 1
    open(state_file, "w").write(str(n))
    if n == 1:
        cmd = 'fd4_state=OPEN; ( : >&4 ) 2>/dev/null || fd4_state=CLOSED; printf "ANIX_OUT=%s FD4=%s\\n" "${{ANIX_OUT:-UNSET}}" "$fd4_state" > ' + shlex.quote(probe_file)
        print(json.dumps({{"type": "text_delta", "text": "INTERMEDIATE_NOTE"}}), flush=True)
        print(json.dumps({{"type": "tool_call", "id": "c_hyb", "name": "msh", "arguments": {{"cmd": cmd}}}}), flush=True)
        print(json.dumps({{"type": "usage", "input_tokens": 20, "output_tokens": 10}}), flush=True)
        print(json.dumps({{"type": "done", "finish_reason": "tool_calls"}}), flush=True)
    else:
        print(json.dumps({{"type": "text_delta", "text": "HYBRID_FINAL_REPLY"}}), flush=True)
        print(json.dumps({{"type": "usage", "input_tokens": 30, "output_tokens": 10}}), flush=True)
        print(json.dumps({{"type": "done", "finish_reason": "stop"}}), flush=True)
"#,
            state_file = state_hybrid.display().to_string(),
            probe_file = probe_file.display().to_string(),
        ),
    );

    fs::write(
        cfg_dir.join("config.toml"),
        format!(
            "[defaults]\nprovider = \"hybrid\"\nmodel = \"m1\"\n\n[providers.hybrid]\nbin = \"{}\"\nmode = \"coprocess\"\napi_key_env = \"\"\n\n[providers.pure]\nbin = \"{}\"\nmode = \"coprocess\"\napi_key_env = \"\"\n",
            mock_hybrid.display(),
            mock_pure.display(),
        ),
    )
    .unwrap();

    let hybrid_script = work.join("hybrid.agent");
    write_exec(
        &hybrid_script,
        "#!/usr/bin/env anix\n---\nname: hybrid\nprovider: hybrid\napproval: auto\n---\nLegacy prompt mentioning ANIX_OUT.\n",
    );

    let mut cmd_hyb = base_cmd(&home, &work);
    cmd_hyb
        .arg(&hybrid_script)
        .arg("go")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_child_fds(&mut cmd_hyb, None);
    let out_hyb = cmd_hyb.output().expect("run hybrid");
    assert_eq!(out_hyb.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&out_hyb.stdout),
        "HYBRID_FINAL_REPLY"
    );
    let hyb_stderr = String::from_utf8_lossy(&out_hyb.stderr);
    assert!(
        hyb_stderr.contains("INTERMEDIATE_NOTE"),
        "intermediate text must flush to stderr: {hyb_stderr:?}"
    );
    assert!(
        !hyb_stderr.contains("HYBRID_FINAL_REPLY"),
        "final reply must not double-print to stderr: {hyb_stderr:?}"
    );
    assert!(
        hyb_stderr.contains("warning:") && hyb_stderr.contains("ANIX_OUT"),
        "expected migration warning for ANIX_OUT in prompt: {hyb_stderr:?}"
    );
    let probe = fs::read_to_string(&probe_file).unwrap();
    assert_eq!(
        probe.trim(),
        "ANIX_OUT=UNSET FD4=CLOSED",
        "fd 4 and ANIX_OUT must be unmounted in output=reply: {probe:?}"
    );

    // Real 2-stage pipeline: `hybrid.agent` -> `to_en.agent` (`shell: none`)!
    let _ = fs::remove_file(&state_hybrid);
    let mut p1_cmd = base_cmd(&home, &work);
    p1_cmd
        .arg(&hybrid_script)
        .arg("go")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    configure_child_fds(&mut p1_cmd, None);
    let mut p1 = p1_cmd.spawn().expect("spawn pipe stage 1");
    let p1_stdout = p1.stdout.take().unwrap();

    let mut p2_cmd = base_cmd(&home, &work);
    p2_cmd
        .args(["--provider", "pure"])
        .arg(&pure_script)
        .stdin(Stdio::from(p1_stdout))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_child_fds(&mut p2_cmd, None);
    let p2_out = p2_cmd.output().expect("run pipe stage 2");
    let p1_status = p1.wait().expect("wait pipe stage 1");
    assert_eq!(p1_status.code(), Some(0));
    assert_eq!(p2_out.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&p2_out.stdout),
        "EN:HYBRID_FINAL_REPLY"
    );

    // -------------------------------------------------------------------------
    // Case 3: `output: explicit` (`-o explicit`) and Case 4: `output: none` (`-o none`)
    // -------------------------------------------------------------------------
    let state_exp = root.join("exp_turn");
    let mock_exp = root.join("mock-exp.sh");
    write_exec(
        &mock_exp,
        &format!(
            r#"#!/usr/bin/env python3
import json, os, sys
state_file = {state_file:?}
for raw in sys.stdin:
    line = raw.strip()
    if not line:
        continue
    n = int(open(state_file).read().strip()) if os.path.exists(state_file) else 0
    n += 1
    open(state_file, "w").write(str(n))
    if n == 1:
        cmd = 'if [ -n "${{ANIX_OUT:-}}" ]; then printf "RAW_EXPLICIT" > "$ANIX_OUT"; else touch side_effect_only.txt; fi'
        print(json.dumps({{"type": "tool_call", "id": "c_exp", "name": "msh", "arguments": {{"cmd": cmd}}}}), flush=True)
        print(json.dumps({{"type": "done", "finish_reason": "tool_calls"}}), flush=True)
    else:
        print(json.dumps({{"type": "text_delta", "text": "PROSE_ON_STDERR"}}), flush=True)
        print(json.dumps({{"type": "done", "finish_reason": "stop"}}), flush=True)
"#,
            state_file = state_exp.display().to_string(),
        ),
    );
    fs::write(
        cfg_dir.join("config.toml"),
        format!(
            "[defaults]\nprovider = \"exp\"\nmodel = \"m1\"\n\n[providers.exp]\nbin = \"{}\"\nmode = \"coprocess\"\napi_key_env = \"\"\n",
            mock_exp.display(),
        ),
    )
    .unwrap();
    let exp_script = work.join("exp.agent");
    write_exec(
        &exp_script,
        "#!/usr/bin/env anix\n---\nname: exp\napproval: auto\n---\nRun.\n",
    );

    // Case 3: `-o explicit`
    let mut cmd_exp = base_cmd(&home, &work);
    cmd_exp
        .args(["-o", "explicit"])
        .arg(&exp_script)
        .arg("go")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_child_fds(&mut cmd_exp, None);
    let out_exp = cmd_exp.output().expect("run explicit");
    assert_eq!(out_exp.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&out_exp.stdout), "RAW_EXPLICIT");
    assert!(String::from_utf8_lossy(&out_exp.stderr).contains("PROSE_ON_STDERR"));

    // Case 4: `-o none` -> strictly 0 bytes on fd 1, side effect performed, prose on fd 2.
    let _ = fs::remove_file(&state_exp);
    let mut cmd_none = base_cmd(&home, &work);
    cmd_none
        .args(["-o", "none"])
        .arg(&exp_script)
        .arg("go")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_child_fds(&mut cmd_none, None);
    let out_none = cmd_none.output().expect("run none");
    assert_eq!(out_none.status.code(), Some(0));
    assert!(
        out_none.stdout.is_empty(),
        "output=none must keep fd 1 strictly 0 bytes, got {:?}",
        String::from_utf8_lossy(&out_none.stdout)
    );
    assert!(work.join("side_effect_only.txt").is_file());
    assert!(String::from_utf8_lossy(&out_none.stderr).contains("PROSE_ON_STDERR"));

    // -------------------------------------------------------------------------
    // Case 5: Startup Fail-Fast (exit 2, before writing WAL `session_init`)
    // -------------------------------------------------------------------------
    let ff_wal = root.join("never_created.ctx.jsonl");
    for args in [
        vec!["-o", "explicit", "--shell", "none"],
        vec!["-o", "explicit", "--approval", "deny"],
        vec!["-o", "reply", "--shell", "/nonexistent/anix-no-such-shell"],
    ] {
        let mut cmd_ff = base_cmd(&home, &work);
        cmd_ff
            .args(&args)
            .arg("-c")
            .arg(&ff_wal)
            .arg(&exp_script)
            .arg("go")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        configure_child_fds(&mut cmd_ff, None);
        let out_ff = cmd_ff.output().expect("run fail-fast");
        assert_eq!(
            out_ff.status.code(),
            Some(2),
            "args={args:?} expected exit 2, stderr={}",
            String::from_utf8_lossy(&out_ff.stderr)
        );
        assert!(
            !ff_wal.exists(),
            "fail-fast must exit before creating WAL for args={args:?}"
        );
    }

    // -------------------------------------------------------------------------
    // Case 6: Legacy WAL lacking `output` field resumes under default `output=reply`
    //         and emits trailing `<runtime_update>` drift notice!
    // -------------------------------------------------------------------------
    let legacy_wal = root.join("legacy_v11.ctx.jsonl");
    fs::write(
        &legacy_wal,
        concat!(
            r#"{"ir_version":1,"type":"session_init","ts":"2026-11-24T12:00:00Z","session_id":"sid-legacy-120","config":{"stderr":"auto","approval":"auto","vision":true}}"#,
            "\n",
            r#"{"type":"message","turn_id":0,"role":"system","ts":"2026-11-24T12:00:00Z","text":"FROZEN_LEGACY_SYS"}"#,
            "\n",
            r#"{"type":"message","turn_id":1,"role":"user","ts":"2026-11-24T12:00:01Z","text":"hi"}"#,
            "\n",
            r#"{"type":"message","turn_id":2,"role":"assistant","ts":"2026-11-24T12:00:02Z","text":"ok"}"#,
            "\n"
        ),
    )
    .unwrap();
    let legacy_req_log = root.join("legacy_req.jsonl");
    let mock_echo_req = root.join("mock-echo-req.sh");
    write_exec(
        &mock_echo_req,
        &format!(
            r#"#!/usr/bin/env python3
import json, sys
for raw in sys.stdin:
    line = raw.strip()
    if not line:
        continue
    with open({log:?}, "a") as f:
        f.write(line + "\n")
    print(json.dumps({{"type": "text_delta", "text": "RESUMED_OK"}}), flush=True)
    print(json.dumps({{"type": "done", "finish_reason": "stop"}}), flush=True)
"#,
            log = legacy_req_log.display().to_string(),
        ),
    );
    fs::write(
        cfg_dir.join("config.toml"),
        format!(
            "[defaults]\nprovider = \"echo\"\nmodel = \"m1\"\n\n[providers.echo]\nbin = \"{}\"\nmode = \"coprocess\"\napi_key_env = \"\"\n",
            mock_echo_req.display(),
        ),
    )
    .unwrap();

    let mut cmd_leg = base_cmd(&home, &work);
    cmd_leg
        .args(["-c"])
        .arg(&legacy_wal)
        .arg(&exp_script)
        .arg("continue")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_child_fds(&mut cmd_leg, None);
    let out_leg = cmd_leg.output().expect("run legacy resume");
    assert_eq!(out_leg.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&out_leg.stdout), "RESUMED_OK");
    let leg_req = fs::read_to_string(&legacy_req_log).unwrap();
    assert!(
        leg_req.contains("output contract switched to reply"),
        "expected output drift runtime_update for legacy WAL: {leg_req}"
    );
    let leg_wal_after = fs::read_to_string(&legacy_wal).unwrap();
    assert!(
        leg_wal_after.contains("\"type\":\"runtime_update\"")
            && leg_wal_after.contains("\"output\":\"reply\""),
        "expected persisted runtime_update in WAL: {leg_wal_after}"
    );

    // -------------------------------------------------------------------------
    // Case 7: Error safety under `output=reply` + `shell=msh`: partial text flushes
    //         to fd 2 and keeps fd 1 strictly at 0 bytes!
    // -------------------------------------------------------------------------
    let mock_err = root.join("mock-err.sh");
    write_exec(
        &mock_err,
        r#"#!/usr/bin/env sh
while IFS= read -r line; do
  [ -z "$line" ] && continue
  printf '%s\n' '{"type":"text_delta","text":"PARTIAL_BEFORE_ERR"}'
  printf '%s\n' '{"type":"error","code":"upstream","message":"boom","retryable":false}'
done
"#,
    );
    fs::write(
        cfg_dir.join("config.toml"),
        format!(
            "[defaults]\nprovider = \"errp\"\nmodel = \"m1\"\n\n[providers.errp]\nbin = \"{}\"\nmode = \"coprocess\"\napi_key_env = \"\"\n",
            mock_err.display(),
        ),
    )
    .unwrap();
    let mut cmd_err = base_cmd(&home, &work);
    cmd_err
        .arg(&exp_script)
        .arg("fail")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_child_fds(&mut cmd_err, None);
    let out_err = cmd_err.output().expect("run error safety");
    assert_eq!(out_err.status.code(), Some(1));
    assert!(
        out_err.stdout.is_empty(),
        "partial text before error must NEVER reach fd 1, got {:?}",
        String::from_utf8_lossy(&out_err.stdout)
    );
    assert!(
        String::from_utf8_lossy(&out_err.stderr).contains("PARTIAL_BEFORE_ERR"),
        "partial text before error should flush to fd 2 for diagnostics"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_v0_3_0_tree_wal_branching_fold_checkpoint_and_leaf_validation() {
    let root = make_temp_env("v030-tree-wal");
    let home = setup_fhs_home(&root);
    let work = root.join("work");
    fs::create_dir_all(&work).unwrap();
    let cfg_dir = home.join(".config/anix");
    fs::create_dir_all(cfg_dir.join("hooks/on-exit.d")).unwrap();

    // Hook capturing OnExitPayload (including v0.3.0 `leaf_id`).
    let on_exit_log = root.join("on_exit.jsonl");
    write_exec(
        &cfg_dir.join("hooks/on-exit.d/10-capture"),
        &format!(
            "#!/usr/bin/env sh\ncat >> \"{}\"\nprintf '\\n' >> \"{}\"\n",
            on_exit_log.display(),
            on_exit_log.display()
        ),
    );

    let req_log = root.join("reqs.jsonl");
    let step_file = root.join("step");
    let mock_prov = root.join("mock-tree-prov.py");
    write_exec(
        &mock_prov,
        &format!(
            r#"#!/usr/bin/env python3
import json, os, sys
req_log = {req_log:?}
step_file = {step_file:?}

for raw in sys.stdin:
    line = raw.strip()
    if not line:
        continue
    with open(req_log, "a") as f:
        f.write(line + "\n")
    n = 0
    if os.path.exists(step_file):
        with open(step_file) as f:
            n = int(f.read().strip() or "0")
    n += 1
    with open(step_file, "w") as f:
        f.write(str(n))

    if n == 1:
        # Run 1: root branch initial reply
        print(json.dumps({{"type": "text_delta", "text": "BRANCH_A_ROOT_REPLY"}}), flush=True)
        print(json.dumps({{"type": "usage", "input_tokens": 100, "output_tokens": 20}}), flush=True)
        print(json.dumps({{"type": "done", "finish_reason": "stop"}}), flush=True)
    elif n == 2:
        # Run 2 turn 1: execute tool that folds turns >= 3 (`aclr -t 3 "dead-end A"`)
        print(json.dumps({{"type": "tool_call", "id": "c1", "name": "msh", "arguments": {{"cmd": "aclr -t 3 'dead-end A'"}}}}), flush=True)
        print(json.dumps({{"type": "usage", "input_tokens": 300, "output_tokens": 30}}), flush=True)
        print(json.dumps({{"type": "done", "finish_reason": "tool_calls"}}), flush=True)
    elif n == 3:
        # Run 2 turn 2: final reply after fold
        print(json.dumps({{"type": "text_delta", "text": "AFTER_FOLD_OK"}}), flush=True)
        print(json.dumps({{"type": "usage", "input_tokens": 120, "output_tokens": 10}}), flush=True)
        print(json.dumps({{"type": "done", "finish_reason": "stop"}}), flush=True)
    elif n == 4:
        # Run 3 turn 1: trigger full compaction (`aclr -s`)
        print(json.dumps({{"type": "tool_call", "id": "c2", "name": "msh", "arguments": {{"cmd": "aclr -s"}}}}), flush=True)
        print(json.dumps({{"type": "usage", "input_tokens": 140, "output_tokens": 15}}), flush=True)
        print(json.dumps({{"type": "done", "finish_reason": "tool_calls"}}), flush=True)
    elif n == 5:
        # Run 3 compactor call (tools == [])
        print(json.dumps({{"type": "text_delta", "text": "Compacted Branch A summary"}}), flush=True)
        print(json.dumps({{"type": "usage", "input_tokens": 50, "output_tokens": 10}}), flush=True)
        print(json.dumps({{"type": "done", "finish_reason": "stop"}}), flush=True)
    elif n == 6:
        # Run 3 turn 2: post-checkpoint reply
        print(json.dumps({{"type": "text_delta", "text": "POST_CKPT_OK"}}), flush=True)
        print(json.dumps({{"type": "usage", "input_tokens": 80, "output_tokens": 12}}), flush=True)
        print(json.dumps({{"type": "done", "finish_reason": "stop"}}), flush=True)
    elif n == 7:
        # Run 4 (`--leaf 4`): Branch B reply
        print(json.dumps({{"type": "text_delta", "text": "BRANCH_B_REPLY"}}), flush=True)
        print(json.dumps({{"type": "usage", "input_tokens": 110, "output_tokens": 18}}), flush=True)
        print(json.dumps({{"type": "done", "finish_reason": "stop"}}), flush=True)
    else:
        # Run 5 (`ANIX_LEAF=3` empty stdin): Regenerate from user node 3
        print(json.dumps({{"type": "text_delta", "text": "REGEN_FROM_USER3"}}), flush=True)
        print(json.dumps({{"type": "usage", "input_tokens": 90, "output_tokens": 14}}), flush=True)
        print(json.dumps({{"type": "done", "finish_reason": "stop"}}), flush=True)
"#,
            req_log = req_log.display().to_string(),
            step_file = step_file.display().to_string(),
        ),
    );

    fs::write(
        cfg_dir.join("config.toml"),
        format!(
            "[defaults]\nprovider = \"mock\"\nmodel = \"m1\"\n\n[providers.mock]\nbin = \"{}\"\nmode = \"coprocess\"\napi_key_env = \"\"\n",
            mock_prov.display()
        ),
    )
    .unwrap();

    let script = work.join("tree.agent");
    fs::write(
        &script,
        "---\nname: tree-agent\napproval: auto\noutput: reply\n---\nTree test agent.\n",
    )
    .unwrap();

    let wal_path = root.join("tree.wal.jsonl");

    // -------------------------------------------------------------------------
    // Run 1: Fresh Tree WAL (`id` 1=session_init, 2=system, 3=user, 4=assistant)
    // -------------------------------------------------------------------------
    let trace1_path = root.join("trace1.jsonl");
    let trace1_file = File::create(&trace1_path).unwrap();
    let mut cmd1 = base_cmd(&home, &work);
    cmd1.args(["-c"])
        .arg(&wal_path)
        .args(["--trace-fd", "6"])
        .arg(&script)
        .arg("root prompt")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_child_fds(&mut cmd1, Some((&trace1_file, 6)));
    let out1 = cmd1.output().expect("run 1");
    drop(trace1_file);
    assert_eq!(
        out1.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&out1.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out1.stdout), "BRANCH_A_ROOT_REPLY");

    let t1_events: Vec<Value> = fs::read_to_string(&trace1_path)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let run1_start = t1_events.iter().find(|e| e["type"] == "run_start").unwrap();
    assert!(run1_start.get("leaf_id").is_none());
    let run1_end = t1_events.iter().find(|e| e["type"] == "run_end").unwrap();
    assert_eq!(run1_end["leaf_id"], 4);
    let cyc1_end = t1_events.iter().find(|e| e["type"] == "cycle_end").unwrap();
    assert_eq!(cyc1_end["leaf_id"], 4);

    let on_exit1: Value = serde_json::from_str(
        fs::read_to_string(&on_exit_log)
            .unwrap()
            .lines()
            .last()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(on_exit1["leaf_id"], 4);

    // -------------------------------------------------------------------------
    // Run 2: Resume Branch A -> tool calls `aclr -t 3 'dead-end A'` -> fold node links parent_id=4
    // -------------------------------------------------------------------------
    let trace2_path = root.join("trace2.jsonl");
    let trace2_file = File::create(&trace2_path).unwrap();
    let mut cmd2 = base_cmd(&home, &work);
    cmd2.args(["-c"])
        .arg(&wal_path)
        .args(["--trace-fd", "6"])
        .arg(&script)
        .arg("try dead end A")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_child_fds(&mut cmd2, Some((&trace2_file, 6)));
    let out2 = cmd2.output().expect("run 2");
    drop(trace2_file);
    assert_eq!(
        out2.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&out2.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out2.stdout), "AFTER_FOLD_OK");

    let t2_events: Vec<Value> = fs::read_to_string(&trace2_path)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let fold_ev = t2_events.iter().find(|e| e["type"] == "fold").unwrap();
    assert_eq!(fold_ev["node_id"], 8);
    assert_eq!(fold_ev["parent_id"], 4);
    let run2_end = t2_events.iter().find(|e| e["type"] == "run_end").unwrap();
    assert_eq!(run2_end["leaf_id"], 9);

    // Verify WAL fold event persisted `parent_id: 4` and `pruned_usage.input_tokens: 300`.
    let wal_lines2: Vec<Value> = fs::read_to_string(&wal_path)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let fold_wal = wal_lines2
        .iter()
        .find(|v| v["type"] == "fold")
        .expect("fold WAL line");
    assert_eq!(fold_wal["id"], 8);
    assert_eq!(fold_wal["parent_id"], 4);
    assert_eq!(fold_wal["pruned_usage"]["input_tokens"], 300);

    // -------------------------------------------------------------------------
    // Run 3: Resume Branch A across fold -> triggers `aclr -s` checkpoint
    //        Verify exact branch usage continuity across fold restart!
    // -------------------------------------------------------------------------
    let trace3_path = root.join("trace3.jsonl");
    let trace3_file = File::create(&trace3_path).unwrap();
    let mut cmd3 = base_cmd(&home, &work);
    cmd3.args(["-c"])
        .arg(&wal_path)
        .args(["--trace-fd", "6"])
        .arg(&script)
        .arg("compact branch A")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_child_fds(&mut cmd3, Some((&trace3_file, 6)));
    let out3 = cmd3.output().expect("run 3");
    drop(trace3_file);
    assert_eq!(
        out3.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&out3.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out3.stdout), "POST_CKPT_OK");

    let t3_events: Vec<Value> = fs::read_to_string(&trace3_path)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let run3_start = t3_events.iter().find(|e| e["type"] == "run_start").unwrap();
    assert_eq!(run3_start["leaf_id"], 9);
    let compact_ev = t3_events.iter().find(|e| e["type"] == "compact").unwrap();
    let ckpt_node_id = compact_ev["checkpoint_node_id"].as_u64().unwrap();
    let run3_end = t3_events.iter().find(|e| e["type"] == "run_end").unwrap();
    let branch_a_tip = run3_end["leaf_id"].as_u64().unwrap();
    assert!(branch_a_tip > ckpt_node_id);
    // Exact lineage usage across fold + checkpoint: 100 (run1) + 300 (pruned) + 120 (after fold)
    // + 140 (pre-ckpt turn) + 50 (compact) + 80 (post-ckpt turn) = 790!
    assert_eq!(run3_end["session_usage"]["input_tokens"], 790);

    // -------------------------------------------------------------------------
    // Run 4: `--leaf 4` branches back BEFORE both `fold` (8) AND `checkpoint`!
    // -------------------------------------------------------------------------
    let trace4_path = root.join("trace4.jsonl");
    let trace4_file = File::create(&trace4_path).unwrap();
    let mut cmd4 = base_cmd(&home, &work);
    cmd4.args(["-c"])
        .arg(&wal_path)
        .args(["--leaf", "4", "--trace-fd", "6"])
        .arg(&script)
        .arg("try branch B")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_child_fds(&mut cmd4, Some((&trace4_file, 6)));
    let out4 = cmd4.output().expect("run 4");
    drop(trace4_file);
    assert_eq!(
        out4.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&out4.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out4.stdout), "BRANCH_B_REPLY");

    let t4_events: Vec<Value> = fs::read_to_string(&trace4_path)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let run4_start = t4_events.iter().find(|e| e["type"] == "run_start").unwrap();
    assert_eq!(run4_start["leaf_id"], 4);
    let run4_end = t4_events.iter().find(|e| e["type"] == "run_end").unwrap();
    let branch_b_tip = run4_end["leaf_id"].as_u64().unwrap();
    assert!(branch_b_tip > branch_a_tip);
    // Branch B usage MUST only include node 4's lineage (100) + Branch B's turn (110) = 210!
    assert_eq!(run4_end["session_usage"]["input_tokens"], 210);

    // Verify req 7 (Branch B) saw ONLY root lineage + new prompt (no Branch A fold/checkpoint leakage).
    let reqs: Vec<Value> = fs::read_to_string(&req_log)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let req7_str = reqs[6].to_string();
    assert!(req7_str.contains("BRANCH_A_ROOT_REPLY"));
    assert!(req7_str.contains("try branch B"));
    assert!(!req7_str.contains("dead-end A"));
    assert!(!req7_str.contains("Compacted Branch A summary"));
    assert!(!req7_str.contains("POST_CKPT_OK"));

    // -------------------------------------------------------------------------
    // Run 5: `ANIX_LEAF=3` with empty stdin regenerates directly from user node 3!
    // -------------------------------------------------------------------------
    let mut cmd5 = base_cmd(&home, &work);
    cmd5.env("ANIX_LEAF", "3")
        .args(["-c"])
        .arg(&wal_path)
        .arg(&script)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_child_fds(&mut cmd5, None);
    let out5 = cmd5.output().expect("run 5");
    assert_eq!(
        out5.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&out5.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out5.stdout), "REGEN_FROM_USER3");

    let reqs_after5: Vec<Value> = fs::read_to_string(&req_log)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let req8_msgs = reqs_after5[7]["messages"].as_array().unwrap();
    assert_eq!(req8_msgs.len(), 2, "expected [system, user(3)] only");
    assert_eq!(
        req8_msgs[1]["text"].as_str().unwrap(),
        "root prompt",
        "CLI positional args must arrive as raw plain text without <args> wrapper"
    );
    let last_wal_line: Value = serde_json::from_str(
        fs::read_to_string(&wal_path)
            .unwrap()
            .lines()
            .last()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(last_wal_line["parent_id"], 3);
    assert_eq!(last_wal_line["text"], "REGEN_FROM_USER3");

    // -------------------------------------------------------------------------
    // Run 6: Fail-Fast `--leaf` exit code 2 validation (zero WAL mutation)
    // -------------------------------------------------------------------------
    let wal_snapshot = fs::read(&wal_path).unwrap();
    let bad_cases: Vec<Vec<String>> = vec![
        vec![
            "-c".into(),
            wal_path.display().to_string(),
            "--leaf".into(),
            "0".into(),
        ],
        vec![
            "-c".into(),
            wal_path.display().to_string(),
            "--leaf".into(),
            "1".into(),
        ],
        vec![
            "-c".into(),
            wal_path.display().to_string(),
            "--leaf".into(),
            ckpt_node_id.to_string(),
        ],
        vec![
            "-c".into(),
            wal_path.display().to_string(),
            "--leaf".into(),
            "9999".into(),
        ],
        vec!["-c".into(), String::new(), "--leaf".into(), "4".into()],
    ];
    for args in bad_cases {
        let mut cmd_bad = base_cmd(&home, &work);
        cmd_bad
            .args(&args)
            .arg(&script)
            .arg("nope")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        configure_child_fds(&mut cmd_bad, None);
        let out_bad = cmd_bad.output().expect("run invalid --leaf");
        assert_eq!(
            out_bad.status.code(),
            Some(2),
            "args={args:?} stderr={}",
            String::from_utf8_lossy(&out_bad.stderr)
        );
    }
    assert_eq!(
        fs::read(&wal_path).unwrap(),
        wal_snapshot,
        "invalid --leaf calls must NEVER mutate WAL"
    );

    let _ = fs::remove_dir_all(&root);
}
