#!/usr/bin/env bash
set -euo pipefail
DIR="${1:?usage: $0 dist/v0.1.0}"
test -d "$DIR" || { echo "release directory not found: $DIR" >&2; exit 1; }
test -f "$DIR/SHA256SUMS" || { echo 'SHA256SUMS missing' >&2; exit 1; }
python3 - "$DIR" <<'PY'
import pathlib, sys
root = pathlib.Path(sys.argv[1])
expected = {
    'plaesy-linux-x86_64.tar.gz',
    'plaesy-linux-arm64.tar.gz',
    'plaesy-macos-x86_64.tar.gz',
    'plaesy-macos-arm64.tar.gz',
    'plaesy-windows-x86_64.zip',
}
bundles = sorted([*root.glob('plaesy-*.tar.gz'), *root.glob('plaesy-*.zip')])
actual = {bundle.name for bundle in bundles}
if actual != expected:
    raise SystemExit(f'expected platform bundles {sorted(expected)}, found {sorted(actual)}')
checksums = {}
for line in (root / 'SHA256SUMS').read_text().splitlines():
    parts = line.split(maxsplit=1)
    if len(parts) != 2 or len(parts[0]) != 64 or any(c not in '0123456789abcdefABCDEF' for c in parts[0]):
        raise SystemExit(f'invalid checksum line: {line!r}')
    name = parts[1].lstrip('*')
    if name.startswith('./'):
        name = name[2:]
    if '/' in name or '\\' in name or name in checksums:
        raise SystemExit(f'invalid or duplicate checksum filename: {name!r}')
    checksums[name] = parts[0]
if set(checksums) != expected:
    raise SystemExit(f'SHA256SUMS must cover exactly {sorted(expected)}, found {sorted(checksums)}')
PY
(cd "$DIR" && sha256sum -c SHA256SUMS)
python3 - "$DIR" <<'PY'
import pathlib, sys, tarfile, zipfile
root = pathlib.Path(sys.argv[1])
bundles = sorted([*root.glob('plaesy-*.tar.gz'), *root.glob('plaesy-*.zip')])
for bundle in bundles:
    names = tarfile.open(bundle, 'r:gz').getnames() if bundle.suffixes == ['.tar', '.gz'] else zipfile.ZipFile(bundle).namelist()
    actual = {pathlib.PurePosixPath(n.rstrip('/')).name for n in names}
    normalized = {name[:-4] if name.endswith('.exe') else name for name in actual}
    missing = {'plaesy', 'LICENSE'} - normalized
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
