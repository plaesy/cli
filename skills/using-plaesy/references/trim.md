# plaesy trim

Token and context compression in two layers: compress a command's output (layer 1), or compress prose files heuristically or through an LLM round trip (layer 2).

## Syntax

```sh
plaesy trim run -- <COMMAND> [ARGS]...                 # run and compress the output
plaesy trim compress --path <FILE|DIR> [--level lite|full|ultra] [--recurse] [--dry-run]
plaesy trim llm-queue --path <FILE>                    # export prose segments to rewrite
plaesy trim apply-llm --path <FILE> --annotations <JSON>   # merge rewritten segments back
plaesy trim report                                     # cumulative savings
```

- `run` passes everything after `--` to the command unchanged.
- `compress` rewrites files in place unless `--dry-run`. `--level` is chosen from the analysis graph when omitted.

## Examples

```sh
plaesy trim run -- cargo test -p plaesy-adapter-relay --locked
plaesy trim compress --path docs/notes --recurse --dry-run
```

## Safety

`compress` without `--dry-run` changes files. Run the dry run first, keep the files under version control, and review the diff.

`plaesy trim <sub> --help` is authoritative.
