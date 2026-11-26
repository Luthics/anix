# Anix (POSIX-Native Agent Core & Runtime)

POSIX-native Agent kernel and reference runtime (`ir_version = 1`, Core spec `v0.3.0-rfc` in [docs/RFC.md](docs/RFC.md), Reference Runtime guide in [docs/RUNTIME.md](docs/RUNTIME.md)) built in Rust as independent static binaries with zero external runtime dependencies.

## Layered Architecture

Anix follows a strict, unidirectional 4-tier progression inspired by Unix and Pi's radical minimalism — enforced at the **POSIX process & wire-protocol boundary**:

| Tier | Name | Scope & Repository | Role |
| :--- | :--- | :--- | :--- |
| **Layer 1** | **`anix-core`** | This repo ([docs/RFC.md](docs/RFC.md), Normative) | Minimal POSIX Agent kernel (`anix` + `anix-proto`): CLI/frontmatter, FD 0/1/2/4..9 & 3-axis output contract, WAL, event-driven signals, and language-agnostic external plugin wire protocols. Zero dependencies on any shell, LLM SDK, or TUI. |
| **Layer 2** | **`anix-runtime`** | This repo ([docs/RUNTIME.md](docs/RUNTIME.md), Informative) | Batteries-included reference distribution: `anix` + `msh` + `a*` (`aview`, `aclr`) + `anix-provider` + built-in hooks (`20-skills`, `30-tips`) + `install.sh`. Every non-core component is replaceable. |
| **Layer 3** | **`anix-agent`** | Separate repo (out of Core RFC) | High-level interactive shell / coding harness (Claude Code / Pi style: TUI, session tree, approval UI, diff viewer, per-turn dynamic context) driving `anix-runtime` strictly over RFC protocols. |
| **Layer 4** | **`anix-*`** | Separate repos (out of Core RFC) | Independent ecosystem packages: custom providers, approval engines, vision bridges, trace dashboards, domain agent/skill packs in any language. |

## Quick Install

Install the latest prebuilt release (`macOS` Apple Silicon / Intel, `Linux` x86_64 / arm64 static musl) into `~/.local`, along with a starter `~/.config/anix/config.toml` and `~/.config/anix/agents/bash.agent`:

```sh
# Public repo (direct curl)
curl -fsSL https://raw.githubusercontent.com/Luthics/anix/main/install.sh | sh

# Private repo (via authenticated GitHub CLI)
gh api repos/Luthics/anix/contents/install.sh -H "Accept: application/vnd.github.raw" | sh
```

Then configure your provider endpoint and API key in `~/.config/anix/config.toml` (or export `OPENAI_API_KEY`), verify with `anix --doctor`, and run the default script:

```sh
anix --doctor
~/.config/anix/agents/bash.agent "show disk usage"
```

## Workspace Architecture

| Crate | Binary | Default FHS Install Path | Role |
| :--- | :--- | :--- | :--- |
| [`crates/anix-proto`](crates/anix-proto) | _(lib)_ | — | Single source of truth for Canonical IR v1 JSONL, WAL events, Provider events, `$ANIX_CTL` frames, Trace envelope, and standard exit codes. |
| [`crates/anix-core`](crates/anix-core) | `anix` | `~/.local/bin/anix` | Core interpreter: CLI/frontmatter/config resolution, FD 0/1/2/4..9 & `--trace-fd` & `--inbox`, WAL & reverse-read replay, `/dev/tty` + external `[approval].bin`, `[vision].bin`, hooks + `anix trust`, signals, zero-poll `pidfd`/`epoll`/`inotify`/`kqueue`, Landlock. |
| [`crates/msh`](crates/msh) | `msh` | `~/.local/libexec/anix/msh` | Model-oriented non-interactive shell wrapper: anti-hang env, binary circuit breaker, ANSI stripping, output folding, command timeout, isolated process groups. |
| [`crates/anix-providers`](crates/anix-providers) | `anix-provider` (BusyBox symlinks: `anix-provider-{openai-responses,openai-compat,anthropic}`) | `~/.local/libexec/anix/anix-provider` | Unified Canonical IR v1 HTTP/SSE adapter supporting persistent `coprocess` and `oneshot` modes across OpenAI Responses, OpenAI-compatible Chat Completions, and Anthropic Messages. |
| [`crates/anix-tools`](crates/anix-tools) | `aview`, `aclr` | `~/.local/libexec/anix/tools/` | Explicit model-facing tools communicating via `$ANIX_CTL` (`fd 6`). |

## Development Commands

```sh
# Format, lint, and test
cargo fmt --all -- --check
cargo xcheck
cargo xclippy
cargo xtest

# Static Linux x86_64 release build (requires x86_64-unknown-linux-musl target)
cargo build --release --workspace --target x86_64-unknown-linux-musl

# Install locally built binaries into ~/.local/{bin,libexec/anix}
./scripts/install-local.sh
```

## License

Licensed under the [Apache License, Version 2.0](LICENSE).
