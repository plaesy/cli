#!/usr/bin/env bash
# Maintainer-only release script. No CI is involved.
set -euo pipefail

VERSION="${1:?usage: $0 v0.1.0 [memory-ref]}"
MEMORY_REF="${2:-c889908da0155d06179593224ca0bd1f876b5e35}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MEMORY_DIR="${MEMORY_DIR:-$ROOT/../memory}"
: "${GITHUB_REPOSITORY:=plaesy/cli}"
MEMORY_REPOSITORY="${MEMORY_REPOSITORY:-plaesy/memory}"
DIST="$ROOT/dist/$VERSION"

command -v cargo >/dev/null || { echo 'cargo is required on the maintainer machine' >&2; exit 1; }
command -v gh >/dev/null || { echo 'gh is required to upload releases' >&2; exit 1; }
test -f "$MEMORY_DIR/Cargo.toml" || { echo "memory checkout not found: $MEMORY_DIR" >&2; exit 1; }
mkdir -p "$DIST"

build_one() {
  local target="$1" suffix="$2" ext="${3:-}"
  rustup target add "$target"
  cargo build --release --locked --target "$target"
  cargo build --release --locked --manifest-path "$MEMORY_DIR/Cargo.toml" --target "$target"
  cp "$ROOT/target/$target/release/plaesy$ext" "$DIST/plaesy-$suffix$ext"
  cp "$MEMORY_DIR/target/$target/release/memory$ext" "$DIST/memory-$suffix$ext"
}

case "$(uname -s):$(uname -m)" in
  Linux:x86_64) build_one x86_64-unknown-linux-gnu linux-x86_64 ;;
  Linux:aarch64|Linux:arm64) build_one aarch64-unknown-linux-gnu linux-arm64 ;;
  Darwin:x86_64) build_one x86_64-apple-darwin macos-x86_64 ;;
  Darwin:arm64) build_one aarch64-apple-darwin macos-arm64 ;;
  *) echo 'This machine cannot build a supported native target; use a maintainer machine for that OS/CPU.' >&2; exit 1 ;;
esac

# Windows artifacts must be built from Windows with the matching Rust target.
for required in "plaesy-$(uname -s | tr '[:upper:]' '[:lower:]')-$(uname -m)"; do :; done
find "$DIST" -type f -exec chmod +x {} +
test -n "$(find "$DIST" -type f -print -quit)"

gh release create "$VERSION" "$DIST"/* --repo "$GITHUB_REPOSITORY" --title "Plaesy $VERSION" --generate-notes
# Memory is independently released, but its asset is published with the same version contract.
gh release create "$VERSION" "$DIST"/memory-* --repo "$MEMORY_REPOSITORY" --title "Memory $VERSION" --generate-notes || true
printf 'Published %s from %s\n' "$VERSION" "$DIST"
