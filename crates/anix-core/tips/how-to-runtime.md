# How Your Anix Runtime Environment Works

You are running as a standard POSIX process (`anix` executing a `*.agent` script). You may be invoked interactively by a human, inside a shell pipeline (`cmd | you | jq`), as a child of another agent, or as a persistent `--inbox` daemon.

## 1. Inputs & Output Contracts (POSIX FDs)
- **How inputs arrive**: CLI positional args are wrapped in `<args>...</args>`, followed by `fd 0` (`stdin`, read to EOF), followed by any extra readable FDs (`fd 4..=9`) wrapped in `<input fd="N">...</input>`. In `--inbox` mode, later tasks arrive serially as `<input source="<name>">...</input>`.
- **How `fd 1` (`stdout`) is governed** (statically fixed at startup; obey your `<posix_runtime>` block):
  - `output=reply` (default): only your **final** reply (after all tool calls finish) goes to `fd 1`; intermediate prose during tool turns goes to `fd 2` (if `stderr=auto`). `$ANIX_OUT` is NOT mounted.
  - `output=explicit`: ONLY raw bytes redirected via `msh` to `$ANIX_OUT` (`fd 4`) reach `fd 1`; normal text goes to `fd 2` (or is hidden from humans when `stderr=explicit`).
  - `output=none`: `fd 1` is strictly silent (0 bytes); deliver results via side effects (e.g. file edits) and human notes via `fd 2` / `$ANIX_ERR`.
- **Write-only side channels**: `$ANIX_OUT` (`fd 4`, only under `output=explicit`), `$ANIX_ERR` (`fd 5`), and `$ANIX_CTL` (`fd 6`) are write-only anonymous pipes — redirect into them, never read or inspect them.

## 2. How `msh` Runs Your Shell Commands
- **Strict POSIX `/bin/sh`**: `msh` executes commands via `/bin/sh -c` (often `dash` on Linux), NOT Bash. Avoid Bash-only syntax (`{a,b}`, `[[ ]]`, arrays, `<(...)`) or invoke `bash -c` explicitly when needed.
- **Non-interactive isolation**: every call runs in an isolated process group with `stdin` on `/dev/null` and anti-hang env (`PAGER=cat`, `NO_COLOR=1`, `GIT_TERMINAL_PROMPT=0`, `DEBIAN_FRONTEND=noninteractive`).
- **Output folding & binary circuit breaker**: outputs exceeding `200` lines or `8192` bytes (`MSH_MAX_LINES` / `MSH_MAX_BYTES`) are folded to the first 20 + last 20 lines with the full text saved to `/tmp/msh_<uuid>.log`. Binary output is suppressed to a temp file (inspect images/PDFs with `aview` if `vision=true`, or text tools if `vision=false`). Prefer targeted reads (`rg`, `head`, `tail`, `sed -n`, `jq`) over dumping huge files at once.
- **Timeouts & backgrounding**: exceeding `timeout_sec` yields to background by default (without killing) and later notifies you via `<runtime_update>` when finished (see `how-to-background.md`).
- **Env isolation & approval**: `anix` clears `ANIX_*` vars before spawning `msh` and injects `ANIX_APPROVAL=<ask|auto|deny>`. Under `approval=ask` without `/dev/tty`, unapproved commands fail closed (exit `75`); under `approval=deny`, non-whitelisted commands return an error tool result.

## 3. Context Lifecycle, WAL, Folding & Compaction
- **Frozen prefix (KV-Cache)**: your initial `system` message (`turn_id: 0`, including `<posix_runtime>`, `<available_skills>`, `<runtime_tips>`) is frozen on turn 0 so the provider can hit prompt cache across turns. Runtime notifications (config drift across restarts, finished background jobs) append at the tail as `<runtime_update>` without invalidating the prefix.
- **Ephemeral vs Tree WAL (`-c` / `--leaf`)**:
  - Without `-c`, your conversation lives only in process memory (destroyed on exit; kept across turns while an `--inbox` daemon remains running).
  - With `-c <file>`, turns are appended to a single-file Tree WAL (`id`/`parent_id`) under an exclusive `flock`. Callers or an outer harness can resume the latest leaf or branch from any earlier closed node via `--leaf <id>`.
- **Turn anchors (`[t<N>]`) & Tail Folding (`aclr -t`)**:
  - Each `msh` result starts with `[t<turn_id>] exit=<code>`.
  - After noisy exploration or dead-end attempts, fold the tail to reclaim context **without breaking your frozen KV-cache prefix**:
    - Explicit summary (zero extra LLM call, preferred when facts are clear): `aclr -t <turn_id> "Checked X; Y failed because Z"`
    - Auto-summary (invokes compactor on only the folded tail): `aclr -t <turn_id>`
  - `anix` aligns `<turn_id>` upward to keep tool-call pairs closed and replaces the folded tail with `<folded turns="...">...</folded>` (branching from the ancestor before `<turn_id>`).
- **Full Compaction (`aclr -s` / auto-threshold / `SIGUSR1`)**:
  - When active tokens reach `compact_threshold` (default 80% of `max_tokens`), or when you run `aclr -s`, `anix` summarizes the whole active window into a `<checkpoint>` section and refreshes the frozen system prefix. Reserve `aclr -s` for major phase boundaries because it resets the prefix cache.

## 4. Budgets & Self-Discipline
- You run under hard `max_turns` and `max_tokens` limits (hitting either exits `124`).
- Read selectively, fold spent exploration tails with `aclr -t`, stop temporary background servers when done, and match your output channel to `<posix_runtime>` so upstream/downstream pipes compose cleanly.
