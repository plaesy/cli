# `plaesy validate`

Run project-wide checks or validate individual project artifacts.

```text
plaesy validate [target | file ...] [flags]
```

Examples:

```bash
plaesy validate                         # project-wide checks
plaesy validate --list                  # list targets
plaesy validate constitution            # validate the constitution
plaesy validate markdown README.md       # lint selected Markdown
plaesy validate links                    # check local links without indexing
plaesy validate links path/to/vault --json
plaesy validate deck.pptx                # dispatch by file extension
```

Without arguments, the command runs project-wide checks for constitution,
project memory, Markdown, and task integrity. Checks that do not apply are
reported as skipped with a reason. A `.md`, `.docx`, `.pptx`, or
`.xlsx` file is routed to its matching validator. Use a named target when you
need target-specific options. The `contracts` and `assumptions` targets are
available explicitly and are not part of the default run.

Project discovery stops at the nearest Git repository boundary. A `.plaesy/`
directory in a parent user profile or unrelated repository is not inherited by
a nested vault or checkout.

| Target | Scope | What it checks |
|---|---|---|
| `contracts` | Repository | Loads and validates each task `contracts.json` |
| `constitution` | Project | Checks the generated constitution against its template contract |
| `memory` | Project | Checks `.plaesy/memory/` for external references and self-containment |
| `markdown` | Project or selected paths | Built-in Markdown lint rules and accessibility checks |
| `links` | Project or selected directory | Checks local Markdown links and heading anchors without touching the memory index |
| `assumptions` | Project | Audits `ASSUMED —` tags, optionally gated by age |
| `docx` | File | Checks `.docx` OOXML structure |
| `pptx` | File | Checks `.pptx` OOXML structure and slide count |
| `xlsx` | File | Checks `.xlsx` OOXML structure and worksheets |
| `design` | Selected paths | Checks frontend output against design rules |
| `evals` | Repository | Runs instruction cases and lint fixtures; model briefs need a runner |

## Named targets

```text
plaesy validate contracts
plaesy validate constitution [path]
plaesy validate memory
plaesy validate markdown [paths...] [flags]
plaesy validate links [directory] [--json]
plaesy validate assumptions [--review-days <n>]
plaesy validate docx <file.docx>
plaesy validate pptx <file.pptx> [expected-slide-count]
plaesy validate xlsx <file.xlsx> [expected-sheet-name...]
plaesy validate design <path...> [--evidence <dir>] [--strict]
plaesy validate evals
```

The Office targets inspect the OOXML container and its XML parts. They do not
render documents, so visual layout is not verified.

The Markdown target includes built-in markdownlint-compatible rules and GitHub
accessibility checks. Run `plaesy validate markdown --list-rules` to
see the implemented rules. It uses `.markdownlint.json`, or `.markdownlintrc.json`
when the current filename is absent, plus the repository baseline unless overridden. Use `--no-baseline` to require zero violations,
`--fix` for supported automatic fixes, or `--write-baseline` to set a new
baseline ceiling.

The links target checks relative Markdown links, image destinations, and heading
anchors. It skips external URLs and does not update or require the Plaesy memory
index. Absolute or machine-specific local destinations are reported. By default
it scans the discovered project root; pass a directory to scan a vault or another
tree directly. Template paths are skipped because they commonly contain intentional
placeholders. `--json` emits the findings as JSON.

Assumptions checks are informational unless a review window is configured or
passed with `--review-days`. The evals target reports model briefs as skipped
unless `PLAESY_EVAL_RUNNER` is configured.

## Plaesy memory validation

`plaesy validate memory` checks the project's generated `.plaesy/memory/` files. For the indexed
Markdown vault, use `plaesy memory hygiene [directory]` to report stale metadata and broken links,
or `plaesy memory stats` to inspect the local index. There is no `plaesy memory validate` command.
This CLI validates existing project constitutions; create and maintain constitution templates in
the project that uses them.

The project check reports known temporary/tool paths and absolute paths inside
user home directories (`~/`, `/Users/<name>`, `/home/<name>`, `/root`, and
`<drive>:\Users\<name>` or `<drive>:\home\<name>`). Absolute system paths such
as `/usr/bin` are not rejected by this portability check.

See [CLI Reference](../reference.md) for the full command tree.
