#!/usr/bin/env bash
set -euo pipefail

# Build & install all v1.0 binaries following Section 2 FHS layout:
#   ~/.local/bin/agent
#   ~/.local/libexec/agent/msh
#   ~/.local/libexec/agent/agent-provider
#   ~/.local/libexec/agent/agent-provider-{openai-responses,openai-compat,anthropic} -> agent-provider
#   ~/.local/libexec/agent/tools/{aview,aclr}

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROFILE="${1:-debug}"
PREFIX="${OS_AGENT_PREFIX:-$HOME/.local}"

export PATH="$HOME/.cargo/bin:$PATH"
cd "${ROOT_DIR}"

if [[ "${PROFILE}" == "--release" || "${PROFILE}" == "release" ]]; then
  cargo build --release --workspace
  BIN_DIR="${ROOT_DIR}/target/release"
else
  cargo build --workspace
  BIN_DIR="${ROOT_DIR}/target/debug"
fi

install -d -m 0755 "${PREFIX}/bin" "${PREFIX}/libexec/agent/tools"
install -m 0755 "${BIN_DIR}/agent" "${PREFIX}/bin/agent"
install -m 0755 "${BIN_DIR}/msh" "${PREFIX}/libexec/agent/msh"
install -m 0755 "${BIN_DIR}/agent-provider" "${PREFIX}/libexec/agent/agent-provider"
ln -sf agent-provider "${PREFIX}/libexec/agent/agent-provider-openai-responses"
ln -sf agent-provider "${PREFIX}/libexec/agent/agent-provider-openai-compat"
ln -sf agent-provider "${PREFIX}/libexec/agent/agent-provider-anthropic"
install -m 0755 "${BIN_DIR}/aview" "${PREFIX}/libexec/agent/tools/aview"
install -m 0755 "${BIN_DIR}/aclr" "${PREFIX}/libexec/agent/tools/aclr"

echo "Installed OS Agent binaries into ${PREFIX}:"
ls -lh "${PREFIX}/bin/agent" \
       "${PREFIX}/libexec/agent/msh" \
       "${PREFIX}/libexec/agent/agent-provider" \
       "${PREFIX}/libexec/agent/agent-provider-openai-responses" \
       "${PREFIX}/libexec/agent/agent-provider-openai-compat" \
       "${PREFIX}/libexec/agent/agent-provider-anthropic" \
       "${PREFIX}/libexec/agent/tools/aview" \
       "${PREFIX}/libexec/agent/tools/aclr"
