#!/usr/bin/env bash
set -euo pipefail

# Build & install all Anix v1.1 binaries following Section 2 FHS layout:
#   ~/.local/bin/anix
#   ~/.local/libexec/anix/msh
#   ~/.local/libexec/anix/anix-provider
#   ~/.local/libexec/anix/anix-provider-{openai-responses,openai-compat,anthropic} -> anix-provider
#   ~/.local/libexec/anix/tools/{aview,aclr}

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROFILE="${1:-debug}"
PREFIX="${ANIX_PREFIX:-$HOME/.local}"

export PATH="$HOME/.cargo/bin:$PATH"
cd "${ROOT_DIR}"

if [[ "${PROFILE}" == "--release" || "${PROFILE}" == "release" ]]; then
  cargo build --release --workspace
  BIN_DIR="${ROOT_DIR}/target/release"
else
  cargo build --workspace
  BIN_DIR="${ROOT_DIR}/target/debug"
fi

install -d -m 0755 "${PREFIX}/bin" "${PREFIX}/libexec/anix/tools"
install -m 0755 "${BIN_DIR}/anix" "${PREFIX}/bin/anix"
install -m 0755 "${BIN_DIR}/msh" "${PREFIX}/libexec/anix/msh"
install -m 0755 "${BIN_DIR}/anix-provider" "${PREFIX}/libexec/anix/anix-provider"
ln -sf anix-provider "${PREFIX}/libexec/anix/anix-provider-openai-responses"
ln -sf anix-provider "${PREFIX}/libexec/anix/anix-provider-openai-compat"
ln -sf anix-provider "${PREFIX}/libexec/anix/anix-provider-anthropic"
install -m 0755 "${BIN_DIR}/aview" "${PREFIX}/libexec/anix/tools/aview"
install -m 0755 "${BIN_DIR}/aclr" "${PREFIX}/libexec/anix/tools/aclr"

echo "Installed Anix binaries into ${PREFIX}:"
ls -lh "${PREFIX}/bin/anix" \
       "${PREFIX}/libexec/anix/msh" \
       "${PREFIX}/libexec/anix/anix-provider" \
       "${PREFIX}/libexec/anix/anix-provider-openai-responses" \
       "${PREFIX}/libexec/anix/anix-provider-openai-compat" \
       "${PREFIX}/libexec/anix/anix-provider-anthropic" \
       "${PREFIX}/libexec/anix/tools/aview" \
       "${PREFIX}/libexec/anix/tools/aclr"
