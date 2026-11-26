#!/usr/bin/env bash
set -euo pipefail

# Build & install Anix v0.4.0 binaries and Layer 2 hooks following FHS layout:
#   ~/.local/bin/anix
#   ~/.local/libexec/anix/msh
#   ~/.local/libexec/anix/anix-approve
#   ~/.local/libexec/anix/anix-hook
#   ~/.local/libexec/anix/anix-provider
#   ~/.local/libexec/anix/anix-provider-{openai-responses,openai-compat,anthropic} -> anix-provider
#   ~/.local/libexec/anix/tools/{aview,aclr}
#   ~/.config/anix/hooks/pre-context.d/{20-skills,30-tips} -> ~/.local/libexec/anix/anix-hook

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROFILE="${1:-debug}"
PREFIX="${ANIX_PREFIX:-$HOME/.local}"
CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/anix"

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
install -m 0755 "${BIN_DIR}/anix-approve" "${PREFIX}/libexec/anix/anix-approve"
install -m 0755 "${BIN_DIR}/anix-hook" "${PREFIX}/libexec/anix/anix-hook"
install -m 0755 "${BIN_DIR}/anix-provider" "${PREFIX}/libexec/anix/anix-provider"
ln -sf anix-provider "${PREFIX}/libexec/anix/anix-provider-openai-responses"
ln -sf anix-provider "${PREFIX}/libexec/anix/anix-provider-openai-compat"
ln -sf anix-provider "${PREFIX}/libexec/anix/anix-provider-anthropic"
install -m 0755 "${BIN_DIR}/aview" "${PREFIX}/libexec/anix/tools/aview"
install -m 0755 "${BIN_DIR}/aclr" "${PREFIX}/libexec/anix/tools/aclr"

# Install default Layer 2 pre-context.d hooks (StrictModes-clean 0755, without clobbering masks/overrides).
install -d -m 0755 "${CONFIG_DIR}/hooks/pre-context.d"
for hook_name in 20-skills 30-tips; do
  if [[ ! -e "${CONFIG_DIR}/hooks/pre-context.d/${hook_name}" && ! -L "${CONFIG_DIR}/hooks/pre-context.d/${hook_name}" ]]; then
    ln -s "${PREFIX}/libexec/anix/anix-hook" "${CONFIG_DIR}/hooks/pre-context.d/${hook_name}"
  fi
done

echo "Installed Anix binaries into ${PREFIX}:"
ls -lh "${PREFIX}/bin/anix" \
       "${PREFIX}/libexec/anix/msh" \
       "${PREFIX}/libexec/anix/anix-approve" \
       "${PREFIX}/libexec/anix/anix-hook" \
       "${PREFIX}/libexec/anix/anix-provider" \
       "${PREFIX}/libexec/anix/anix-provider-openai-responses" \
       "${PREFIX}/libexec/anix/anix-provider-openai-compat" \
       "${PREFIX}/libexec/anix/anix-provider-anthropic" \
       "${PREFIX}/libexec/anix/tools/aview" \
       "${PREFIX}/libexec/anix/tools/aclr"
