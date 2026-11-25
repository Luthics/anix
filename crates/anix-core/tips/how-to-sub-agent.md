# How to Orchestrate Sub-Agents via POSIX Pipes

Invoke child `*.agent` scripts directly inside `msh` as ordinary executables using `-q` (`--stderr explicit`) so `fd 1` returns pure machine-consumable output.

By default (`output: reply`), a child agent's final reply is delivered directly to `fd 1` (`stdout`) — pure-text filters with `shell: none` stream in real time with zero shell overhead. Pass `-o explicit` (or set `output: explicit` in frontmatter) when a sub-agent must stream binary or large files directly to `$ANIX_OUT`, or `-o none` for silent side-effect-only agents.

## Pattern
- **Single sub-agent (clean stdout capture)**:
  ```sh
  result=$(./reviewer.agent -q 4< diff.patch -- "Return JSON findings")
  ```
- **Pipeline composition**:
  ```sh
  git diff HEAD~1 | ./reviewer.agent -q | ./translator.agent -q > review_en.md
  ```
- **Parallel fan-out & join**:
  ```sh
  ./audit.agent -q -- "Audit auth/" > /tmp/a.out & p1=$!
  ./audit.agent -q -- "Audit net/"  > /tmp/b.out & p2=$!
  wait "$p1" "$p2"
  ```

## Pitfalls
- `anix` strips all `ANIX_*` env vars before spawning `msh` (preserving only `ANIX_LOG`/`ANIX_TRACE_LEVEL` and injecting the parent's resolved `ANIX_APPROVAL`), so child agents use their own Frontmatter and never collide with the parent WAL `flock`. Pass `-c <separate.ctx.jsonl>` explicitly if a child needs multi-turn state.
- Always pass `-q` when capturing a child agent's stdout in pipes; under `approval=ask` without `/dev/tty`, a shell-enabled child exits `75` (Fail-Closed). Pure-text children (`shell: none`) never spawn a shell and ignore `approval`.
- Attach `--trace-fd=3 3> /tmp/child.trace.jsonl` when debugging nested runs; `ppid` in the trace envelope links parent and child automatically.
