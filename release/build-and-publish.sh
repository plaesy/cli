#!/usr/bin/env bash
# Maintainer-only manual release for one target. No CI is involved.
set -euo pipefail
VERSION="${1:?usage: $0 v0.1.0 [target]}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST="$ROOT/dist/$VERSION"

PACKAGE_VERSION="$(sed -n 's/^version = "\([^"]*\)"$/\1/p' "$ROOT/Cargo.toml" | head -n 1)"
[ "$VERSION" = "v$PACKAGE_VERSION" ] || { echo "release tag $VERSION must match Cargo.toml version v$PACKAGE_VERSION" >&2; exit 1; }
TARGET="${2:-$(rustc -vV | sed -n 's/^host: //p')}"

command -v cargo >/dev/null || { echo 'cargo is required on the maintainer machine' >&2; exit 1; }
command -v sha256sum >/dev/null || { echo 'sha256sum is required' >&2; exit 1; }
test -f "$ROOT/Cargo.lock" || { echo 'Cargo.lock is required for reproducible release builds' >&2; exit 1; }
test -f "$ROOT/LICENSE" || { echo 'LICENSE is required in release bundles' >&2; exit 1; }
case "$TARGET" in
  x86_64-unknown-linux-gnu) PLATFORM=linux-x86_64; EXT=; ARCHIVE=tar.gz ;;
  aarch64-unknown-linux-gnu) PLATFORM=linux-arm64; EXT=; ARCHIVE=tar.gz ;;
  x86_64-apple-darwin) PLATFORM=macos-x86_64; EXT=; ARCHIVE=tar.gz ;;
  aarch64-apple-darwin) PLATFORM=macos-arm64; EXT=; ARCHIVE=tar.gz ;;
  x86_64-pc-windows-msvc) PLATFORM=windows-x86_64; EXT=.exe; ARCHIVE=zip ;;
  *) echo "unsupported target: $TARGET" >&2; exit 1 ;;
esac
mkdir -p "$DIST"
WORK="$(mktemp -d "$DIST/.work-$PLATFORM.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT
trap 'rm -rf "$WORK"; exit 130' INT
trap 'rm -rf "$WORK"; exit 143' TERM
cargo build --release --locked --target "$TARGET"
cp "$ROOT/target/$TARGET/release/plaesy$EXT" "$WORK/plaesy$EXT"
cp "$ROOT/LICENSE" "$WORK/"
NAME="plaesy-${PLATFORM}.${ARCHIVE}"
if [ "$ARCHIVE" = tar.gz ]; then tar -czf "$DIST/$NAME" -C "$WORK" .
else (cd "$WORK" && zip -q -r "$DIST/$NAME" .); fi
(cd "$DIST" && sha256sum "$NAME" > SHA256SUMS)
printf '{"version":"%s","platform":"%s","target":"%s","bundle":"%s","bundles":["%s"]}\n' "$VERSION" "$PLATFORM" "$TARGET" "$NAME" "$NAME" > "$DIST/manifest.json"
printf 'Built %s\nChecksum: %s\nManifest: %s\n' "$DIST/$NAME" "$DIST/SHA256SUMS" "$DIST/manifest.json"
printf 'Run release/publish.sh after all platform builds are collected.\n'
