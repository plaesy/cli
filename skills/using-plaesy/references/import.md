# plaesy import

Turns existing code or a database schema into an IR document (JSON on stdout; a loss report on stderr).

## Syntax

```sh
plaesy import --from sql|rust|python --name <IR-NAME> <PATH>
```

| `--from` | Input | Output IR |
|---|---|---|
| `sql` | A SQLite database file, opened read-only | `data-ir/1` |
| `rust` | A Rust source file | `api-ir/1` |
| `python` | A Python source file | `api-ir/1` |

A wrong SQLite path fails instead of creating an empty database. Types with no `data-ir/1` equivalent (for example `REAL`) are an error, not a silent translation.

## Examples

```sh
plaesy import --from sql --name shop app.sqlite > shop.data-ir.json
plaesy import --from rust --name relay src/relay.rs > relay.api-ir.json
plaesy openapi relay.api-ir.json > relay.openapi.json
```

## Verify

Read the stderr report for anything the importer dropped, then feed the result to `openapi` (for `api-ir/1`) or inspect it.

`plaesy import --help` is authoritative.
