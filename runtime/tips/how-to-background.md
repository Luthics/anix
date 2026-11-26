# How to Run Long Commands, Background Jobs & Async Sub-Agents

`msh` natively supports per-call `timeout_sec`, automatic yield-to-background, `hold` leases, and automatic Maildir reaper callbacks (`$ANIX_INBOX`).

## 1. Preferred: `msh` Built-In Background & Maildir Wakeup
- **Start a server or long job immediately in background** (`timeout_sec: 0`):
  - Call `shell` (`msh`) with `{"cmd": "my-server --port 8080", "timeout_sec": 0}`.
  - `msh` launches it in an isolated `setsid()` process group (with side channels closed), writes stdout/stderr to a persistent log file, emits a `hold` lease on `$ANIX_CTL`, and returns `job`, `pid`, `pgid`, `log`, and `exit_file` immediately.
- **Long foreground builds / tests** (`timeout_sec: 300`):
  - If it finishes in time, output and exit code are returned directly.
  - If it exceeds `timeout_sec` (default `kill_on_timeout: false`), `msh` moves it to background, emits a `hold` lease, and returns the handle.
  - When running inside a persistent `--inbox` agent (`$ANIX_INBOX` is set), `msh`'s background reaper automatically delivers an atomic Maildir completion message (`Anix-In-Reply-To: <job_id>`) back to `$ANIX_INBOX`, releasing the `hold` lease and immediately waking the sleeping agent without any `sleep` polling!
- **Hard-kill on timeout** (only when desired): pass `"kill_on_timeout": true` alongside `"timeout_sec": N`.
- **Inspect or stop a background job via POSIX commands**:
  - Progress: `tail -n 50 <log_path>`
  - Alive check: `kill -0 <pid> 2>/dev/null && echo running || cat <exit_file>`
  - Stop entire process group cleanly: `kill -TERM -<pgid>`

## 2. Async Sub-Agent with `--notify` Callback
Launch an async child agent that notifies your `$ANIX_INBOX` upon exit:
```sh
./worker.agent -q -c /tmp/w.ctx.jsonl --notify "$ANIX_INBOX" --notify-id "task-1" "Build release" >/tmp/w.out 2>/tmp/w.err
```
Or launch a persistent `--inbox` daemon with `timeout_sec: 0` and talk to it synchronously via `anix send /tmp/w-inbox --wait "status?"`.

## 3. Manual POSIX Detach (Only Inside Multi-Step Foreground Scripts)
If a foreground script spawns a background command with `&`, redirect `0/1/2` and close inherited side channels (`4>&- 5>&- 6>&-`):
```sh
nohup setsid my-server </dev/null >/tmp/server.log 2>&1 4>&- 5>&- 6>&- &
echo $! > /tmp/server.pid
```

## Pitfalls
- **Container / CI cgroup boundary**: background processes still belong to the container or CI step's cgroup — when PID 1 exits, the kernel reaps them.
- Always stop long-lived test servers (`kill -TERM -<pgid>`) once verification is complete.
