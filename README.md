# Anix (POSIX-Native Agent Core & Runtime)

POSIX-native Agent kernel and reference runtime (`ir_version = 1`, Core spec `v0.4.0-rfc` in [docs/RFC.md](docs/RFC.md), Reference Runtime guide in [docs/RUNTIME.md](docs/RUNTIME.md)) built in Rust as independent static binaries with zero external runtime dependencies.

> **Core philosophy**: The kernel owns only three primitives — an **Agent file (`*.agent`)**, a **Tree WAL**, and a **Maildir**. Every wakeup is a message arriving in Maildir; every callback is `anix send`.

## Layered Architecture

Anix follows a strict, unidirectional 4-tier progression enforced at the **POSIX process & wire-protocol boundary**:

| Tier | Name | Scope & Repository | Role |
| :--- | :--- | :--- | :--- |
| **Layer 1** | **`anix-core`** | This repo ([docs/RFC.md](docs/RFC.md), Normative) | Minimal POSIX Agent kernel (`anix` + `anix-proto`): two-axis contract (`shell` + `output`), FD 0/1/2/4..9, Tree WAL (`--leaf`, `fork`, `fsck`, `log`), unified Maildir event loop (`--inbox`, `send [--wait]`, `--notify`, `hold` leases), POSIX `StrictModes`, and language-agnostic wire protocols. Zero dependencies on any shell, approval UI, LLM SDK, or TUI. |
| **Layer 2** | **`anix-runtime`** | This repo ([docs/RUNTIME.md](docs/RUNTIME.md), Informative) | Batteries-included reference distribution: `anix` + `msh` (with background reaper) + `anix-approve` (shell approval wrapper) + `a*` (`aview`, `aclr`) + `anix-provider` + standalone `pre-context.d/` hooks (`20-skills`, `30-tips`) + `install.sh`. Every non-core component is replaceable. |
| **Layer 3** | **`anix-agent`** | Separate repo (out of Core RFC) | High-level interactive shell / coding harness (Claude Code / Pi style: TUI, session tree browser, approval UI, diff viewer) driving `anix-runtime` strictly over RFC protocols. |
| **Layer 4** | **`anix-*`** | Separate repos (out of Core RFC) | Independent ecosystem packages: custom providers, policy engines, socket bridges, trace dashboards, domain agent/skill packs in any language. |

## Quick Install

Install the latest prebuilt release (`macOS` Apple Silicon / Intel, `Linux` x86_64 / arm64 static musl) into `~/.local`, along with a starter `~/.config/anix/config.toml`, `hooks/pre-context.d/{20-skills,30-tips}`, and `~/.config/anix/agents/bash.agent`:

```sh
# Public repo (direct curl)
curl -fsSL https://raw.githubusercontent.com/Luthics/anix/main/install.sh | sh

# Private repo (via authenticated GitHub CLI)
gh api repos/Luthics/anix/contents/install.sh -H "Accept: application/vnd.github.raw" | sh
```

Then configure your provider endpoint and API key in `~/.config/anix/config.toml` (or export `OPENAI_API_KEY`), verify with `anix --doctor`, and run:

```sh
anix --doctor
~/.config/anix/agents/bash.agent "show disk usage"
```

## Workspace Architecture

| Crate | Binary | Default FHS Install Path | Role |
| :--- | :--- | :--- | :--- |
| [`crates/anix-proto`](crates/anix-proto) | _(lib)_ | — | Single source of truth for Canonical IR v1 JSONL, Tree WAL events, Maildir envelope headers, Provider events, `$ANIX_CTL` frames, Trace envelope, and standard exit codes. |
| [`crates/anix-core`](crates/anix-core) | `anix` | `~/.local/bin/anix` | Pure Core kernel: CLI (`run`, `send`, `fork`, `fsck`, `log`, `--doctor`), 4-layer config, FD 0/1/2/4..9 & `--trace-fd`, Tree WAL + cross-file `fork`, unified Maildir loop (`--inbox`/`hold`/`--notify`/`--lock-wait`), POSIX `StrictModes`, signals, Landlock. |
| [`crates/msh`](crates/msh) | `msh` | `~/.local/libexec/anix/msh` | Model-oriented non-interactive shell wrapper: anti-hang env, binary circuit breaker, ANSI stripping, output folding, background reaper notifying `$ANIX_INBOX` via Maildir + `$ANIX_CTL` `hold` leases. |
| [`crates/anix-providers`](crates/anix-providers) | `anix-provider` (BusyBox symlinks: `anix-provider-{openai-responses,openai-compat,anthropic}`) | `~/.local/libexec/anix/anix-provider` | Unified Canonical IR v1 HTTP/SSE adapter supporting persistent `coprocess` and `oneshot` modes across OpenAI Responses, OpenAI-compatible Chat Completions, and Anthropic Messages. |
| [`crates/anix-tools`](crates/anix-tools) | `aview`, `aclr`, `anix-approve` | `~/.local/libexec/anix/{tools/,anix-approve}` | Layer 2 model tools (`aview`, `aclr`) and composable shell approval wrapper (`anix-approve -- msh`). |

## Development Commands

```sh
# Format, lint, and test
cargo fmt --all -- --check
cargo xcheck
cargo xclippy
cargo xtest

# Static Linux x86_64 release build (requires x86_64-unknown-linux-musl target)
cargo build --release --workspace --target x86_64-unknown-linux-musl

# Install locally built binaries & Layer 2 hooks into ~/.local and ~/.config/anix
./scripts/install-local.sh
```

## License

Licensed under the [Apache License, Version 2.0](LICENSE).
