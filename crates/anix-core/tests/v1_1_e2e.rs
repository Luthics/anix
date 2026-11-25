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
if [ "$1" = "-t" ] && [ "$#" -eq 2 ]; then
  printf '{"op":"fold","from_turn":%s}\n' "$2" > "$ANIX_CTL"
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
        .env_remove("ANIX_YES")
        .env_remove("ANIX_CONTEXT")
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

    let script = work.join("hello.anix");
    fs::write(
        &script,
        "---\nname: trace-demo\napproval: auto\n---\nSay hi.\n",
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

    let script = work.join("a.anix");
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

    let script = work.join("v.anix");
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

    let script = work.join("worker.anix");
    fs::write(
        &script,
        "---\nname: inbox-worker\napproval: auto\n---\nWork.\n",
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

    let script = workdir.join("sess.anix");
    write_exec(
        &script,
        "#!/usr/bin/env anix\n---\nname: sess-test\n---\nSession test prompt\n",
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
