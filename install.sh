#!/usr/bin/env sh
# One-line installer for Anix (https://github.com/Luthics/anix)
# Usage (public repo):
#   curl -fsSL https://raw.githubusercontent.com/Luthics/anix/main/install.sh | sh
#
# Usage (private repo, using GitHub CLI or GITHUB_TOKEN/GH_TOKEN):
#   gh api repos/Luthics/anix/contents/install.sh -H "Accept: application/vnd.github.raw" | sh
#
# Environment overrides:
#   ANIX_VERSION=v0.0.8           Pin a specific release tag (default: latest release)
#   ANIX_PREFIX=$HOME/.local      Install prefix (default: ~/.local)
#   ANIX_REPO=Luthics/anix
#   GITHUB_TOKEN / GH_TOKEN       Optional GitHub token (needed if repo is private and `gh` is absent)
set -eu

REPO="${ANIX_REPO:-Luthics/anix}"
PREFIX="${ANIX_PREFIX:-$HOME/.local}"
CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/anix"
AGENTS_DIR="${CONFIG_DIR}/agents"
VERSION="${ANIX_VERSION:-}"
TOKEN="${GITHUB_TOKEN:-${GH_TOKEN:-}}"

while [ "$#" -gt 0 ]; do
  case "$1" in
    --version)
      shift
      [ "$#" -gt 0 ] || { echo "install.sh: --version requires an argument" >&2; exit 2; }
      VERSION="$1"
      ;;
    --prefix)
      shift
      [ "$#" -gt 0 ] || { echo "install.sh: --prefix requires an argument" >&2; exit 2; }
      PREFIX="$1"
      ;;
    -h|--help)
      sed -n '2,13p' "$0" 2>/dev/null || echo "Usage: install.sh [--version vX.Y.Z] [--prefix DIR]"
      exit 0
      ;;
    *)
      echo "install.sh: unknown option: $1" >&2
      exit 2
      ;;
  esac
  shift
done

say() {
  printf '%s\n' "$*"
}

die() {
  printf 'install.sh: error: %s\n' "$*" >&2
  exit 1
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || die "required command not found: $1"
}

fetch() {
  url="$1"
  dest="$2"
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL --retry 3 --retry-delay 1 -o "$dest" "$url"
  elif command -v wget >/dev/null 2>&1; then
    wget -qO "$dest" "$url"
  else
    die "neither curl nor wget is installed"
  fi
}

detect_target() {
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Darwin)
      case "$arch" in
        arm64|aarch64) echo "aarch64-apple-darwin" ;;
        x86_64|amd64)  echo "x86_64-apple-darwin" ;;
        *) die "unsupported macOS architecture: $arch" ;;
      esac
      ;;
    Linux)
      case "$arch" in
        x86_64|amd64)  echo "x86_64-unknown-linux-musl" ;;
        aarch64|arm64) echo "aarch64-unknown-linux-musl" ;;
        *) die "unsupported Linux architecture: $arch" ;;
      esac
      ;;
    *)
      die "unsupported operating system: $os (supported: macOS, Linux)"
      ;;
  esac
}

resolve_latest_version() {
  # 1. Public redirect (avoids GitHub API rate limit)
  if command -v curl >/dev/null 2>&1; then
    eff="$(curl -fsSL -o /dev/null -w '%{url_effective}' "https://github.com/${REPO}/releases/latest" 2>/dev/null || true)"
    tag="${eff##*/}"
    case "$tag" in
      v*) printf '%s' "$tag"; return 0 ;;
    esac
  fi
  # 2. Private repo via GITHUB_TOKEN / GH_TOKEN
  if [ -n "$TOKEN" ] && command -v curl >/dev/null 2>&1; then
    tag="$(curl -fsSL -H "Authorization: Bearer ${TOKEN}" -H "Accept: application/vnd.github+json" \
      "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null \
      | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1 || true)"
    case "$tag" in
      v*) printf '%s' "$tag"; return 0 ;;
    esac
  fi
  # 3. Private repo via authenticated `gh` CLI
  if command -v gh >/dev/null 2>&1; then
    tag="$(gh release view -R "$REPO" --json tagName -q .tagName 2>/dev/null || true)"
    case "$tag" in
      v*) printf '%s' "$tag"; return 0 ;;
    esac
  fi
  printf '%s' "v0.0.8"
}

download_release_assets() {
  dest_dir="$1"
  archive_name="$2"
  base_url="https://github.com/${REPO}/releases/download/${VERSION}"

  # Fast path: public direct download
  if fetch "${base_url}/${archive_name}" "${dest_dir}/${archive_name}" 2>/dev/null \
     && fetch "${base_url}/SHA256SUMS.txt" "${dest_dir}/SHA256SUMS.txt" 2>/dev/null; then
    return 0
  fi

  # Private repo fallback 1: authenticated `gh` CLI
  if command -v gh >/dev/null 2>&1 && gh auth status >/dev/null 2>&1; then
    say "    Direct download unavailable (private repo?); downloading via gh CLI ..."
    gh release download "$VERSION" -R "$REPO" \
      -p "$archive_name" \
      -p "SHA256SUMS.txt" \
      -D "$dest_dir" --clobber
    return 0
  fi

  # Private repo fallback 2: GitHub API with GITHUB_TOKEN / GH_TOKEN
  if [ -n "$TOKEN" ] && command -v curl >/dev/null 2>&1; then
    say "    Direct download unavailable; downloading via GitHub API token ..."
    rel_json="${dest_dir}/release.json"
    curl -fsSL -H "Authorization: Bearer ${TOKEN}" -H "Accept: application/vnd.github+json" \
      "https://api.github.com/repos/${REPO}/releases/tags/${VERSION}" -o "$rel_json"
    for asset in "$archive_name" "SHA256SUMS.txt"; do
      asset_url="$(awk -v name="$asset" '
        /"url":/ { last_url = $0 }
        index($0, "\"name\": \"" name "\"") > 0 {
          sub(/.*"url":[[:space:]]*"/, "", last_url)
          sub(/".*/, "", last_url)
          print last_url
          exit
        }
      ' "$rel_json")"
      [ -n "$asset_url" ] || die "asset ${asset} not found in release ${VERSION}"
      curl -fsSL -H "Authorization: Bearer ${TOKEN}" -H "Accept: application/octet-stream" \
        -o "${dest_dir}/${asset}" "$asset_url"
    done
    return 0
  fi

  die "failed to download ${archive_name} from ${REPO} (${VERSION}). If the repo is private, log in with 'gh auth login' or export GITHUB_TOKEN."
}

download_repo_file() {
  repo_path="$1"
  dest="$2"
  ref="${3:-main}"

  if fetch "https://raw.githubusercontent.com/${REPO}/${ref}/${repo_path}" "$dest" 2>/dev/null; then
    return 0
  fi
  if command -v gh >/dev/null 2>&1 && gh auth status >/dev/null 2>&1; then
    gh api "repos/${REPO}/contents/${repo_path}?ref=${ref}" \
      -H "Accept: application/vnd.github.raw" > "$dest" 2>/dev/null && return 0
  fi
  if [ -n "$TOKEN" ] && command -v curl >/dev/null 2>&1; then
    curl -fsSL -H "Authorization: Bearer ${TOKEN}" \
      -H "Accept: application/vnd.github.raw" \
      "https://api.github.com/repos/${REPO}/contents/${repo_path}?ref=${ref}" \
      -o "$dest" 2>/dev/null && return 0
  fi
  return 1
}

verify_sha256() {
  archive_path="$1"
  sums_path="$2"
  archive_name="$(basename "$archive_path")"
  expected="$(awk -v f="$archive_name" '$2 == f || $2 == ("*" f) { print $1; exit }' "$sums_path")"
  [ -n "$expected" ] || die "checksum entry for ${archive_name} not found in SHA256SUMS.txt"

  if command -v sha256sum >/dev/null 2>&1; then
    actual="$(sha256sum "$archive_path" | awk '{ print $1 }')"
  elif command -v shasum >/dev/null 2>&1; then
    actual="$(shasum -a 256 "$archive_path" | awk '{ print $1 }')"
  else
    say "Warning: neither sha256sum nor shasum found; skipping checksum verification."
    return 0
  fi

  [ "$actual" = "$expected" ] || die "SHA-256 mismatch for ${archive_name} (expected ${expected}, got ${actual})"
}

need_cmd tar

TARGET="$(detect_target)"
if [ -z "$VERSION" ]; then
  VERSION="$(resolve_latest_version)"
fi

ARCHIVE="anix-${VERSION}-${TARGET}.tar.gz"
TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/anix-install.XXXXXX")"
trap 'rm -rf "$TMP_DIR"' EXIT INT TERM

say "==> Installing Anix ${VERSION} (${TARGET})"
download_release_assets "${TMP_DIR}" "${ARCHIVE}"

say "==> Verifying SHA-256 checksum ..."
verify_sha256 "${TMP_DIR}/${ARCHIVE}" "${TMP_DIR}/SHA256SUMS.txt"

say "==> Extracting and installing into ${PREFIX} ..."
tar -xzf "${TMP_DIR}/${ARCHIVE}" -C "${TMP_DIR}"
STAGE="${TMP_DIR}/anix-${VERSION}-${TARGET}"
[ -d "$STAGE" ] || die "unexpected archive layout: missing ${STAGE}"

mkdir -p "${PREFIX}/bin" "${PREFIX}/libexec/anix/tools"
install -m 0755 "${STAGE}/bin/anix" "${PREFIX}/bin/anix"
install -m 0755 "${STAGE}/libexec/anix/msh" "${PREFIX}/libexec/anix/msh"
if [ -f "${STAGE}/libexec/anix/anix-provider" ]; then
  install -m 0755 "${STAGE}/libexec/anix/anix-provider" "${PREFIX}/libexec/anix/anix-provider"
  for alias_name in anix-provider-openai-responses anix-provider-openai-compat anix-provider-anthropic; do
    if [ -e "${STAGE}/libexec/anix/${alias_name}" ] || [ -L "${STAGE}/libexec/anix/${alias_name}" ]; then
      ln -sf anix-provider "${PREFIX}/libexec/anix/${alias_name}"
    fi
  done
else
  for prov_bin in "${STAGE}"/libexec/anix/anix-provider-*; do
    if [ -f "$prov_bin" ]; then
      install -m 0755 "$prov_bin" "${PREFIX}/libexec/anix/$(basename "$prov_bin")"
    fi
  done
fi
install -m 0755 "${STAGE}/libexec/anix/tools/aview" "${PREFIX}/libexec/anix/tools/aview"
install -m 0755 "${STAGE}/libexec/anix/tools/aclr" "${PREFIX}/libexec/anix/tools/aclr"

mkdir -p "${CONFIG_DIR}" "${AGENTS_DIR}"
if [ ! -f "${CONFIG_DIR}/config.toml" ]; then
  umask 077
  cat > "${CONFIG_DIR}/config.toml" <<'EOF'
[defaults]
provider = "openai-responses"
model = "deepseek-v4.1-flash"
max_turns = 100
max_tokens = 256000
compact_threshold = 0.8
approval = "auto"
output = "reply"
stderr = "auto"

[providers.openai-responses]
bin = "anix-provider-openai-responses"
base_url = "https://api.openai.com/v1"
api_key_env = "OPENAI_API_KEY"
# api_key = "sk-..."   # Optional static key (api_key_env takes precedence when set)
timeout_sec = 120
EOF
  chmod 0600 "${CONFIG_DIR}/config.toml"
  say "==> Created starter config at ${CONFIG_DIR}/config.toml (0600)"
else
  say "==> Kept existing config at ${CONFIG_DIR}/config.toml"
fi

if [ ! -f "${AGENTS_DIR}/bash.agent" ]; then
  if [ -f "${STAGE}/examples/bash.agent" ]; then
    install -m 0755 "${STAGE}/examples/bash.agent" "${AGENTS_DIR}/bash.agent"
  elif download_repo_file "examples/bash.agent" "${TMP_DIR}/bash.agent" "${VERSION}" \
       || download_repo_file "examples/bash.agent" "${TMP_DIR}/bash.agent" "main"; then
    install -m 0755 "${TMP_DIR}/bash.agent" "${AGENTS_DIR}/bash.agent"
  else
    cat > "${AGENTS_DIR}/bash.agent" <<'EOF'
#!/usr/bin/env anix
---
name: bash-agent
provider: openai-responses
model: deepseek-v4.1-flash
shell: msh
max_turns: 100
max_tokens: 256000
compact_threshold: 0.8
approval: auto
output: reply
stderr: auto
---
你是 Bash 专家，响应用户的指令
EOF
    chmod 0755 "${AGENTS_DIR}/bash.agent"
  fi
  say "==> Installed default agent script at ${AGENTS_DIR}/bash.agent"
else
  say "==> Kept existing ${AGENTS_DIR}/bash.agent"
fi

say ""
say "Installed Anix successfully:"
say "  $("${PREFIX}/bin/anix" --version)"
say "  Binaries: ${PREFIX}/bin/anix, ${PREFIX}/libexec/anix/{msh,anix-provider{,-openai-responses,-openai-compat,-anthropic},tools/{aview,aclr}}"
say "  Config:   ${CONFIG_DIR}/config.toml"
say "  Agents:   ${AGENTS_DIR}/bash.agent"

case ":${PATH}:" in
  *":${PREFIX}/bin:"*) ;;
  *)
    say ""
    say "Add ${PREFIX}/bin to your PATH (e.g. in ~/.zshrc or ~/.bashrc):"
    say "  export PATH=\"${PREFIX}/bin:\$PATH\""
    ;;
esac

say ""
say "Next steps:"
say "  1. Set your provider base_url and API key in ${CONFIG_DIR}/config.toml (or export OPENAI_API_KEY=...)"
say "  2. Run: ${PREFIX}/bin/anix --doctor"
say "  3. Try: ${AGENTS_DIR}/bash.agent \"show disk usage\""
