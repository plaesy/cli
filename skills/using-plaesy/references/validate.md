# plaesy validate

Project-wide checks, or one validator per target or file. Project discovery stops at the nearest git repository boundary.

## Contents

- [Syntax](#syntax)
- [Targets](#targets)
- [Markdown lint](#markdown-lint)
- [Examples](#examples)

## Syntax

```sh
plaesy validate                       # constitution, project memory, Markdown, task integrity
plaesy validate --list                # list targets
plaesy validate <FILE>...             # .md, .docx, .pptx, .xlsx: the extension picks the checker
plaesy validate <TARGET> [options]
```

Checks that do not apply are reported as skipped with a reason. `contracts` and `assumptions` are not part of the default run.

## Targets

| Target | Syntax | Checks |
|---|---|---|
| `markdown` | `markdown [PATHS]... [options]` | Built-in markdownlint-compatible rules plus accessibility checks |
| `links` | `links [DIR] [--json]` | Relative Markdown links, images, and heading anchors; skips URLs and template paths; never touches the memory index |
| `contracts` | `contracts` | Every task `contracts.json` loads and is well formed; runs nothing |
| `constitution` | `constitution [PATH]` | A generated constitution against its template contract |
| `memory` | `memory` | `.plaesy/memory/` for external references and self-containment (for the vault use `plaesy memory hygiene`) |
| `assumptions` | `assumptions [--review-days N]` | `ASSUMED —` tags, optionally gated by age |
| `docx` | `docx <FILE>` | OOXML structure |
| `pptx` | `pptx <FILE> [EXPECTED-SLIDES]` | OOXML structure and slide count |
| `xlsx` | `xlsx <FILE> [SHEET]...` | OOXML structure and expected sheet names |
| `design` | `design <PATHS>... [--evidence <DIR>] [--strict]` | Frontend output against design rules; `--strict` fails on WARN |
| `evals` | `evals` | Instruction cases and lint fixtures; model briefs need `PLAESY_EVAL_RUNNER` |

The Office targets inspect the container and XML parts; they do not render layout.

## Markdown lint

| Option | Meaning |
|---|---|
| `--no-baseline` | Ignore the ratchet baseline and require zero violations |
| `--fix` | Apply machine-fixable rules in place (**write**) |
| `--summary` | Per-rule histogram only |
| `--list-rules` | Show implemented rules |
| `--config <FILE>` | Default `.markdownlint.json`, then `.markdownlintrc.json` |
| `--baseline <FILE>` / `--write-baseline` | Use or re-record `.markdownlint-baseline.json` |
| `--max-files N` | Offending files to detail (default 20; 0 = all) |

Without `--no-baseline`, the run passes while the total stays at or below the recorded baseline, so new violations can hide inside an old budget. Use `--no-baseline` on files you changed.

## Examples

```sh
plaesy validate markdown docs/commands skills/using-plaesy --no-baseline
plaesy validate links ~/vault --json
plaesy validate contracts
plaesy validate deck.pptx
plaesy validate xlsx budget.xlsx Summary Data
```

`plaesy validate <target> --help` is authoritative.
