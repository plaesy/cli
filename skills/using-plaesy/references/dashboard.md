# plaesy dashboard

Writes one static HTML page with task counts per status and delegate usage totals. The page has no scripts and no network references, so it opens offline.

## Syntax

```sh
plaesy dashboard [--path <DIR>] [--out <FILE>]
```

| Option | Default |
|---|---|
| `--path` | `.`; the nearest ancestor containing `.plaesy/` is used |
| `--out` | `.plaesy/analysis/dashboard.html` under the project root |

## Verify

Open the printed file in a browser and compare the counts with `plaesy tasks board`.

`plaesy dashboard --help` is authoritative.
