# OS Agent (POSIX-Native Agent Runtime)

POSIX-native Agent runtime (`ir_version = 1`) built in Rust as independent static binaries with zero external runtime dependencies.

## Quick Install

Install the latest prebuilt release (`macOS` Apple Silicon / Intel, `Linux` x86_64 / arm64 static musl) into `~/.local`, along with a starter `~/.config/agent/config.toml` and `~/.config/agent/agents/bash.agent`:

```sh
# Public repo (direct curl)
curl -fsSL https://raw.githubusercontent.com/Luthics/os-agent/main/install.sh | sh

# Private repo (via authenticated GitHub CLI)
gh api repos/Luthics/os-agent/contents/install.sh -H "Accept: application/vnd.github.raw" | sh
```

Then configure your provider endpoint and API key in `~/.config/agent/config.toml` (or export `OPENAI_API_KEY`), verify with `agent --doctor`, and run the default script:

```sh
agent --doctor
~/.config/agent/agents/bash.agent "show disk usage"
```

## Workspace Architecture

| Crate | Binary | Default FHS Install Path | Role |
| :--- | :--- | :--- | :--- |
| [`crates/os-agent-proto`](crates/os-agent-proto) | _(lib)_ | — | Single source of truth for Canonical IR v1 JSONL, WAL events, Provider events, `$AGENT_CTL` frames, Trace envelope, and standard exit codes. |
| [`crates/os-agent-core`](crates/os-agent-core) | `agent` | `~/.local/bin/agent` | Core interpreter: CLI/frontmatter/config resolution, FD 0/1/2/3/4..9 & `--inbox`, WAL & reverse-read replay, `/dev/tty` approval, hooks + `agent trust`, signals, Linux `pidfd`/Landlock. |
| [`crates/msh`](crates/msh) | `msh` | `~/.local/libexec/agent/msh` | Model-oriented non-interactive shell wrapper: anti-hang env, binary circuit breaker, ANSI stripping, output folding, command timeout, isolated process groups. |
| [`crates/agent-providers`](crates/agent-providers) | `agent-provider` (BusyBox symlinks: `agent-provider-{openai-responses,openai-compat,anthropic}`) | `~/.local/libexec/agent/agent-provider` | Unified stateless Canonical IR v1 HTTP/SSE adapter supporting OpenAI Responses, OpenAI-compatible Chat Completions, and Anthropic Messages. |
| [`crates/agent-tools`](crates/agent-tools) | `aview`, `aclr` | `~/.local/libexec/agent/tools/` | Explicit model-facing tools communicating via `$AGENT_CTL` (`fd 6`). |

## Development Commands

```sh
# Format, lint, and test
cargo fmt --all -- --check
cargo xcheck
cargo xclippy
cargo xtest

# Static Linux x86_64 release build (requires x86_64-unknown-linux-musl target)
cargo build --release --workspace --target x86_64-unknown-linux-musl

# Install locally built binaries into ~/.local/{bin,libexec/agent}
./scripts/install-local.sh
```

## License

Licensed under the [Apache License, Version 2.0](LICENSE).
