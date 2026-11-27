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
- **Self-fork parallel exploration (inherit your current session context with zero WAL lock contention)**:
  `anix` exposes three read-only introspection variables to your shell: `$ANIX_WAL` (active WAL path), `$ANIX_LEAF` (latest closed, fork-safe node ID before your current tool call), and `$ANIX_AGENT` (your current `*.agent` script path):
  ```sh
  anix fork "$ANIX_WAL" --leaf "$ANIX_LEAF" -o /tmp/branch_a.wal.jsonl
  anix fork "$ANIX_WAL" --leaf "$ANIX_LEAF" -o /tmp/branch_b.wal.jsonl
  "$ANIX_AGENT" -q -c /tmp/branch_a.wal.jsonl "Explore approach A" > /tmp/a.out & p1=$!
  "$ANIX_AGENT" -q -c /tmp/branch_b.wal.jsonl "Explore approach B" > /tmp/b.out & p2=$!
  wait "$p1" "$p2"
  ```
  Or asynchronously notify your persistent `--inbox` when a forked branch finishes:
  ```sh
  anix fork "$ANIX_WAL" --leaf "$ANIX_LEAF" -o /tmp/audit.wal.jsonl
  "$ANIX_AGENT" -q -c /tmp/audit.wal.jsonl --notify "$ANIX_INBOX" --notify-id "audit-1" \
    "Audit auth/ and summarize findings" >/tmp/audit.out 2>&1 &
  ```
- **Persistent worker RPC via `anix send --wait`**:
  ```sh
  reply=$(anix send /tmp/worker-inbox --wait --timeout 60 "Run tests and report failures")
  ```
- **Async specialized child callback into parent `--inbox`**:
  ```sh
  ./audit.agent -q --notify "$ANIX_INBOX" --notify-id "audit-1" "Audit auth/" >/tmp/audit.out 2>&1 &
  ```

## Pitfalls
- `anix` clears all inherited `ANIX_*` env vars before spawning child shells (preserving only `ANIX_LOG`/`ANIX_TRACE_LEVEL` and actively injecting `$ANIX_INBOX`, `$ANIX_WAL`, `$ANIX_LEAF`, and `$ANIX_AGENT`). Child `anix` processes **never** read those variables implicitly as their own CLI/config flags — pass `-c` / `--notify` explicitly.
- Always pass `--leaf "$ANIX_LEAF"` (not raw EOF) when forking your own active WAL mid-turn so the child starts from a cleanly closed turn.
- Pass `-c <separate.ctx.jsonl>` (or fork via `anix fork`) when children need independent multi-turn state.
- Attach `--trace-fd=3 3> /tmp/child.trace.jsonl` when debugging nested runs; `ppid` in the trace envelope links parent and child automatically.
