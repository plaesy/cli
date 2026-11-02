#!/usr/bin/env bash
set -euo pipefail
DIR="${1:?usage: $0 dist/v0.1.0}"
test -f "$DIR/SHA256SUMS" || { echo 'SHA256SUMS missing' >&2; exit 1; }
(cd "$DIR" && sha256sum -c SHA256SUMS)
test -f "$DIR/manifest.json" || { echo 'manifest.json missing' >&2; exit 1; }
python3 - "$DIR/manifest.json" <<'PY'
import json, pathlib, sys
p = pathlib.Path(sys.argv[1])
data = json.loads(p.read_text())
for key in ('version', 'platform', 'target', 'bundle'):
    if not data.get(key): raise SystemExit(f'manifest field missing: {key}')
bundle = p.parent / data['bundle']
if not bundle.is_file(): raise SystemExit(f'bundle missing: {bundle}')
print(f"verified {bundle.name} for {data['target']}")
PY