#!/usr/bin/env bash
set -euo pipefail
DIR="${1:?usage: $0 dist/v0.1.0}"
test -d "$DIR" || { echo "release directory not found: $DIR" >&2; exit 1; }
test -f "$DIR/SHA256SUMS" || { echo 'SHA256SUMS missing' >&2; exit 1; }
(cd "$DIR" && sha256sum -c SHA256SUMS)
python3 - "$DIR" <<'PY'
import pathlib, sys, tarfile, zipfile
root = pathlib.Path(sys.argv[1])
bundles = sorted([*root.glob('plaesy-*.tar.gz'), *root.glob('plaesy-*.zip')])
if not bundles:
    raise SystemExit('no platform bundles found')
for bundle in bundles:
    names = tarfile.open(bundle, 'r:gz').getnames() if bundle.suffixes == ['.tar', '.gz'] else zipfile.ZipFile(bundle).namelist()
    actual = {pathlib.PurePosixPath(n.rstrip('/')).name for n in names}
    missing = {'plaesy', 'memory'} - actual
    if missing:
        raise SystemExit(f'{bundle}: missing {sorted(missing)}')
    print(f'verified {bundle.name}')
manifest = root / 'manifest.json'
if manifest.exists():
    import json
    data = json.loads(manifest.read_text())
    if not data.get('version') or not data.get('bundles'):
        raise SystemExit('manifest must contain version and bundles')
    expected = sorted(p.name for p in bundles)
    if sorted(data['bundles']) != expected:
        raise SystemExit('manifest bundle list does not match release directory')
PY