#!/usr/bin/env sh
# Install Plaesy and its standalone memory engine without Rust or Git.
set -eu

OWNER="${PLAESY_GITHUB_OWNER:-plaesy}"
CLI_REPO="${PLAESY_CLI_REPO:-cli}"
MEMORY_REPO="${PLAESY_MEMORY_REPO:-memory}"
VERSION="${PLAESY_VERSION:-latest}"
PREFIX="${PLAESY_INSTALL_DIR:-$HOME/.local/bin}"
BASE_URL="https://github.com"

fail() { printf 'Plaesy installation failed: %s\n' "$1" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || fail "required command not found: $1"; }
need curl
need uname

OS="$(uname -s)"
ARCH="$(uname -m)"
case "$OS" in
  Linux) OS_ID=linux ;;
  Darwin) OS_ID=macos ;;
  *) fail "unsupported operating system: $OS (use the Windows installer on Windows)" ;;
esac
case "$ARCH" in
  x86_64|amd64) ARCH_ID=x86_64 ;;
  aarch64|arm64) ARCH_ID=arm64 ;;
  *) fail "unsupported CPU architecture: $ARCH" ;;
esac

asset() {
  repo="$1"; name="$2"
  if [ "$VERSION" = latest ]; then
    printf '%s/%s/%s/releases/latest/download/%s' "$BASE_URL" "$OWNER" "$repo" "$name"
  else
    printf '%s/%s/%s/releases/download/%s/%s' "$BASE_URL" "$OWNER" "$repo" "$VERSION" "$name"
  fi
}

mkdir -p "$PREFIX"
TMP="$(mktemp -d 2>/dev/null || mktemp -d -t plaesy)"
trap 'rm -rf "$TMP"' EXIT INT TERM

printf 'Installing Plaesy for %s/%s...\n' "$OS_ID" "$ARCH_ID"
curl --fail --location --silent --show-error "$(asset "$CLI_REPO" "plaesy-${OS_ID}-${ARCH_ID}")" -o "$TMP/plaesy" \
  || fail "Plaesy release asset is unavailable. Publish plaesy-${OS_ID}-${ARCH_ID} first."
curl --fail --location --silent --show-error "$(asset "$MEMORY_REPO" "memory-${OS_ID}-${ARCH_ID}")" -o "$TMP/memory" \
  || fail "Memory release asset is unavailable. Publish memory-${OS_ID}-${ARCH_ID} first."

install -m 0755 "$TMP/plaesy" "$PREFIX/plaesy"
install -m 0755 "$TMP/memory" "$PREFIX/memory"

case ":${PATH}:" in
  *:"$PREFIX":*) : ;;
  *)
    printf '\nInstalled to %s. Add it to PATH:\n' "$PREFIX"
    printf '  export PATH="%s:$PATH"\n' "$PREFIX"
    printf 'Then open a new terminal or run: . %s\n' "$HOME/.profile"
    ;;
esac

printf '\nPlaesy installed successfully.\n'
"$PREFIX/plaesy" status || true
printf '\nTry: plaesy --help\n'
