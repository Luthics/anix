# How to Launch Detached Background Processes & Async Sub-Agents

Detach long-running servers, builds, or persistent `--inbox` child agents cleanly so `msh` returns immediately and the background process survives parent exit.

## Pattern
- **Detached background command** (close FDs 4/5/6 and redirect 0/1/2):
  ```sh
  nohup setsid my-server </dev/null >/tmp/server.log 2>&1 4>&- 5>&- 6>&- &
  echo $! > /tmp/server.pid
  ```
- **Persistent async child agent (`--inbox`)**:
  ```sh
  nohup setsid ./worker.anix -q -c /tmp/w.ctx.jsonl --inbox /tmp/w-inbox --idle-timeout 300 \
    </dev/null >/tmp/w.out 2>/tmp/w.err 4>&- 5>&- 6>&- &
  echo $! > /tmp/w.pid
  ```
- On macOS without `setsid`, use `nohup ... </dev/null >/tmp/job.log 2>&1 4>&- 5>&- 6>&- &` or `python3 -c 'import os,sys; os.setsid(); os.execvp(sys.argv[1], sys.argv[1:])' ...`.

## Pitfalls
- **MUST close `4>&- 5>&- 6>&-` and redirect `0/1/2`**: any background child keeping `$ANIX_OUT`/`$ANIX_ERR`/`$ANIX_CTL` or `msh` stdout/stderr open blocks `msh` until `MSH_TIMEOUT_SEC` timeout.
- **Process group isolation**: `setsid`/`nohup` moves the child out of `msh`'s process group so `msh` teardown or parent `anix` exit does not kill it.
- **Container / CI cgroup boundary**: detached processes still belong to the container or CI step's cgroup — when PID 1 or the CI job exits, the kernel reaps all remaining processes.
- Always record PID and log paths so later turns can inspect or stop cleanly (`kill -TERM "$(cat /tmp/w.pid)"`).
