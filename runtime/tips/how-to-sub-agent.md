# How to Orchestrate Sub-Agents via POSIX Pipes, Maildir & `anix fork`

Invoke child `*.agent` scripts directly as ordinary POSIX executables using `-q` (`--stderr explicit`) so `fd 1` returns pure machine-consumable output.

By default (`output: reply`), a child agent's final reply goes directly to `fd 1` (`stdout`) — pure-text filters with `shell: none` stream in real time with zero shell overhead. Pass `-o explicit` when a sub-agent must write binary or large files directly to `$ANIX_OUT`, or `-o none` for silent side-effect-only agents.

## Patterns
- **Single sub-agent (clean stdout capture)**:
  ```sh
  result=$(./reviewer.agent -q 4< diff.patch -- "Return JSON findings")
  ```
- **Pipeline composition**:
  ```sh
  git diff HEAD~1 | ./reviewer.agent -q | ./translator.agent -q > review_en.md
  ```
- **Parallel branch exploration via `anix fork` (zero WAL lock contention)**:
  ```sh
  anix fork /tmp/main.ctx.jsonl --leaf 6 -o /tmp/branch_a.ctx.jsonl
  anix fork /tmp/main.ctx.jsonl --leaf 6 -o /tmp/branch_b.ctx.jsonl
  ./coder.agent -q -c /tmp/branch_a.ctx.jsonl "Try approach A" > /tmp/a.out & p1=$!
  ./coder.agent -q -c /tmp/branch_b.ctx.jsonl "Try approach B" > /tmp/b.out & p2=$!
  wait "$p1" "$p2"
  ```
- **Persistent worker RPC via `anix send --wait`**:
  ```sh
  reply=$(anix send /tmp/worker-inbox --wait --timeout 60 "Run tests and report failures")
  ```
- **Async child callback into parent `--inbox`**:
  ```sh
  ./audit.agent -q --notify "$ANIX_INBOX" --notify-id "audit-1" "Audit auth/" >/tmp/audit.out 2>&1 &
  ```

## Pitfalls
- `anix` clears all `ANIX_*` env vars before spawning child shells (preserving only `ANIX_LOG`/`ANIX_TRACE_LEVEL` and actively injecting `$ANIX_INBOX` when the parent runs in `--inbox` mode). Child `anix` processes **never** read `$ANIX_INBOX` as their own `--inbox` config.
- Pass `-c <separate.ctx.jsonl>` (or fork via `anix fork`) when children need independent multi-turn state.
- Attach `--trace-fd=3 3> /tmp/child.trace.jsonl` when debugging nested runs; `ppid` in the trace envelope links parent and child automatically.
