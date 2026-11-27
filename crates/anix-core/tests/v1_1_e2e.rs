//! End-to-end integration tests for Anix v0.4.0 ("Pure 3-Primitive Unix OS Agent Kernel"):
//! - Explicit `--trace-fd=<N>`, no implicit `fd 3`, `cycle_start`/`cycle_end`/`idle` Trace events
//! - POSIX `StrictModes` gate (exit 75) & Layer 2 `anix-approve` wrapper chain (`shell: anix-approve -- msh`)
//! - Layer 2 standalone `pre-context.d/` hooks (`20-skills`, `30-tips`), media passthrough, custom `tool_name`
//! - `aclr -t` auto-summary tail fold, `aclr -s` checkpoint, persistent `coprocess`, `session_id`
//! - Maildir IPC: `anix send --wait [--timeout]`, burst batching + debounce, `cur/` crash dedup,
//!   `Anix-Control: compact`, `--notify`, and `ANIX_INBOX` write-only isolation
//! - `hold` / `release` leases, `msh` background reaper Maildir wakeup, and `hold-expired` synthesis
//! - Orthogonal `output=reply|explicit|none` & `shell=none`, 4-layer explicit `--config` (no `.anix/` upward search)
//! - Tree WAL `--leaf`, `--lock-wait`, parallel `anix fork`, `anix fsck`, and `anix log`

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::io::{AsRawFd, RawFd};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

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

fn target_bin_dir() -> PathBuf {
    let bin = agent_bin();
    let dir = bin.parent().expect("anix binary parent dir");
    // When running via `cargo test`, `CARGO_BIN_EXE_anix` is in `target/debug/`;
    // when running cross-compiled test binary on remote Linux, binaries may live in `../libexec/anix`.
    dir.to_path_buf()
}

fn find_companion_bin(name: &str) -> Option<PathBuf> {
    let dir = target_bin_dir();
    let mut candidates = vec![dir.join(name)];
    if let Some(parent) = dir.parent() {
        candidates.push(parent.join("libexec/anix").join(name));
        candidates.push(parent.join("libexec/anix/tools").join(name));
        candidates.push(parent.join(name));
    }
    if let Ok(home) = std::env::var("HOME") {
        let h = PathBuf::from(home);
        candidates.push(h.join(".local/libexec/anix").join(name));
        candidates.push(h.join(".local/libexec/anix/tools").join(name));
    }
    candidates.into_iter().find(|p| p.is_file())
}

fn make_temp_env(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "anix-v04-{tag}-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    fs::create_dir_all(&dir).expect("create temp test dir");
    let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o755));
    dir
}

fn mkdir_0755(path: &Path) {
    fs::create_dir_all(path).expect("create dir");
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o755));
}

fn write_exec(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        mkdir_0755(parent);
    }
    fs::write(path, content).expect("write executable script");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("chmod 0755");
}

fn setup_fhs_home(root: &Path) -> PathBuf {
    let home = root.join("home");
    let libexec = home.join(".local/libexec/anix");
    let tools = libexec.join("tools");
    mkdir_0755(&tools);

    for bin_name in ["msh", "anix-approve", "anix-hook"] {
        if let Some(src) = find_companion_bin(bin_name) {
            let _ = std::os::unix::fs::symlink(&src, libexec.join(bin_name));
        } else if bin_name == "msh" {
            write_exec(
                &libexec.join("msh"),
                "#!/usr/bin/env sh\nexec /bin/sh \"$@\"\n",
            );
        }
    }
    if let Some(src) = find_companion_bin("aclr") {
        let _ = std::os::unix::fs::symlink(&src, tools.join("aclr"));
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
        .env_remove("ANIX_CONFIG")
        .env_remove("ANIX_HOOKS_DIR")
        .env_remove("ANIX_TRACE_FD")
        .env_remove("ANIX_TRACE_LEVEL")
        .env_remove("ANIX_OUTPUT")
        .env_remove("ANIX_SHELL")
        .env_remove("ANIX_STDERR")
        .env_remove("ANIX_CONTEXT")
        .env_remove("ANIX_LEAF")
        .env_remove("ANIX_INBOX")
        .env_remove("ANIX_IDLE_TIMEOUT");
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
    mkdir_0755(&work);

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
    mkdir_0755(&cfg_dir);
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
        "---\nname: trace-demo\noutput: explicit\n---\nSay hi.\n",
    )
    .unwrap();

    // 1. Open writable fd 3 WITHOUT `--trace-fd`: MUST NOT touch fd 3!
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
    assert!(fs::read_to_string(&fd3_path).unwrap().is_empty());
    let stderr1 = String::from_utf8_lossy(&out1.stderr);
    assert!(
        stderr1.contains("hello-no-nl\n[usage] "),
        "expected newline before [usage], got: {stderr1:?}"
    );
    assert!(stderr1.ends_with('\n'));

    // 2. Closed/unwritable `--trace-fd=7` MUST fail with exit code 2.
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

    // 3. Valid `--trace-fd=4` writes full cycle_start/cycle_end JSONL.
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
    let cycle_end = events
        .iter()
        .find(|v| v["type"] == "cycle_end")
        .expect("cycle_end event");
    assert_eq!(cycle_end["cycle_id"], 1);
    assert_eq!(cycle_end["source"], "initial");
    assert_eq!(cycle_end["reason"], "completed");
    assert_eq!(cycle_end["exit_code"], 0);
    assert_eq!(cycle_end["cycle_usage"]["input_tokens"], 100);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn strict_modes_exit_75_and_layer2_anix_approve_wrapper_chain() {
    let root = make_temp_env("strict-approve");
    let home = setup_fhs_home(&root);
    let work = root.join("work");
    mkdir_0755(&work);

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
{{"type":"tool_call","id":"c1","name":"shell","arguments":{{"cmd":"touch allowed.txt"}}}}
{{"type":"done","finish_reason":"tool_calls"}}
EOF
  elif [ "$n" -eq 2 ]; then
    cat <<'EOF'
{{"type":"tool_call","id":"c2","name":"shell","arguments":{{"cmd":"touch blocked.txt"}}}}
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

    let decider = root.join("decider.sh");
    write_exec(
        &decider,
        r#"#!/usr/bin/env sh
input=$(cat)
case "$input" in
  *allowed.txt*) exit 0 ;;
  *blocked.txt*) echo "blocked by policy"; exit 1 ;;
  *) exit 2 ;;
esac
"#,
    );

    let cfg_dir = home.join(".config/anix");
    mkdir_0755(&cfg_dir);
    fs::write(
        cfg_dir.join("config.toml"),
        format!(
            "[defaults]\nprovider = \"mock\"\nmodel = \"m1\"\n\n[providers.mock]\nbin = \"{}\"\nmode = \"coprocess\"\napi_key_env = \"\"\n",
            mock_prov.display()
        ),
    )
    .unwrap();

    let script = work.join("a.agent");
    fs::write(
        &script,
        format!(
            "---\nname: app-demo\nshell: anix-approve --mode ask --decider {} -- msh\n---\nRun commands.\n",
            decider.display()
        ),
    )
    .unwrap();

    // 1. POSIX StrictModes: group-writable provider binary MUST fail-closed with exit 75!
    fs::set_permissions(&mock_prov, fs::Permissions::from_mode(0o775)).unwrap();
    let mut cmd_sm = base_cmd(&home, &work);
    cmd_sm
        .arg(&script)
        .arg("go")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_child_fds(&mut cmd_sm, None);
    let out_sm = cmd_sm.output().expect("run strict modes violation");
    assert_eq!(
        out_sm.status.code(),
        Some(75),
        "expected exit 75 for group-writable provider, stderr={}",
        String::from_utf8_lossy(&out_sm.stderr)
    );
    fs::set_permissions(&mock_prov, fs::Permissions::from_mode(0o755)).unwrap();

    // 2. POSIX StrictModes on `pre-context.d/` hook script (0775 -> exit 75; 0755 -> ok).
    let pre_dir = cfg_dir.join("hooks/pre-context.d");
    mkdir_0755(&pre_dir);
    let bad_hook = pre_dir.join("10-noop");
    write_exec(&bad_hook, "#!/usr/bin/env sh\ncat\n");
    fs::set_permissions(&bad_hook, fs::Permissions::from_mode(0o775)).unwrap();
    let mut cmd_hook_sm = base_cmd(&home, &work);
    cmd_hook_sm
        .arg(&script)
        .arg("go")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_child_fds(&mut cmd_hook_sm, None);
    let out_hook_sm = cmd_hook_sm
        .output()
        .expect("run hook strict modes violation");
    assert_eq!(out_hook_sm.status.code(), Some(75));
    fs::set_permissions(&bad_hook, fs::Permissions::from_mode(0o755)).unwrap();

    // 3. Layer 2 `anix-approve --mode ask --decider ... -- msh`:
    //    turn 1 allowed -> allowed.txt created; turn 2 soft-denied (exit 126) -> model sees error & finishes!
    let _ = fs::remove_file(&state_file);
    let mut cmd_ok = base_cmd(&home, &work);
    cmd_ok
        .arg(&script)
        .arg("go")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_child_fds(&mut cmd_ok, None);
    let out_ok = cmd_ok.output().expect("run anix-approve wrapper chain");
    assert_eq!(
        out_ok.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&out_ok.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out_ok.stdout), "done");
    assert!(work.join("allowed.txt").is_file());
    assert!(!work.join("blocked.txt").exists());

    // 4. `anix-approve --mode deny --fail-on-deny -- msh` aborts `anix` with exit 75 (`EX_TEMPFAIL`).
    let _ = fs::remove_file(&state_file);
    let mut cmd_deny = base_cmd(&home, &work);
    cmd_deny
        .args(["--shell", "anix-approve --mode deny --fail-on-deny -- msh"])
        .arg(&script)
        .arg("go")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_child_fds(&mut cmd_deny, None);
    let out_deny = cmd_deny.output().expect("run anix-approve fail-on-deny");
    assert_eq!(
        out_deny.status.code(),
        Some(75),
        "stderr={}",
        String::from_utf8_lossy(&out_deny.stderr)
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn layer2_standalone_hooks_skills_tips_media_passthrough_and_custom_tool_name() {
    let root = make_temp_env("l2-hooks-media");
    let home = setup_fhs_home(&root);
    let work = root.join("work");
    mkdir_0755(&work);

    // Install Layer 2 `20-skills` and `30-tips` symlinks pointing to `anix-hook`.
    let hook_bin = home.join(".local/libexec/anix/anix-hook");
    assert!(
        hook_bin.exists(),
        "anix-hook companion binary must exist in test FHS"
    );
    let pre_dir = home.join(".config/anix/hooks/pre-context.d");
    mkdir_0755(&pre_dir);
    std::os::unix::fs::symlink(&hook_bin, pre_dir.join("20-skills")).unwrap();
    std::os::unix::fs::symlink(&hook_bin, pre_dir.join("30-tips")).unwrap();

    // Create a project skill to verify `20-skills` discovers it.
    let skill_dir = work.join(".anix/skills/demo-skill");
    mkdir_0755(&skill_dir);
    fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: demo-skill\ndescription: Demo skill description\n---\nBody.\n",
    )
    .unwrap();

    let emit_media = root.join("emit-media.sh");
    write_exec(
        &emit_media,
        r#"#!/usr/bin/env sh
printf '%s\n' '{"op":"media","mime":"image/png","path":"/tmp/fake.png","source":"demo.png"}' > "$ANIX_CTL"
"#,
    );

    let req_log = root.join("reqs.jsonl");
    let state_file = root.join("turn_state");
    let mock_prov = root.join("mock-provider.sh");
    write_exec(
        &mock_prov,
        &format!(
            r#"#!/usr/bin/env sh
STATE="{}"
EMIT="{}"
REQLOG="{}"
while IFS= read -r line; do
  [ -z "$line" ] && continue
  printf '%s\n' "$line" >> "$REQLOG"
  n=0
  [ -f "$STATE" ] && n=$(cat "$STATE")
  n=$((n + 1))
  echo "$n" > "$STATE"
  if [ "$n" -eq 1 ]; then
    printf '%s\n' "{{\"type\":\"tool_call\",\"id\":\"c1\",\"name\":\"exec_sh\",\"arguments\":{{\"cmd\":\"$EMIT\"}}}}"
    printf '%s\n' '{{"type":"done","finish_reason":"tool_calls"}}'
  else
    printf '%s\n' '{{"type":"text_delta","text":"media-ok"}}'
    printf '%s\n' '{{"type":"done","finish_reason":"stop"}}'
  fi
done
"#,
            state_file.display(),
            emit_media.display(),
            req_log.display()
        ),
    );

    let cfg_dir = home.join(".config/anix");
    fs::write(
        cfg_dir.join("config.toml"),
        format!(
            "[defaults]\nprovider = \"mock\"\nmodel = \"m1\"\n\n[providers.mock]\nbin = \"{}\"\nmode = \"coprocess\"\napi_key_env = \"\"\n",
            mock_prov.display()
        ),
    )
    .unwrap();

    let script = work.join("v.agent");
    fs::write(
        &script,
        "---\nname: l2-demo\ntool_name: exec_sh\n---\nInspect.\n",
    )
    .unwrap();

    let wal_path = root.join("l2.wal.jsonl");
    let mut cmd = base_cmd(&home, &work);
    cmd.args(["-c"])
        .arg(&wal_path)
        .arg(&script)
        .arg("inspect")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_child_fds(&mut cmd, None);
    let out = cmd.output().expect("run l2 hooks + media passthrough");
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), "media-ok");

    let reqs: Vec<Value> = fs::read_to_string(&req_log)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(reqs.len(), 2);
    // Custom `tool_name: exec_sh` exposed in IR header (no hardcoded "msh"!).
    assert_eq!(reqs[0]["header"]["tools"][0]["name"], "exec_sh");
    let sys_text = reqs[0]["messages"][0]["text"].as_str().unwrap();
    assert!(
        sys_text.contains("<available_skills>") && sys_text.contains("demo-skill"),
        "expected Layer 2 20-skills output in system: {sys_text}"
    );
    assert!(
        sys_text.contains("<runtime_tips>")
            && sys_text.contains("how-to-runtime.md")
            && sys_text.contains("how-to-mcp.md"),
        "expected Layer 2 30-tips output in system: {sys_text}"
    );

    // Media frame passed straight through to Provider in turn 2 tool message!
    let tool_msg = reqs[1]["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["role"] == "tool")
        .expect("tool msg");
    assert_eq!(tool_msg["media"][0]["mime"], "image/png");
    assert_eq!(tool_msg["media"][0]["path"], "/tmp/fake.png");

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn aclr_auto_summary_tail_fold_coprocess_reuse_and_session_id_persistence() {
    let root = make_temp_env("autofold-session");
    let home = setup_fhs_home(&root);
    let work = root.join("work");
    mkdir_0755(&work);

    let sid_log = root.join("provider_sids.jsonl");
    let state_file = root.join("turn_counter.txt");
    let wal_path = root.join("session.ctx.jsonl");
    let trace1_path = root.join("trace1.jsonl");
    let trace2_path = root.join("trace2.jsonl");

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
            "pid": os.getpid(),
            "env_sid": env_sid,
            "req_sid": req.get("session_id"),
            "tools_len": len(req["header"]["tools"]),
            "has_folded": "Auto folded dead-end" in json.dumps(req["messages"]),
        }}) + "\n")
    if n == 1:
        cmd = 'printf "sid=%s\\n" "${{ANIX_SESSION_ID:-UNSET}}" > "$ANIX_OUT"; aclr -t 2'
        print(json.dumps({{"type": "tool_call", "id": "call_s1", "name": "shell", "arguments": {{"cmd": cmd}}}}), flush=True)
        print(json.dumps({{"type": "usage", "input_tokens": 50, "output_tokens": 10}}), flush=True)
        print(json.dumps({{"type": "done", "finish_reason": "tool_calls"}}), flush=True)
    elif n == 2:
        # Auto-fold summarizer (`tools: []`)
        print(json.dumps({{"type": "text_delta", "text": "Auto folded dead-end"}}), flush=True)
        print(json.dumps({{"type": "usage", "input_tokens": 60, "output_tokens": 15}}), flush=True)
        print(json.dumps({{"type": "done", "finish_reason": "stop"}}), flush=True)
    elif n == 3:
        cmd = 'aclr -s'
        print(json.dumps({{"type": "tool_call", "id": "call_s2", "name": "shell", "arguments": {{"cmd": cmd}}}}), flush=True)
        print(json.dumps({{"type": "usage", "input_tokens": 45, "output_tokens": 8}}), flush=True)
        print(json.dumps({{"type": "done", "finish_reason": "tool_calls"}}), flush=True)
    elif n == 4:
        # Full compaction (`tools: []`)
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

    let cfg_dir = home.join(".config/anix");
    mkdir_0755(&cfg_dir);
    fs::write(
        cfg_dir.join("config.toml"),
        format!(
            "[defaults]\nprovider = \"mock\"\nmodel = \"mock-model\"\n\n[providers.mock]\nbin = \"{}\"\nmode = \"coprocess\"\napi_key_env = \"\"\n",
            mock_provider.display()
        ),
    )
    .unwrap();

    let script = work.join("sess.agent");
    write_exec(
        &script,
        "#!/usr/bin/env anix\n---\nname: sess-test\noutput: explicit\n---\nSession test prompt\n",
    );

    let tf1 = File::create(&trace1_path).unwrap();
    let mut cmd1 = base_cmd(&home, &work);
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
    assert_eq!(String::from_utf8_lossy(&out1.stdout), "sid=UNSET\n");

    let tf2 = File::create(&trace2_path).unwrap();
    let mut cmd2 = base_cmd(&home, &work);
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

    let records: Vec<Value> = fs::read_to_string(&sid_log)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(records.len(), 6, "records={records:?}");
    // All 5 calls in run 1 reused the exact same coprocess PID!
    let run1_pid = records[0]["pid"].as_i64().unwrap();
    for rec in &records[0..5] {
        assert_eq!(rec["pid"].as_i64().unwrap(), run1_pid);
    }
    assert_eq!(records[1]["tools_len"], 0);
    assert_eq!(records[2]["has_folded"], true);
    assert_eq!(records[3]["tools_len"], 0);
    // All 6 calls across run 1 + checkpoint + run 2 shared the exact same `session_id`!
    let sid = records[0]["req_sid"].as_str().unwrap();
    for rec in &records {
        assert_eq!(rec["req_sid"].as_str(), Some(sid));
        assert_eq!(rec["env_sid"].as_str(), Some(sid));
    }

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn v0_4_0_maildir_send_wait_batch_debounce_crash_dedup_compact_and_notify() {
    let root = make_temp_env("maildir-ipc");
    let home = setup_fhs_home(&root);
    let work = root.join("work");
    mkdir_0755(&work);

    let req_log = root.join("reqs.jsonl");
    let mock_prov = root.join("mock-maildir-prov.py");
    write_exec(
        &mock_prov,
        &format!(
            r#"#!/usr/bin/env python3
import json, sys

req_log = {req_log:?}
for raw in sys.stdin:
    line = raw.strip()
    if not line:
        continue
    with open(req_log, "a") as f:
        f.write(line + "\n")
    req = json.loads(line)
    if len(req["header"]["tools"]) == 0:
        print(json.dumps({{"type": "text_delta", "text": "MAILDIR_COMPACT_SUMMARY"}}), flush=True)
        print(json.dumps({{"type": "usage", "input_tokens": 20, "output_tokens": 5}}), flush=True)
        print(json.dumps({{"type": "done", "finish_reason": "stop"}}), flush=True)
        continue
    last_user = ""
    for m in req["messages"]:
        if m["role"] == "user":
            last_user = m["text"]
    reply = "ECHO:" + last_user.replace("\n", "|")
    print(json.dumps({{"type": "text_delta", "text": reply}}), flush=True)
    print(json.dumps({{"type": "usage", "input_tokens": 30, "output_tokens": 10}}), flush=True)
    print(json.dumps({{"type": "done", "finish_reason": "stop"}}), flush=True)
"#,
            req_log = req_log.display().to_string(),
        ),
    );

    let cfg_dir = home.join(".config/anix");
    mkdir_0755(&cfg_dir);
    fs::write(
        cfg_dir.join("config.toml"),
        format!(
            "[defaults]\nprovider = \"mock\"\nmodel = \"m1\"\n\n[providers.mock]\nbin = \"{}\"\nmode = \"coprocess\"\napi_key_env = \"\"\n",
            mock_prov.display()
        ),
    )
    .unwrap();

    let script = work.join("daemon.agent");
    fs::write(&script, "---\nname: maildir-daemon\n---\nEcho.\n").unwrap();

    // 1. HARD RULE: `ANIX_INBOX` in env MUST NEVER activate `--inbox` mode!
    let ignored_inbox = root.join("never_used_inbox");
    let mut cmd_env_guard = base_cmd(&home, &work);
    cmd_env_guard
        .env("ANIX_INBOX", &ignored_inbox)
        .arg(&script)
        .arg("oneshot-check")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_child_fds(&mut cmd_env_guard, None);
    let out_guard = cmd_env_guard.output().expect("run with ANIX_INBOX env");
    assert_eq!(out_guard.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&out_guard.stdout),
        "ECHO:oneshot-check"
    );
    assert!(
        !ignored_inbox.exists(),
        "ANIX_INBOX env must never be read as --inbox"
    );

    // 2. `anix send --wait --timeout 1` on unwatched Maildir exits 124!
    let dead_inbox = root.join("dead-inbox");
    let out_to = base_cmd(&home, &work)
        .args([
            "send",
            &dead_inbox.display().to_string(),
            "--wait",
            "--timeout",
            "1",
            "nobody home",
        ])
        .stdin(Stdio::null())
        .output()
        .expect("run send timeout");
    assert_eq!(out_to.status.code(), Some(124));

    // 3. Prepare WAL with an already-committed message_id `dup-msg-1` + pre-seeded `cur/`
    //    containing BOTH `dup-msg-1` (must be skipped straight to done/) AND `uncommitted-2`
    //    (must be recovered & processed once)!
    let inbox_dir = root.join("inbox");
    let notify_dir = root.join("notify");
    let wal_path = root.join("daemon.wal.jsonl");
    let trace_path = root.join("daemon.trace.jsonl");
    fs::write(
        &wal_path,
        concat!(
            r#"{"type":"session_init","id":1,"ir_version":1,"ts":"2026-11-27T00:00:00Z","session_id":"sid-md","config":{"output":"reply","shell":"msh","tool_name":"shell","stderr":"auto"}}"#,
            "\n",
            r#"{"type":"message","id":2,"parent_id":1,"turn_id":0,"role":"system","ts":"2026-11-27T00:00:00Z","text":"sys"}"#,
            "\n",
            r#"{"type":"message","id":3,"parent_id":2,"turn_id":1,"role":"user","ts":"2026-11-27T00:00:01Z","text":"already done","message_ids":["dup-msg-1"]}"#,
            "\n",
            r#"{"type":"message","id":4,"parent_id":3,"turn_id":2,"role":"assistant","ts":"2026-11-27T00:00:02Z","text":"ok"}"#,
            "\n",
        ),
    )
    .unwrap();
    for sub in ["tmp", "new", "cur", "done"] {
        mkdir_0755(&inbox_dir.join(sub));
    }
    fs::write(
        inbox_dir.join("cur/001-dup.msg"),
        "Anix-Message-Id: dup-msg-1\n\nSHOULD_NOT_REPROCESS",
    )
    .unwrap();
    fs::write(
        inbox_dir.join("cur/002-rec.msg"),
        "Anix-Message-Id: rec-msg-2\n\nRECOVER_ME",
    )
    .unwrap();

    // Launch persistent `--inbox` daemon with `--notify` and `--idle-timeout 2`!
    let trace_file = File::create(&trace_path).unwrap();
    let mut daemon_cmd = base_cmd(&home, &work);
    daemon_cmd
        .args([
            "--inbox",
            &inbox_dir.display().to_string(),
            "--inbox-debounce-ms",
            "150",
            "--idle-timeout",
            "2",
            "--notify",
            &notify_dir.display().to_string(),
            "--notify-id",
            "daemon-42",
            "--trace-fd",
            "6",
            "-c",
            &wal_path.display().to_string(),
        ])
        .arg(&script)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_child_fds(&mut daemon_cmd, Some((&trace_file, 6)));
    let daemon = daemon_cmd.spawn().expect("spawn maildir daemon");

    // Wait briefly for startup cur/ recovery, then fire:
    // (a) Synchronous `anix send --wait` RPC
    thread::sleep(Duration::from_millis(300));
    let sync_out = base_cmd(&home, &work)
        .args([
            "send",
            &inbox_dir.display().to_string(),
            "--wait",
            "--timeout",
            "10",
            "--from",
            "client-a",
            "SYNC_PING",
        ])
        .stdin(Stdio::null())
        .output()
        .expect("run anix send --wait");
    assert_eq!(
        sync_out.status.code(),
        Some(0),
        "sync send stderr={}",
        String::from_utf8_lossy(&sync_out.stderr)
    );
    let sync_reply = String::from_utf8_lossy(&sync_out.stdout);
    assert!(
        sync_reply.contains("SYNC_PING"),
        "expected sync reply containing SYNC_PING, got: {sync_reply:?}"
    );

    // (b) Burst of TWO messages arriving simultaneously -> MUST coalesce into ONE batch!
    let b1 = base_cmd(&home, &work)
        .args(["send", &inbox_dir.display().to_string(), "BURST_ONE"])
        .stdin(Stdio::null())
        .status()
        .expect("send burst 1");
    let b2 = base_cmd(&home, &work)
        .args(["send", &inbox_dir.display().to_string(), "BURST_TWO"])
        .stdin(Stdio::null())
        .status()
        .expect("send burst 2");
    assert!(b1.success() && b2.success());

    // Wait for burst cycle to finish, then send `Anix-Control: compact` synchronously!
    thread::sleep(Duration::from_millis(500));
    let cmp_out = base_cmd(&home, &work)
        .args([
            "send",
            &inbox_dir.display().to_string(),
            "--wait",
            "--timeout",
            "10",
            "--control",
            "compact",
        ])
        .stdin(Stdio::null())
        .output()
        .expect("send --control compact");
    assert_eq!(
        cmp_out.status.code(),
        Some(0),
        "compact stderr={}",
        String::from_utf8_lossy(&cmp_out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&cmp_out.stdout), "compacted\n");

    // Daemon now idles out cleanly after 2s and writes `--notify`!
    let daemon_out = daemon.wait_with_output().expect("wait daemon");
    drop(trace_file);
    assert_eq!(
        daemon_out.status.code(),
        Some(0),
        "daemon stderr={}",
        String::from_utf8_lossy(&daemon_out.stderr)
    );

    // Verify dedup skipped `SHOULD_NOT_REPROCESS` while recovering `RECOVER_ME`.
    let req_text = fs::read_to_string(&req_log).unwrap();
    assert!(
        !req_text.contains("SHOULD_NOT_REPROCESS"),
        "already-committed cur/ message must be deduplicated"
    );
    assert!(
        req_text.contains("RECOVER_ME"),
        "uncommitted cur/ message must be recovered"
    );
    // Verify BURST_ONE and BURST_TWO coalesced into ONE single request!
    let burst_reqs: Vec<&str> = req_text
        .lines()
        .filter(|l| l.contains("BURST_ONE") && l.contains("BURST_TWO"))
        .collect();
    assert!(
        !burst_reqs.is_empty(),
        "expected BURST_ONE and BURST_TWO batched into one turn: {req_text}"
    );
    // Verify WAL contains checkpoint from `Anix-Control: compact` and `message_ids`.
    let wal_after = fs::read_to_string(&wal_path).unwrap();
    assert!(wal_after.contains("\"type\":\"checkpoint\""));
    assert!(wal_after.contains("MAILDIR_COMPACT_SUMMARY"));
    assert!(wal_after.contains("\"message_ids\""));

    // Verify `--notify` delivered exit notification with `Anix-Exit-Code: 0` & `Anix-In-Reply-To: daemon-42`!
    let notify_files: Vec<PathBuf> = fs::read_dir(notify_dir.join("new"))
        .expect("read notify/new")
        .flatten()
        .map(|e| e.path())
        .collect();
    assert_eq!(notify_files.len(), 1, "expected 1 notify file");
    let notify_content = fs::read_to_string(&notify_files[0]).unwrap();
    assert!(notify_content.contains("Anix-Exit-Code: 0"));
    assert!(notify_content.contains("Anix-In-Reply-To: daemon-42"));
    assert!(notify_content.contains("Anix-From: maildir-daemon"));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn v0_4_0_hold_lease_msh_reaper_wakeup_and_hold_expired() {
    let root = make_temp_env("hold-reaper");
    let home = setup_fhs_home(&root);
    let work = root.join("work");
    mkdir_0755(&work);

    // -------------------------------------------------------------------------
    // Subcase A: `msh` background job (`timeout_sec: 0`) emits `hold` lease,
    // prevents `--idle-timeout 1` exit, and reaper delivers Maildir to `$ANIX_INBOX`
    // at t=2s waking the idle agent and releasing the `hold`!
    // -------------------------------------------------------------------------
    let req_log_a = root.join("reqs_a.jsonl");
    let state_a = root.join("state_a");
    let mock_a = root.join("mock-reaper.py");
    write_exec(
        &mock_a,
        &format!(
            r#"#!/usr/bin/env python3
import json, os, sys

req_log = {req_log:?}
state_file = {state_file:?}
for raw in sys.stdin:
    line = raw.strip()
    if not line:
        continue
    with open(req_log, "a") as f:
        f.write(line + "\n")
    n = int(open(state_file).read().strip()) if os.path.exists(state_file) else 0
    n += 1
    open(state_file, "w").write(str(n))
    if n == 1:
        print(json.dumps({{"type": "tool_call", "id": "c_bg", "name": "shell", "arguments": {{"cmd": "sleep 2; echo REAP-WOKE-ME", "timeout_sec": 0}}}}), flush=True)
        print(json.dumps({{"type": "done", "finish_reason": "tool_calls"}}), flush=True)
    elif n == 2:
        print(json.dumps({{"type": "text_delta", "text": "waiting-on-bg"}}), flush=True)
        print(json.dumps({{"type": "done", "finish_reason": "stop"}}), flush=True)
    else:
        print(json.dumps({{"type": "text_delta", "text": "reaper-handled"}}), flush=True)
        print(json.dumps({{"type": "done", "finish_reason": "stop"}}), flush=True)
"#,
            req_log = req_log_a.display().to_string(),
            state_file = state_a.display().to_string(),
        ),
    );

    let cfg_dir = home.join(".config/anix");
    mkdir_0755(&cfg_dir);
    fs::write(
        cfg_dir.join("config.toml"),
        format!(
            "[defaults]\nprovider = \"mocka\"\nmodel = \"m1\"\n\n[providers.mocka]\nbin = \"{}\"\nmode = \"coprocess\"\napi_key_env = \"\"\n",
            mock_a.display()
        ),
    )
    .unwrap();

    let script_a = work.join("reaper.agent");
    fs::write(&script_a, "---\nname: reaper-agent\n---\nRun bg job.\n").unwrap();

    let inbox_a = root.join("inbox_a");
    let trace_a = root.join("trace_a.jsonl");
    let tf_a = File::create(&trace_a).unwrap();
    let mut cmd_a = base_cmd(&home, &work);
    cmd_a
        .args([
            "--inbox",
            &inbox_a.display().to_string(),
            "--idle-timeout",
            "1",
            "--inbox-debounce-ms",
            "50",
            "--trace-fd",
            "5",
        ])
        .arg(&script_a)
        .arg("launch bg")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_child_fds(&mut cmd_a, Some((&tf_a, 5)));
    let out_a = cmd_a.output().expect("run reaper hold test");
    drop(tf_a);
    assert_eq!(
        out_a.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&out_a.stderr)
    );

    let reqs_a = fs::read_to_string(&req_log_a).unwrap();
    assert_eq!(reqs_a.lines().count(), 3, "expected 3 turns, got: {reqs_a}");
    assert!(
        reqs_a.contains("REAP-WOKE-ME") && reqs_a.contains("from=\\\"msh\\\""),
        "expected msh reaper Maildir delivery in turn 3: {reqs_a}"
    );
    let events_a: Vec<Value> = fs::read_to_string(&trace_a)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert!(events_a.iter().any(|e| e["type"] == "hold_started"));
    assert!(
        events_a
            .iter()
            .any(|e| e["type"] == "hold_released" && e["reason"] == "in_reply_to"),
        "events={events_a:?}"
    );

    // -------------------------------------------------------------------------
    // Subcase B: Unreleased `hold` lease expires -> kernel synthesizes
    // `<runtime_update>Hold expired: ...</runtime_update>` and wakes model!
    // -------------------------------------------------------------------------
    let req_log_b = root.join("reqs_b.jsonl");
    let state_b = root.join("state_b");
    let mock_b = root.join("mock-expire.py");
    write_exec(
        &mock_b,
        &format!(
            r#"#!/usr/bin/env python3
import json, os, sys

req_log = {req_log:?}
state_file = {state_file:?}
for raw in sys.stdin:
    line = raw.strip()
    if not line:
        continue
    with open(req_log, "a") as f:
        f.write(line + "\n")
    n = int(open(state_file).read().strip()) if os.path.exists(state_file) else 0
    n += 1
    open(state_file, "w").write(str(n))
    if n == 1:
        cmd = 'printf \'{{"op":"hold","id":"h_stuck","deadline_sec":1,"label":"stuck worker"}}\\n\' > "$ANIX_CTL"'
        print(json.dumps({{"type": "tool_call", "id": "c_h", "name": "shell", "arguments": {{"cmd": cmd}}}}), flush=True)
        print(json.dumps({{"type": "done", "finish_reason": "tool_calls"}}), flush=True)
    elif n == 2:
        print(json.dumps({{"type": "text_delta", "text": "holding"}}), flush=True)
        print(json.dumps({{"type": "done", "finish_reason": "stop"}}), flush=True)
    else:
        print(json.dumps({{"type": "text_delta", "text": "recovered-after-expiry"}}), flush=True)
        print(json.dumps({{"type": "done", "finish_reason": "stop"}}), flush=True)
"#,
            req_log = req_log_b.display().to_string(),
            state_file = state_b.display().to_string(),
        ),
    );

    fs::write(
        cfg_dir.join("config.toml"),
        format!(
            "[defaults]\nprovider = \"mockb\"\nmodel = \"m1\"\n\n[providers.mockb]\nbin = \"{}\"\nmode = \"coprocess\"\napi_key_env = \"\"\n",
            mock_b.display()
        ),
    )
    .unwrap();

    let inbox_b = root.join("inbox_b");
    let trace_b = root.join("trace_b.jsonl");
    let tf_b = File::create(&trace_b).unwrap();
    let mut cmd_b = base_cmd(&home, &work);
    cmd_b
        .args([
            "--inbox",
            &inbox_b.display().to_string(),
            "--idle-timeout",
            "1",
            "--trace-fd",
            "5",
        ])
        .arg(&script_a)
        .arg("register hold")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_child_fds(&mut cmd_b, Some((&tf_b, 5)));
    let out_b = cmd_b.output().expect("run hold expired test");
    drop(tf_b);
    assert_eq!(out_b.status.code(), Some(0));

    let reqs_b = fs::read_to_string(&req_log_b).unwrap();
    assert!(
        reqs_b.contains("Hold expired: id=h_stuck (stuck worker)"),
        "expected synthetic hold-expired runtime_update: {reqs_b}"
    );
    let events_b: Vec<Value> = fs::read_to_string(&trace_b)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert!(
        events_b
            .iter()
            .any(|e| e["type"] == "hold_expired" && e["id"] == "h_stuck"),
        "events={events_b:?}"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn v0_4_0_orthogonal_output_contracts_shell_none_and_explicit_single_config() {
    let root = make_temp_env("v04-output-cfg");
    let home = setup_fhs_home(&root);
    let work = root.join("work");
    mkdir_0755(&work);

    // Place a decoy `.anix/config.toml` in `work`: v0.4.0 MUST completely ignore it!
    let decoy_dir = work.join(".anix");
    mkdir_0755(&decoy_dir);
    fs::write(
        decoy_dir.join("config.toml"),
        "[defaults]\nprovider = \"nonexistent_decoy\"\nmodel = \"bad\"\n",
    )
    .unwrap();

    let mock_pure = root.join("mock-pure.py");
    write_exec(
        &mock_pure,
        r#"#!/usr/bin/env python3
import json, sys
for raw in sys.stdin:
    line = raw.strip()
    if not line:
        continue
    req = json.loads(line)
    last_user = ""
    for m in req["messages"]:
        if m["role"] == "user":
            last_user = m["text"]
    out = "EN:" + last_user.strip()
    print(json.dumps({"type": "text_delta", "text": out}), flush=True)
    print(json.dumps({"type": "usage", "input_tokens": 12, "output_tokens": 6}), flush=True)
    print(json.dumps({"type": "done", "finish_reason": "stop"}), flush=True)
"#,
    );

    // Explicit `--config` file (outside default XDG dir!)
    let explicit_cfg = root.join("custom.toml");
    fs::write(
        &explicit_cfg,
        format!(
            "[defaults]\nprovider = \"pure\"\nmodel = \"m1\"\n\n[providers.pure]\nbin = \"{}\"\nmode = \"coprocess\"\napi_key_env = \"\"\n",
            mock_pure.display()
        ),
    )
    .unwrap();

    let pure_script = work.join("to_en.agent");
    write_exec(
        &pure_script,
        "#!/usr/bin/env anix\n---\nname: to-en\nshell: none\noutput: reply\n---\nTranslate.\n",
    );

    // 1. `--config <FILE>` succeeds & ignores `.anix/config.toml` decoy!
    let mut cmd_pure = base_cmd(&home, &work);
    cmd_pure
        .args(["--config"])
        .arg(&explicit_cfg)
        .arg(&pure_script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_child_fds(&mut cmd_pure, None);
    let mut child = cmd_pure.spawn().expect("spawn pure filter");
    child.stdin.take().unwrap().write_all(b"Ciallo").unwrap();
    let out_pure = child.wait_with_output().expect("wait pure filter");
    assert_eq!(
        out_pure.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&out_pure.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out_pure.stdout), "EN:Ciallo");

    // 2. Startup Fail-Fast exit 2 (never writes WAL):
    //    - explicit `--config` missing file
    //    - `-o explicit --shell none`
    //    - nonexistent `--shell`
    let ff_wal = root.join("never.wal.jsonl");
    for args in [
        vec!["--config", "/nonexistent/anix-missing.toml"],
        vec![
            "--config",
            &explicit_cfg.display().to_string(),
            "-o",
            "explicit",
            "--shell",
            "none",
        ],
        vec![
            "--config",
            &explicit_cfg.display().to_string(),
            "--shell",
            "/nonexistent/no-shell",
        ],
    ] {
        let mut cmd_ff = base_cmd(&home, &work);
        cmd_ff
            .args(&args)
            .arg("-c")
            .arg(&ff_wal)
            .arg(&pure_script)
            .arg("go")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        configure_child_fds(&mut cmd_ff, None);
        let out_ff = cmd_ff.output().expect("run fail-fast");
        assert_eq!(out_ff.status.code(), Some(2), "args={args:?}");
        assert!(!ff_wal.exists());
    }

    // 3. Legacy pre-v0.4 WAL (with old `approval`/`vision` config fields) resumes cleanly!
    let legacy_wal = root.join("legacy.wal.jsonl");
    fs::write(
        &legacy_wal,
        concat!(
            r#"{"ir_version":1,"type":"session_init","ts":"2026-11-24T12:00:00Z","session_id":"sid-legacy","config":{"stderr":"auto","approval":"auto","vision":true}}"#,
            "\n",
            r#"{"type":"message","turn_id":0,"role":"system","ts":"2026-11-24T12:00:00Z","text":"FROZEN_SYS"}"#,
            "\n",
            r#"{"type":"message","turn_id":1,"role":"user","ts":"2026-11-24T12:00:01Z","text":"hi"}"#,
            "\n",
            r#"{"type":"message","turn_id":2,"role":"assistant","ts":"2026-11-24T12:00:02Z","text":"ok"}"#,
            "\n",
        ),
    )
    .unwrap();
    let mut cmd_leg = base_cmd(&home, &work);
    cmd_leg
        .args(["--config"])
        .arg(&explicit_cfg)
        .args(["-c"])
        .arg(&legacy_wal)
        .arg(&pure_script)
        .arg("continue")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_child_fds(&mut cmd_leg, None);
    let out_leg = cmd_leg.output().expect("run legacy wal resume");
    assert_eq!(out_leg.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&out_leg.stdout), "EN:continue");

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn v0_4_0_tree_wal_lock_wait_parallel_fork_fsck_and_log() {
    let root = make_temp_env("tree-fork-lock");
    let home = setup_fhs_home(&root);
    let work = root.join("work");
    mkdir_0755(&work);

    let req_log = root.join("reqs.jsonl");
    let mock_prov = root.join("mock-fork-prov.py");
    write_exec(
        &mock_prov,
        &format!(
            r#"#!/usr/bin/env python3
import json, sys, time

req_log = {req_log:?}
for raw in sys.stdin:
    line = raw.strip()
    if not line:
        continue
    with open(req_log, "a") as f:
        f.write(line + "\n")
    req = json.loads(line)
    last_user = ""
    for m in req["messages"]:
        if m["role"] == "user":
            last_user = m["text"]
    if "SLOW_LOCK" in last_user:
        time.sleep(1.2)
    history = "+".join(m.get("text", "") for m in req["messages"] if m["role"] == "user")
    print(json.dumps({{"type": "text_delta", "text": "HIST:" + history}}), flush=True)
    print(json.dumps({{"type": "usage", "input_tokens": 50, "output_tokens": 10}}), flush=True)
    print(json.dumps({{"type": "done", "finish_reason": "stop"}}), flush=True)
"#,
            req_log = req_log.display().to_string(),
        ),
    );

    let cfg_dir = home.join(".config/anix");
    mkdir_0755(&cfg_dir);
    fs::write(
        cfg_dir.join("config.toml"),
        format!(
            "[defaults]\nprovider = \"mock\"\nmodel = \"m1\"\n\n[providers.mock]\nbin = \"{}\"\nmode = \"coprocess\"\napi_key_env = \"\"\n",
            mock_prov.display()
        ),
    )
    .unwrap();

    let script = work.join("tree.agent");
    fs::write(&script, "---\nname: tree-fork\n---\nTree.\n").unwrap();

    let main_wal = root.join("main.wal.jsonl");

    // 1. Seed main WAL (nodes: 1=init, 2=sys, 3=user("ROOT"), 4=asst)
    let mut cmd1 = base_cmd(&home, &work);
    cmd1.args(["-c"])
        .arg(&main_wal)
        .arg(&script)
        .arg("ROOT")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_child_fds(&mut cmd1, None);
    let out1 = cmd1.output().expect("seed main wal");
    assert_eq!(out1.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&out1.stdout), "HIST:ROOT");

    // 2. Test `--lock-wait`: launch slow writer holding `flock` on `main_wal` for ~1.2s;
    //    - concurrent non-waiting run exits 1 (Locked),
    //    - concurrent `--lock-wait=5` waits cleanly and succeeds!
    let mut slow_cmd = base_cmd(&home, &work);
    slow_cmd
        .args(["-c"])
        .arg(&main_wal)
        .arg(&script)
        .arg("SLOW_LOCK")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_child_fds(&mut slow_cmd, None);
    let slow_child = slow_cmd.spawn().expect("spawn slow writer");
    thread::sleep(Duration::from_millis(250));

    let mut no_wait_cmd = base_cmd(&home, &work);
    no_wait_cmd
        .args(["-c"])
        .arg(&main_wal)
        .arg(&script)
        .arg("FAIL_FAST")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_child_fds(&mut no_wait_cmd, None);
    let out_locked = no_wait_cmd.output().expect("run locked without wait");
    assert_eq!(out_locked.status.code(), Some(1));

    let started_wait = Instant::now();
    let mut wait_cmd = base_cmd(&home, &work);
    wait_cmd
        .args(["--lock-wait=5", "-c"])
        .arg(&main_wal)
        .arg(&script)
        .arg("AFTER_WAIT")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_child_fds(&mut wait_cmd, None);
    let out_wait = wait_cmd.output().expect("run with --lock-wait=5");
    let slow_out = slow_child.wait_with_output().expect("wait slow child");
    assert_eq!(slow_out.status.code(), Some(0));
    assert_eq!(
        out_wait.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&out_wait.stderr)
    );
    assert!(started_wait.elapsed() >= Duration::from_millis(500));
    assert_eq!(
        String::from_utf8_lossy(&out_wait.stdout),
        "HIST:ROOT+SLOW_LOCK+AFTER_WAIT"
    );

    // 3. Cross-file parallel branching via `anix fork`:
    //    Fork `main_wal` at leaf 4 (ROOT only) into TWO independent files `b1` and `b2`,
    //    and run BOTH in parallel with zero lock conflict!
    let b1_wal = root.join("branch1.wal.jsonl");
    let b2_wal = root.join("branch2.wal.jsonl");
    let f1 = base_cmd(&home, &work)
        .args([
            "fork",
            &main_wal.display().to_string(),
            "--leaf",
            "4",
            "-o",
            &b1_wal.display().to_string(),
        ])
        .output()
        .expect("fork b1");
    let f2 = base_cmd(&home, &work)
        .args([
            "fork",
            &main_wal.display().to_string(),
            "--leaf",
            "4",
            "-o",
            &b2_wal.display().to_string(),
        ])
        .output()
        .expect("fork b2");
    assert_eq!(f1.status.code(), Some(0));
    assert_eq!(f2.status.code(), Some(0));

    let mut c_b1 = base_cmd(&home, &work);
    c_b1.args(["-c"])
        .arg(&b1_wal)
        .arg(&script)
        .arg("BRANCH_ONE")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_child_fds(&mut c_b1, None);
    let mut c_b2 = base_cmd(&home, &work);
    c_b2.args(["-c"])
        .arg(&b2_wal)
        .arg(&script)
        .arg("BRANCH_TWO")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_child_fds(&mut c_b2, None);

    let p1 = c_b1.spawn().expect("spawn branch 1");
    let p2 = c_b2.spawn().expect("spawn branch 2");
    let o1 = p1.wait_with_output().expect("wait branch 1");
    let o2 = p2.wait_with_output().expect("wait branch 2");
    assert_eq!(o1.status.code(), Some(0));
    assert_eq!(o2.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&o1.stdout), "HIST:ROOT+BRANCH_ONE");
    assert_eq!(String::from_utf8_lossy(&o2.stdout), "HIST:ROOT+BRANCH_TWO");

    // 4. `anix fsck` & `anix log` on valid & corrupted WALs!
    let fsck_ok = base_cmd(&home, &work)
        .args(["fsck", &b1_wal.display().to_string()])
        .output()
        .expect("fsck b1");
    assert_eq!(fsck_ok.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&fsck_ok.stdout).contains("[OK]"));

    let log_out = base_cmd(&home, &work)
        .args(["log", &b1_wal.display().to_string(), "--leaf", "3"])
        .output()
        .expect("log b1");
    assert_eq!(log_out.status.code(), Some(0));
    let log_text = String::from_utf8_lossy(&log_out.stdout);
    assert!(log_text.contains("ROOT") && log_text.contains("BRANCH_ONE"));

    let bad_wal = root.join("corrupt.wal.jsonl");
    fs::write(&bad_wal, "{\"type\":\"session_init\",\"id\":1,\"ir_version\":1,\"ts\":\"2026-11-27T00:00:00Z\",\"config\":{}}\n{BROKEN_JSON\n").unwrap();
    let fsck_bad = base_cmd(&home, &work)
        .args(["fsck", &bad_wal.display().to_string()])
        .output()
        .expect("fsck bad");
    assert_eq!(fsck_bad.status.code(), Some(1));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn mid_cycle_steering_injects_after_full_tool_group_never_between_tools() {
    let root = make_temp_env("mid-cycle-steer");
    let home = setup_fhs_home(&root);
    let work = root.join("work");
    mkdir_0755(&work);
    let inbox = root.join("inbox");
    let wal_path = root.join("session.wal.jsonl");
    let trace_path = root.join("trace.jsonl");
    let ir_log = root.join("ir_turn2.json");

    // Provider turn 1: emit TWO tool calls (`tc_1`, `tc_2`).
    // `tc_1` drops a steering message into `inbox/new/` while running.
    // Turn 2: record exact message roles seen by provider and assert `STEER_NOW` arrived ONLY
    // AFTER both `tool(tc_1)` and `tool(tc_2)`!
    let mock_prov = root.join("mock-steer.py");
    write_exec(
        &mock_prov,
        &format!(
            r#"#!/usr/bin/env python3
import json, sys

turn = 0
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    req = json.loads(line)
    turn += 1
    if turn == 1:
        events = [
            {{"type": "tool_call", "id": "tc_1", "name": "shell", "arguments": {{"cmd": "anix_steer_drop"}}}},
            {{"type": "tool_call", "id": "tc_2", "name": "shell", "arguments": {{"cmd": "echo second_tool_ok"}}}},
            {{"type": "usage", "input_tokens": 50, "output_tokens": 20}},
            {{"type": "done", "finish_reason": "tool_calls"}},
        ]
    else:
        with open({ir_log:?}, "w") as f:
            json.dump(req["messages"], f)
        events = [
            {{"type": "text_delta", "text": "steered-ok"}},
            {{"type": "usage", "input_tokens": 80, "output_tokens": 10}},
            {{"type": "done", "finish_reason": "stop"}},
        ]
    for ev in events:
        sys.stdout.write(json.dumps(ev) + "\n")
    sys.stdout.flush()
"#,
            ir_log = ir_log.display().to_string(),
        ),
    );

    // Custom shell that handles `anix_steer_drop` by atomically dropping a steering message into inbox/new/.
    let test_shell = root.join("steer-shell.sh");
    write_exec(
        &test_shell,
        &format!(
            r#"#!/usr/bin/env sh
if [ "$1" = "-c" ] && [ "$2" = "anix_steer_drop" ]; then
  printf 'Anix-Message-Id: steer_01\nAnix-From: user\n\nSTEER_NOW\n' > "{inbox}/tmp/steer.msg"
  mv "{inbox}/tmp/steer.msg" "{inbox}/new/9999999999-steer.msg"
  printf 'first_tool_ok\n'
  exit 0
fi
exec /bin/sh "$@"
"#,
            inbox = inbox.display(),
        ),
    );

    let cfg_dir = home.join(".config/anix");
    mkdir_0755(&cfg_dir);
    fs::write(
        cfg_dir.join("config.toml"),
        format!(
            "[defaults]\nprovider = \"mock\"\nmodel = \"m1\"\n\n[providers.mock]\nbin = \"{}\"\nmode = \"coprocess\"\napi_key_env = \"\"\n",
            mock_prov.display()
        ),
    )
    .unwrap();

    let script = work.join("steer.agent");
    fs::write(
        &script,
        format!(
            "---\nname: steer-demo\nshell: {}\noutput: reply\n---\nSteer test.\n",
            test_shell.display()
        ),
    )
    .unwrap();

    let trace_file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&trace_path)
        .unwrap();

    let mut daemon = base_cmd(&home, &work);
    daemon
        .args([
            "--inbox",
            &inbox.display().to_string(),
            "--idle-timeout",
            "1",
            "--inbox-debounce-ms",
            "20",
            "--trace-fd",
            "9",
            "-c",
            &wal_path.display().to_string(),
        ])
        .arg(&script)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    configure_child_fds(&mut daemon, Some((&trace_file, 9)));
    let child = daemon.spawn().expect("spawn steering daemon");
    drop(trace_file);

    // Wait briefly for Maildir creation, then send initial message via `anix send`.
    let wait_start = Instant::now();
    while !inbox.join("new").is_dir() && wait_start.elapsed() < Duration::from_secs(5) {
        thread::sleep(Duration::from_millis(20));
    }
    let send_out = base_cmd(&home, &work)
        .args(["send", &inbox.display().to_string(), "START_CYCLE"])
        .output()
        .expect("send START_CYCLE");
    assert!(send_out.status.success());

    let out = child.wait_with_output().expect("wait daemon");
    assert!(
        out.status.success(),
        "daemon failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Verify exact turn-2 IR message order:
    // [system, user(START_CYCLE), assistant(tc_1, tc_2), tool(tc_1), tool(tc_2), user(STEER_NOW)]
    let ir_raw = fs::read_to_string(&ir_log).expect("read turn 2 IR log");
    let msgs: Vec<Value> = serde_json::from_str(&ir_raw).expect("parse IR messages");
    let roles: Vec<&str> = msgs
        .iter()
        .map(|m| m.get("role").and_then(Value::as_str).unwrap_or(""))
        .collect();
    assert_eq!(
        roles,
        vec!["system", "user", "assistant", "tool", "tool", "user"],
        "steered user message MUST appear ONLY after ALL paired tool results, never between tc_1 and tc_2"
    );
    assert_eq!(
        msgs[3].get("tool_call_id").and_then(Value::as_str),
        Some("tc_1")
    );
    assert_eq!(
        msgs[4].get("tool_call_id").and_then(Value::as_str),
        Some("tc_2")
    );
    assert!(
        msgs[5]
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or("")
            .contains("STEER_NOW"),
        "expected STEER_NOW in steered user message: {:?}",
        msgs[5]
    );

    // Verify `inbox_steered` trace event fired and steered file moved to `done/`.
    let trace_raw = fs::read_to_string(&trace_path).expect("read trace");
    assert!(
        trace_raw.contains("\"type\":\"inbox_steered\"") && trace_raw.contains("steer_01"),
        "expected inbox_steered trace event, got: {trace_raw}"
    );
    assert!(
        inbox.join("done/9999999999-steer.msg").is_file(),
        "steered message must be moved to done/ on cycle commit"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn shell_exposes_self_fork_env_vars_and_mid_turn_fork_passes_fsck() {
    let root = make_temp_env("self-fork-env");
    let home = setup_fhs_home(&root);
    let work = root.join("work");
    mkdir_0755(&work);

    let wal_path = root.join("main.wal.jsonl");
    let forked_path = root.join("child-branch.wal.jsonl");
    let probe_path = root.join("env-probe.txt");
    let anix_exe = PathBuf::from(env!("CARGO_BIN_EXE_anix"));

    let mock_prov = root.join("mock-fork-prov.py");
    write_exec(
        &mock_prov,
        &format!(
            r#"#!/usr/bin/env python3
import json, sys

for raw in sys.stdin:
    line = raw.strip()
    if not line:
        continue
    req = json.loads(line)
    has_tool_res = any(m.get("role") == "tool" for m in req["messages"])
    if not has_tool_res:
        cmd = (
            'printf "WAL=%s\nLEAF=%s\nAGENT=%s\n" "$ANIX_WAL" "$ANIX_LEAF" "$ANIX_AGENT" > '
            + {probe_path:?}
            + ' && '
            + {anix_exe:?}
            + ' fork "$ANIX_WAL" --leaf "$ANIX_LEAF" -o '
            + {forked_path:?}
            + ' && '
            + {anix_exe:?}
            + ' fsck '
            + {forked_path:?}
        )
        print(json.dumps({{
            "type": "tool_call",
            "id": "tc_self_fork",
            "name": "shell",
            "arguments": {{"cmd": cmd}},
        }}), flush=True)
        print(json.dumps({{"type": "usage", "input_tokens": 20, "output_tokens": 10}}), flush=True)
        print(json.dumps({{"type": "done", "finish_reason": "tool_calls"}}), flush=True)
    else:
        print(json.dumps({{"type": "text_delta", "text": "SELF_FORK_OK"}}), flush=True)
        print(json.dumps({{"type": "usage", "input_tokens": 30, "output_tokens": 5}}), flush=True)
        print(json.dumps({{"type": "done", "finish_reason": "stop"}}), flush=True)
"#,
            probe_path = probe_path.display().to_string(),
            anix_exe = anix_exe.display().to_string(),
            forked_path = forked_path.display().to_string(),
        ),
    );

    let cfg_dir = home.join(".config/anix");
    mkdir_0755(&cfg_dir);
    fs::write(
        cfg_dir.join("config.toml"),
        format!(
            "[defaults]\nprovider = \"mock\"\nmodel = \"m1\"\n\n[providers.mock]\nbin = \"{}\"\nmode = \"coprocess\"\napi_key_env = \"\"\n",
            mock_prov.display()
        ),
    )
    .unwrap();

    let script = work.join("self-fork.agent");
    write_exec(
        &script,
        "#!/usr/bin/env anix\n---\nname: self-fork\n---\nTest.\n",
    );

    let mut cmd = base_cmd(&home, &work);
    cmd.args([
        "-c",
        &wal_path.display().to_string(),
        &script.display().to_string(),
        "Please self-fork mid-turn",
    ])
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
    configure_child_fds(&mut cmd, None);
    let out = cmd.output().expect("run self-fork");
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), "SELF_FORK_OK");

    let probe = fs::read_to_string(&probe_path).expect("read env probe");
    let canon_wal = fs::canonicalize(&wal_path).unwrap();
    let canon_script = fs::canonicalize(&script).unwrap();
    assert!(
        probe.contains(&format!("WAL={}", canon_wal.display())),
        "probe={probe}"
    );
    // Node 1=SessionInit, 2=System, 3=User (closed!); Node 4=open Assistant(tool_calls) must NOT be chosen!
    assert!(
        probe.contains("LEAF=3\n"),
        "expected closed leaf=3, got: {probe}"
    );
    assert!(
        probe.contains(&format!("AGENT={}", canon_script.display())),
        "probe={probe}"
    );
    assert!(forked_path.is_file(), "forked WAL must exist and pass fsck");

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn ctl_plan_frame_emits_ephemeral_plan_update_without_wal_bloat() {
    let root = make_temp_env("ctl-plan-frame");
    let home = setup_fhs_home(&root);
    let work = root.join("work");
    mkdir_0755(&work);

    let wal_path = root.join("plan.wal.jsonl");
    let trace_path = root.join("plan.trace.jsonl");

    let mock_prov = root.join("mock-plan-prov.py");
    write_exec(
        &mock_prov,
        r#"#!/usr/bin/env python3
import json, sys

for raw in sys.stdin:
    line = raw.strip()
    if not line:
        continue
    req = json.loads(line)
    has_tool_res = any(m.get("role") == "tool" for m in req["messages"])
    if not has_tool_res:
        cmd = (
            'printf \'{"op":"plan","steps":[{"title":"Inspect","status":"done"},'
            '{"title":"Patch","status":"in_progress"}]}\\n\' > "$ANIX_CTL" && echo PLAN_SENT'
        )
        print(json.dumps({
            "type": "tool_call",
            "id": "tc_plan",
            "name": "shell",
            "arguments": {"cmd": cmd},
        }), flush=True)
        print(json.dumps({"type": "usage", "input_tokens": 12, "output_tokens": 6}), flush=True)
        print(json.dumps({"type": "done", "finish_reason": "tool_calls"}), flush=True)
    else:
        print(json.dumps({"type": "text_delta", "text": "PLAN_E2E_OK"}), flush=True)
        print(json.dumps({"type": "usage", "input_tokens": 20, "output_tokens": 4}), flush=True)
        print(json.dumps({"type": "done", "finish_reason": "stop"}), flush=True)
"#,
    );

    let cfg_dir = home.join(".config/anix");
    mkdir_0755(&cfg_dir);
    fs::write(
        cfg_dir.join("config.toml"),
        format!(
            "[defaults]\nprovider = \"mock\"\nmodel = \"m1\"\n\n[providers.mock]\nbin = \"{}\"\nmode = \"coprocess\"\napi_key_env = \"\"\n",
            mock_prov.display()
        ),
    )
    .unwrap();

    let script = work.join("plan.agent");
    write_exec(
        &script,
        "#!/usr/bin/env anix\n---\nname: plan-test\n---\nTest.\n",
    );

    let trace_file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&trace_path)
        .unwrap();

    let mut cmd = base_cmd(&home, &work);
    cmd.args([
        "--trace-fd",
        "9",
        "-c",
        &wal_path.display().to_string(),
        &script.display().to_string(),
        "Run plan update",
    ])
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
    configure_child_fds(&mut cmd, Some((&trace_file, 9)));
    let out = cmd.output().expect("run plan test");
    drop(trace_file);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), "PLAN_E2E_OK");

    let trace_raw = fs::read_to_string(&trace_path).expect("read trace");
    let plan_line = trace_raw
        .lines()
        .find(|l| l.contains("\"type\":\"plan_update\""))
        .unwrap_or_else(|| panic!("expected plan_update in trace, got:\n{trace_raw}"));
    let parsed: Value = serde_json::from_str(plan_line).expect("parse plan_update JSON");
    let steps = parsed["steps"].as_array().expect("steps array");
    assert_eq!(steps.len(), 2);
    assert_eq!(steps[0]["title"], "Inspect");
    assert_eq!(steps[0]["status"], "done");
    assert_eq!(steps[1]["title"], "Patch");
    assert_eq!(steps[1]["status"], "in_progress");

    // Verify ephemeral contract: WAL must NOT contain any `plan_update` record.
    let wal_raw = fs::read_to_string(&wal_path).expect("read wal");
    assert!(
        !wal_raw.contains("\"type\":\"plan_update\""),
        "plan_update must be ephemeral and never written to WAL"
    );

    let _ = fs::remove_dir_all(&root);
}
