#!/usr/bin/env bash
# Maintainer-only manual release. No CI is involved.
set -euo pipefail
VERSION="${1:?usage: $0 v0.1.0 [memory-dir] [target]}"
MEMORY_DIR="${2:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../../memory" 2>/dev/null && pwd || true)}"
TARGET="${3:-$(rustc -vV | sed -n 's/^host: //p')}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MEMORY_REF="${MEMORY_REF:-unknown}"
DIST="$ROOT/dist/$VERSION"

command -v cargo >/dev/null || { echo 'cargo is required on the maintainer machine' >&2; exit 1; }
command -v sha256sum >/dev/null || { echo 'sha256sum is required' >&2; exit 1; }
test -f "$MEMORY_DIR/Cargo.toml" || { echo "memory checkout not found: $MEMORY_DIR" >&2; exit 1; }
case "$TARGET" in
  x86_64-unknown-linux-gnu) PLATFORM=linux-x86_64; EXT=; ARCHIVE=tar.gz ;;
  aarch64-unknown-linux-gnu) PLATFORM=linux-arm64; EXT=; ARCHIVE=tar.gz ;;
  x86_64-apple-darwin) PLATFORM=macos-x86_64; EXT=; ARCHIVE=tar.gz ;;
  aarch64-apple-darwin) PLATFORM=macos-arm64; EXT=; ARCHIVE=tar.gz ;;
  x86_64-pc-windows-msvc) PLATFORM=windows-x86_64; EXT=.exe; ARCHIVE=zip ;;
  *) echo "unsupported target: $TARGET" >&2; exit 1 ;;
esac
rm -rf "$DIST"; mkdir -p "$DIST/work"
cargo build --release --locked --target "$TARGET"
cargo build --release --locked --manifest-path "$MEMORY_DIR/Cargo.toml" --target "$TARGET"
cp "$ROOT/target/$TARGET/release/plaesy$EXT" "$DIST/work/plaesy$EXT"
cp "$MEMORY_DIR/target/$TARGET/release/memory$EXT" "$DIST/work/memory$EXT"
cp "$ROOT/LICENSE" "$DIST/work/" 2>/dev/null || true
NAME="plaesy-${PLATFORM}.${ARCHIVE}"
if [ "$ARCHIVE" = tar.gz ]; then tar -czf "$DIST/$NAME" -C "$DIST/work" .
else (cd "$DIST/work" && zip -q -r "$DIST/$NAME" .); fi
(cd "$DIST" && sha256sum "$NAME" > SHA256SUMS)
printf '{"version":"%s","platform":"%s","target":"%s","memory_ref":"%s","bundle":"%s"}\n' "$VERSION" "$PLATFORM" "$TARGET" "$MEMORY_REF" "$NAME" > "$DIST/manifest.json"
printf 'Built %s\nChecksum: %s\nManifest: %s\n' "$DIST/$NAME" "$DIST/SHA256SUMS" "$DIST/manifest.json"
printf 'Run release/publish.sh after all platform builds are collected.\n'