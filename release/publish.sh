#!/usr/bin/env bash
# Publish one already-built, verified multi-platform release. No CI is involved.
set -euo pipefail
VERSION="${1:?usage: $0 v0.1.0 [dist-root]}"
ROOT="${2:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../dist/$VERSION" && pwd)}"
REPO="${GITHUB_REPOSITORY:-plaesy/cli}"
command -v gh >/dev/null || { echo 'gh is required to publish' >&2; exit 1; }
command -v sha256sum >/dev/null || { echo 'sha256sum is required' >&2; exit 1; }
command -v python3 >/dev/null || { echo 'python3 is required to build the release manifest' >&2; exit 1; }
[ -d "$ROOT" ] || { echo "release directory not found: $ROOT" >&2; exit 1; }
mapfile -t bundles < <(find "$ROOT" -maxdepth 1 -type f \( -name 'plaesy-*.tar.gz' -o -name 'plaesy-*.zip' \) -print | sort)
[ "${#bundles[@]}" -gt 0 ] || { echo "no platform bundles found in $ROOT" >&2; exit 1; }
for bundle in "${bundles[@]}"; do
  archive="$bundle" python3 - <<'PY'
import os, tarfile, zipfile
p = os.environ['archive']
if p.endswith('.tar.gz'):
    names = tarfile.open(p, 'r:gz').getnames()
else:
    names = zipfile.ZipFile(p).namelist()
required = {'plaesy', 'memory'}
actual = {n.rstrip('/').split('/')[-1] for n in names}
missing = required - actual
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
if ! gh release view "$VERSION" --repo "$REPO" >/dev/null 2>&1; then
  gh release create "$VERSION" --repo "$REPO" --draft --verify-tag --title "Plaesy $VERSION" --generate-notes
fi
for bundle in "${bundles[@]}"; do gh release upload "$VERSION" "$bundle" --repo "$REPO"; done
gh release upload "$VERSION" "$ROOT/SHA256SUMS" "$ROOT/manifest.json" --repo "$REPO"
printf 'Uploaded draft release %s. Inspect it, then publish it with:\n  gh release edit %s --repo %s --draft=false\n' "$VERSION" "$VERSION" "$REPO"