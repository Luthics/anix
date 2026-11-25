# How to Run Long Commands, Background Jobs & Async Sub-Agents

`msh` natively supports per-call `timeout_sec` and automatic yield-to-background (v1.1.2) — no manual `nohup setsid ... 4>&- 5>&- 6>&- &` boilerplate is needed for ordinary long commands or servers.

## 1. Preferred: `msh` Built-In Background & Per-Call `timeout_sec`
- **Start a server or long job immediately in background** (`timeout_sec: 0`):
  - Call `msh` with `{"cmd": "my-server --port 8080", "timeout_sec": 0}`.
  - `msh` launches it in an isolated `setsid()` process group (with side channels `4/5/6` automatically closed), writes stdout/stderr to a persistent log file, and returns `job`, `pid`, `pgid`, `log`, and `exit_file` immediately.
- **Long foreground builds / tests** (`timeout_sec: 300`, clamped to `max_msh_timeout_sec`):
  - If the command finishes in time, normal output and exit code are returned directly.
  - If it exceeds `timeout_sec` (with default `kill_on_timeout: false`), `msh` **does NOT kill it** — it moves it to background, returns the partial output plus `pid`/`pgid`/`log`/`exit_file`, and `anix` automatically injects a `<runtime_update>` notification with exit code and log tail once it completes.
- **Hard-kill on timeout** (only when desired): pass `"kill_on_timeout": true` alongside `"timeout_sec": N`.
- **Inspect or stop a background job via POSIX commands**:
  - Progress: `tail -n 50 <log_path>`
  - Alive check: `kill -0 <pid> 2>/dev/null && echo running || cat <exit_file>`
  - Stop entire process group cleanly: `kill -TERM -<pgid>`

## 2. Persistent Async Child Agent (`--inbox`)
Start an independent `--inbox` daemon with `msh` `timeout_sec: 0`:
```sh
./worker.agent -q -c /tmp/w.ctx.jsonl --inbox /tmp/w-inbox --idle-timeout 300 >/tmp/w.out 2>/tmp/w.err
```

## 3. Manual POSIX Detach (Only When Spawning Background Subcommands Inside a Multi-Step Foreground Script)
If a foreground `msh` script itself spawns a background command with `&`, redirect `0/1/2` and close `4>&- 5>&- 6>&-` so inherited side channels are not held open:
```sh
nohup setsid my-server </dev/null >/tmp/server.log 2>&1 4>&- 5>&- 6>&- &
echo $! > /tmp/server.pid
```

## Pitfalls
- **Container / CI cgroup boundary**: background processes still belong to the container or CI step's cgroup — when PID 1 or the CI job exits, the kernel reaps all remaining processes.
- Always stop long-lived test servers (`kill -TERM -<pgid>`) once verification is complete.
