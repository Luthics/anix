# OS Agent (POSIX-Native Agent Runtime)

POSIX-native Agent runtime (`ir_version = 1`) built in Rust as independent static binaries with zero external runtime dependencies.

## Quick Install

Install the latest prebuilt release (`macOS` Apple Silicon / Intel, `Linux` x86_64 / arm64 static musl) into `~/.local`:

```sh
# Public repo (direct curl)
curl -fsSL https://raw.githubusercontent.com/Luthics/os-agent/main/install.sh | sh

# Private repo (via authenticated GitHub CLI)
gh api repos/Luthics/os-agent/contents/install.sh -H "Accept: application/vnd.github.raw" | sh
```

To also install the companion Zsh plugin ([`Luthics/zsh-os-agent`](https://github.com/Luthics/zsh-os-agent)), append `-s -- --with-zsh`:

```sh
curl -fsSL https://raw.githubusercontent.com/Luthics/os-agent/main/install.sh | sh -s -- --with-zsh
```

Then configure your provider endpoint and API key in `~/.config/agent/config.toml` (or export `OPENAI_API_KEY`), and run:

```sh
agent --doctor
```

## Workspace Architecture

| Crate | Binary | Default FHS Install Path | Role |
| :--- | :--- | :--- | :--- |
| [`crates/os-agent-proto`](crates/os-agent-proto) | _(lib)_ | — | Single source of truth for Canonical IR v1 JSONL, WAL events, Provider events, `$AGENT_CTL` frames, Trace envelope, and standard exit codes. |
| [`crates/os-agent-core`](crates/os-agent-core) | `agent` | `~/.local/bin/agent` | Core interpreter: CLI/frontmatter/config resolution, FD 0/1/2/3/4..9 & `--inbox`, WAL & reverse-read replay, `/dev/tty` approval, hooks + `agent trust`, signals, Linux `pidfd`/Landlock. |
| [`crates/msh`](crates/msh) | `msh` | `~/.local/libexec/agent/msh` | Model-oriented non-interactive shell wrapper: anti-hang env, binary circuit breaker, ANSI stripping, output folding, command timeout, isolated process groups. |
| [`crates/provider-openai-responses`](crates/provider-openai-responses) | `agent-provider-openai-responses` | `~/.local/libexec/agent/agent-provider-openai-responses` | Stateless Canonical IR v1 ↔ OpenAI Responses API streaming adapter. |
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
