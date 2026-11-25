# How to Orchestrate Sub-Agents via POSIX Pipes

Invoke child `*.anix` scripts directly inside `msh` as ordinary executables using `-q` (`--stderr explicit`) so `fd 1` returns pure machine-consumable output.

## Pattern
- **Single sub-agent (clean stdout capture)**:
  ```sh
  result=$(./reviewer.anix -q 4< diff.patch -- "Return JSON findings to ANIX_OUT")
  ```
- **Pipeline composition**:
  ```sh
  git diff HEAD~1 | ./reviewer.anix -q | ./fixer.anix -q > fixed.patch
  ```
- **Parallel fan-out & join**:
  ```sh
  ./audit.anix -q -- "Audit auth/" > /tmp/a.out & p1=$!
  ./audit.anix -q -- "Audit net/"  > /tmp/b.out & p2=$!
  wait "$p1" "$p2"
  ```

## Pitfalls
- `anix` strips all `ANIX_*` env vars before spawning `msh` (preserving only `ANIX_LOG`/`ANIX_TRACE_LEVEL` and injecting the parent's resolved `ANIX_APPROVAL`), so child agents use their own Frontmatter and never collide with the parent WAL `flock`. Pass `-c <separate.ctx.jsonl>` explicitly if a child needs multi-turn state.
- Always pass `-q` when capturing a child agent's stdout in pipes; under `approval=ask` without `/dev/tty`, a child exits `75` (Fail-Closed).
- Attach `--trace-fd=3 3> /tmp/child.trace.jsonl` when debugging nested runs; `ppid` in the trace envelope links parent and child automatically.
