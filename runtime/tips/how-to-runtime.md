# How Your Anix Runtime Environment Works

You are running as a standard POSIX process (`anix` executing a `*.agent` script). You may be invoked interactively by a human, inside a shell pipeline (`cmd | you | jq`), as a child of another agent, or as a persistent `--inbox` daemon.

## 1. Inputs & Output Contracts (POSIX FDs)
- **How inputs arrive**: CLI positional args and `fd 0` (`stdin`, read to EOF) arrive as plain text (separated by a blank line when both are present), followed by any extra readable FDs (`fd 4..=9`) wrapped in `<input fd="N">...</input>`. In `--inbox` mode, later tasks arrive as `<input source="<name>" ...>...</input>` (burst arrivals are coalesced into one turn batch).
- **How `fd 1` (`stdout`) is governed** (statically fixed at startup; obey your `<posix_runtime>` block):
  - `output=reply` (default): only your **final** reply (after all tool calls finish) goes to `fd 1`; intermediate prose during tool turns goes to `fd 2` (if `stderr=auto`). `$ANIX_OUT` is NOT mounted.
  - `output=explicit`: ONLY raw bytes redirected via your shell tool to `$ANIX_OUT` reach `fd 1`; normal text goes to `fd 2` (or is hidden from humans when `stderr=explicit`).
  - `output=none`: `fd 1` is strictly silent (0 bytes); deliver results via side effects (e.g. file edits) and human notes via `fd 2` / `$ANIX_ERR`.
- **Write-only side channels**: `$ANIX_OUT` (only under `output=explicit`), `$ANIX_ERR`, and `$ANIX_CTL` are write-only anonymous pipes — always use the environment variable paths (never hardcode FD numbers), and never read or inspect them.

## 2. How Your Shell Executor (`msh` / Wrapper Chains) Works
- **Strict POSIX `/bin/sh`**: default `msh` executes commands via `/bin/sh -c` (often `dash` on Linux), NOT Bash. Avoid Bash-only syntax (`{a,b}`, `[[ ]]`, arrays, `<(...)`) or invoke `bash -c` explicitly when needed.
- **Non-interactive isolation**: every call runs in an isolated process group with `stdin` on `/dev/null` and anti-hang env (`PAGER=cat`, `NO_COLOR=1`, `GIT_TERMINAL_PROMPT=0`, `DEBIAN_FRONTEND=noninteractive`).
- **Output folding & binary circuit breaker**: outputs exceeding `200` lines or `8192` bytes (`MSH_MAX_LINES` / `MSH_MAX_BYTES`) are folded to the first 20 + last 20 lines with the full text saved to `/tmp/msh_<uuid>.log`. Binary output is suppressed to a temp file (inspect images/PDFs with `aview` when your provider supports vision, or text tools otherwise). Prefer targeted reads (`rg`, `head`, `tail`, `sed -n`, `jq`) over dumping huge files at once.
- **Timeouts, `hold` leases & Maildir wakeup**: exceeding `timeout_sec` yields to background by default, emits a `hold` lease on `$ANIX_CTL` (preventing `--idle-timeout` exit), and in `--inbox` mode automatically delivers a completion message back to `$ANIX_INBOX` when finished (see `how-to-background.md`).
- **Env isolation & approval wrappers**: `anix` clears `ANIX_*` vars before spawning the shell (preserving only `ANIX_LOG`/`ANIX_TRACE_LEVEL` and actively injecting `$ANIX_OUT`/`$ANIX_ERR`/`$ANIX_CTL`, `$ANIX_TOOL_ARGS`, `$ANIX_WAL`/`$ANIX_LEAF`/`$ANIX_AGENT`, and `$ANIX_INBOX` when running in `--inbox` mode). If your agent is wrapped with `anix-approve`, disallowed commands return an error tool result or abort with exit `75`.

## 3. Context Lifecycle, Tree WAL, Parallel `fork`, Folding & Compaction
- **Frozen prefix (KV-Cache)**: your initial `system` message (`turn_id: 0`) is frozen on turn 0 so the provider can hit prompt cache across turns. Runtime notices append at the tail as `<runtime_update>` without invalidating the prefix.
- **Tree WAL (`-c`, `--leaf`, `anix fork`, `anix fsck`, `anix log`)**:
  - Without `-c`, conversation state lives only in process memory.
  - With `-c <file>`, turns are appended to a Tree WAL (`id`/`parent_id`) under exclusive `flock` (supports `--lock-wait[=<SEC>]`).
  - Use `anix fork <src.jsonl> [--leaf <id>] -o <dst.jsonl>` to spawn independent parallel branch files that read `<src.jsonl>` read-only with zero lock contention.
  - Inspect or verify WAL files anytime with `anix log <wal> [--leaf <id>]` and `anix fsck <wal>`.
- **Turn anchors (`[t<N>]`) & Tail Folding (`aclr -t`)**:
  - Each tool result starts with `[t<turn_id>] exit=<code>`.
  - Fold dead-end exploration tails without breaking your frozen KV-cache prefix: `aclr -t <turn_id> "Checked X; Y failed"` (explicit) or `aclr -t <turn_id>` (auto-summary).
- **Full Compaction (`aclr -s` / auto-threshold / `SIGUSR1` / `Anix-Control: compact`)**:
  - Summarizes the active window into a `<checkpoint>` section and refreshes the frozen system prefix.
- **Live Progress Checklist (`aplan`)**:
  - For multi-step tasks (3+ steps), emit live UI progress over `$ANIX_CTL` without adding WAL bloat using Markdown checkboxes on stdin (`aplan <<'EOF'\n- [x] Done\n- [~] Working\n- [ ] Next\nEOF`) or flags (`aplan -d "Done" -i "Working" -p "Next"` / `aplan clear`).

## 4. Budgets & Self-Discipline
- You run under hard `max_turns` and `max_tokens` limits (hitting either exits `124`).
- Read selectively, fold spent exploration tails with `aclr -t`, stop temporary background servers when done, and match your output channel to `<posix_runtime>`.
