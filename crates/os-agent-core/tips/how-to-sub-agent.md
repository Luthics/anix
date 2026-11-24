# How to Orchestrate Sub-Agents via POSIX Pipes

Invoke child `*.agent` scripts directly inside `msh` as ordinary executables using `-q` (`--stderr explicit`) so `fd 1` returns pure machine-consumable output.

## Pattern
- **Single sub-agent (clean stdout capture)**:
  ```sh
  result=$(./reviewer.agent -q 4< diff.patch -- "Return JSON findings to AGENT_OUT")
  ```
- **Pipeline composition**:
  ```sh
  git diff HEAD~1 | ./reviewer.agent -q | ./fixer.agent -q > fixed.patch
  ```
- **Parallel fan-out & join**:
  ```sh
  ./audit.agent -q -- "Audit auth/" > /tmp/a.out & p1=$!
  ./audit.agent -q -- "Audit net/"  > /tmp/b.out & p2=$!
  wait "$p1" "$p2"
  ```

## Pitfalls
- `agent` automatically strips `AGENT_CONTEXT` before spawning `msh`, so child agents never collide with the parent WAL `flock`. Pass `-c <separate.ctx.jsonl>` explicitly if a child needs multi-turn state.
- Always pass `-q` (or `-y` if non-interactive) when capturing a child agent's stdout in pipes; under `approval=ask` without `/dev/tty`, a child exits `42` (Fail-Closed).
- Attach `3> /tmp/child.trace.jsonl` when debugging nested runs; `ppid` in the trace envelope links parent and child automatically.
