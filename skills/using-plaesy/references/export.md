# plaesy export

Writes the task list or the delegate run summary per provider as Markdown or CSV. Reads only local project files.

## Syntax

```sh
plaesy export tasks|runs [--format md|csv] [--out <FILE>]
```

Output goes to stdout unless `--out` is given. `runs` summarizes `.plaesy/analysis/delegate-usage.jsonl`.

## Examples

```sh
plaesy export tasks --format csv --out tasks.csv
plaesy export runs
```

`plaesy export --help` is authoritative.
