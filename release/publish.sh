#!/usr/bin/env bash
# Publish one already-built, verified multi-platform release. No CI is involved.
set -euo pipefail
VERSION="${1:?usage: $0 v0.1.0 [dist-root]}"
SCRIPT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT="${2:-$SCRIPT_ROOT/dist/$VERSION}"
REPO="${GITHUB_REPOSITORY:-plaesy/cli}"
command -v python3 >/dev/null || { echo 'python3 is required to build the release manifest' >&2; exit 1; }
PACKAGE_VERSION="$(python3 - "$SCRIPT_ROOT/Cargo.toml" <<'PY'
import pathlib, sys, tomllib
print(tomllib.loads(pathlib.Path(sys.argv[1]).read_text())['package']['version'])
PY
)"
[ "$VERSION" = "v$PACKAGE_VERSION" ] || { echo "release tag $VERSION must match Cargo.toml version v$PACKAGE_VERSION" >&2; exit 1; }
command -v gh >/dev/null || { echo 'gh is required to publish' >&2; exit 1; }
command -v sha256sum >/dev/null || { echo 'sha256sum is required' >&2; exit 1; }
[ -d "$ROOT" ] || { echo "release directory not found: $ROOT" >&2; exit 1; }
mapfile -t bundles < <(find "$ROOT" -maxdepth 1 -type f \( -name 'plaesy-*.tar.gz' -o -name 'plaesy-*.zip' \) -print | sort)
expected=(
  plaesy-linux-x86_64.tar.gz
  plaesy-linux-arm64.tar.gz
  plaesy-macos-x86_64.tar.gz
  plaesy-macos-arm64.tar.gz
  plaesy-windows-x86_64.zip
)
[ "${#bundles[@]}" -eq "${#expected[@]}" ] || { echo "expected exactly five platform bundles in $ROOT" >&2; exit 1; }
for name in "${expected[@]}"; do
  [ -f "$ROOT/$name" ] || { echo "required platform bundle is missing: $name" >&2; exit 1; }
done
for bundle in "${bundles[@]}"; do
  archive="$bundle" python3 - <<'PY'
import os, tarfile, zipfile
p = os.environ['archive']
if p.endswith('.tar.gz'):
    names = tarfile.open(p, 'r:gz').getnames()
else:
    names = zipfile.ZipFile(p).namelist()
actual = {n.rstrip('/').split('/')[-1] for n in names}
actual = {name[:-4] if name.endswith('.exe') else name for name in actual}
missing = {'plaesy', 'LICENSE'} - actual
if missing:
    raise SystemExit(f'{p}: missing {sorted(missing)}')
PY
done
(cd "$ROOT" && sha256sum "${bundles[@]/#"$ROOT"\//}" > SHA256SUMS)
python3 - "$ROOT" "$VERSION" <<'PY'
import json, pathlib, sys
root, version = pathlib.Path(sys.argv[1]), sys.argv[2]
files = sorted(p.name for p in root.iterdir() if p.name.startswith('plaesy-') and p.suffix in ('.zip', '.gz'))
(root / 'manifest.json').write_text(json.dumps({'version': version, 'bundles': files}, indent=2) + '\n')
PY
if existing_draft="$(gh release view "$VERSION" --repo "$REPO" --json isDraft --jq '.isDraft' 2>/dev/null)"; then
  [ "$existing_draft" = "true" ] || {
    echo "release $VERSION already exists and is published; refusing to modify it" >&2
    exit 1
  }
else
  gh release create "$VERSION" --repo "$REPO" --draft --verify-tag --title "Plaesy $VERSION" --generate-notes
fi
for bundle in "${bundles[@]}"; do gh release upload "$VERSION" "$bundle" --repo "$REPO" --clobber; done
gh release upload "$VERSION" "$ROOT/SHA256SUMS" "$ROOT/manifest.json" --repo "$REPO" --clobber
printf 'Uploaded draft release %s. Inspect it, then publish it with:\n  gh release edit %s --repo %s --draft=false\n' "$VERSION" "$VERSION" "$REPO"
