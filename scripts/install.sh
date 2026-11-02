#!/usr/bin/env sh
# Install a verified Plaesy bundle. Rust and Git are not required.
set -eu

OWNER="${PLAESY_GITHUB_OWNER:-plaesy}"
REPO="${PLAESY_RELEASE_REPO:-cli}"
VERSION="${PLAESY_VERSION:-latest}"
PREFIX="${PLAESY_INSTALL_DIR:-$HOME/.local/bin}"
BASE_URL="https://github.com/$OWNER/$REPO/releases"

fail() { printf 'Plaesy installation failed: %s\n' "$1" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || fail "required command not found: $1"; }
need curl
need tar
need mktemp

OS="$(uname -s)"; ARCH="$(uname -m)"
case "$OS" in Linux) OS_ID=linux ;; Darwin) OS_ID=macos ;; *) fail "unsupported OS: $OS" ;; esac
case "$ARCH" in x86_64|amd64) ARCH_ID=x86_64 ;; aarch64|arm64) ARCH_ID=arm64 ;; *) fail "unsupported CPU: $ARCH" ;; esac

if [ "$VERSION" = latest ]; then
  URL="$BASE_URL/latest/download/plaesy-${OS_ID}-${ARCH_ID}.tar.gz"
  SUMS_URL="$BASE_URL/latest/download/SHA256SUMS"
else
  case "$VERSION" in v*) TAG="$VERSION" ;; *) TAG="v$VERSION" ;; esac
  URL="$BASE_URL/download/$TAG/plaesy-${OS_ID}-${ARCH_ID}.tar.gz"
  SUMS_URL="$BASE_URL/download/$TAG/SHA256SUMS"
fi

TMP="$(mktemp -d)"; trap 'rm -rf "$TMP"' EXIT INT TERM
mkdir -p "$PREFIX"
printf 'Installing Plaesy for %s/%s...\n' "$OS_ID" "$ARCH_ID"
curl --proto '=https' --tlsv1.2 --fail --location --silent --show-error "$URL" -o "$TMP/bundle.tar.gz" \
  || fail "release bundle not found: $URL"
curl --proto '=https' --tlsv1.2 --fail --location --silent --show-error "$SUMS_URL" -o "$TMP/SHA256SUMS" \
  || fail "release checksums not found: $SUMS_URL"

EXPECTED="$(awk -v n="plaesy-${OS_ID}-${ARCH_ID}.tar.gz" '$2 == n || $3 == n {print $1; exit}' "$TMP/SHA256SUMS")"
[ -n "$EXPECTED" ] || fail "checksum entry is missing for the downloaded bundle"
if command -v sha256sum >/dev/null 2>&1; then ACTUAL="$(sha256sum "$TMP/bundle.tar.gz" | awk '{print $1}')"
elif command -v shasum >/dev/null 2>&1; then ACTUAL="$(shasum -a 256 "$TMP/bundle.tar.gz" | awk '{print $1}')"
else fail "sha256sum or shasum is required"; fi
[ "$EXPECTED" = "$ACTUAL" ] || fail "checksum mismatch"

tar -xzf "$TMP/bundle.tar.gz" -C "$TMP"
[ -f "$TMP/plaesy" ] && [ -f "$TMP/memory" ] || fail "bundle must contain plaesy and memory"
install -m 0755 "$TMP/plaesy" "$PREFIX/plaesy"
install -m 0755 "$TMP/memory" "$PREFIX/memory"

case ":${PATH}:" in *:"$PREFIX":*) ;; *) printf '\nAdd to PATH and open a new terminal:\n  export PATH="%s:$PATH"\n' "$PREFIX" ;; esac
printf '\nPlaesy installed successfully.\n'
"$PREFIX/plaesy" status || true
printf 'Try: plaesy --help\n'