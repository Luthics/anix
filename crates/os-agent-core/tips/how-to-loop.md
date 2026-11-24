# How to Loop / Schedule Recurring Checks (`--inbox`)

Use a Maildir-style `--inbox` daemon plus atomic `tmp/ -> new/` delivery instead of keeping `msh` blocked with `sleep`.

## Pattern
1. Start a persistent worker holding a WAL context:
   ```sh
   ./worker.agent -c /tmp/loop.ctx.jsonl --inbox /tmp/loop-inbox --idle-timeout 0
   ```
2. Deliver a message atomically (NEVER write directly into `new/`):
   ```sh
   msg="$(date +%s%N)-$$.msg"
   printf '%s\n' "Check deploy health" > "/tmp/loop-inbox/tmp/$msg" \
     && mv "/tmp/loop-inbox/tmp/$msg" "/tmp/loop-inbox/new/$msg"
   ```
3. Schedule delayed/recurring triggers from outside using `cron`, `systemd` timers, or a detached notifier:
   ```sh
   (sleep 300; m="$(date +%s%N).msg"; echo "tick" > /tmp/loop-inbox/tmp/$m && mv /tmp/loop-inbox/tmp/$m /tmp/loop-inbox/new/$m) </dev/null >/dev/null 2>&1 &
   ```

## Pitfalls
- Never run `sleep` inside `msh` for long waits (`MSH_TIMEOUT_SEC` defaults to 120s).
- Always write to `tmp/` first and `mv` into `new/` so the reader never sees partial bytes.
- To stop the loop cleanly, send `SIGTERM` to the daemon PID (exits 143 and releases `flock`).
