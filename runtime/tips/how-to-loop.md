# How to Loop, Schedule & Communicate via Maildir (`--inbox` / `anix send`)

Use a persistent `--inbox <DIR>` daemon and `anix send` instead of keeping shell commands blocked in `sleep` loops.

## Pattern
1. Start a persistent worker holding a WAL context:
   ```sh
   ./worker.agent -c /tmp/loop.ctx.jsonl --inbox /tmp/loop-inbox --idle-timeout 0
   ```
2. Deliver an asynchronous message atomically (`tmp/` -> `fsync` -> `new/`):
   ```sh
   anix send /tmp/loop-inbox --from scheduler "Check deploy health"
   ```
3. Perform a synchronous request/reply (creates a temporary `Anix-Reply-To` Maildir, blocks until reply, prints reply to `stdout`):
   ```sh
   answer=$(anix send /tmp/loop-inbox --wait --timeout 60 "Summarize current status")
   ```
4. Trigger out-of-band context compaction via Maildir (equivalent to `SIGUSR1`):
   ```sh
   anix send /tmp/loop-inbox --wait --control compact
   ```

## Pitfalls
- Never write directly into `<inbox>/new/` with `>` — always use `anix send` (or write to `tmp/` and `mv` into `new/`).
- Concurrent messages arriving within `--inbox-debounce-ms` (default 100ms) are coalesced into a single turn batch.
- `--idle-timeout` only counts down when `new/` is empty AND zero active `hold` leases remain.
- To stop the daemon cleanly, send `SIGTERM` (exits 143 and releases `flock`).
