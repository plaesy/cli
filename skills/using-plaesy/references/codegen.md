# plaesy codegen

Generates source code from a `flow-ir/1` JSON document. Code goes to stdout; the report goes to stderr.

## Syntax

```sh
plaesy codegen --lang js|go <FILE>
```

## Example

```sh
plaesy codegen --lang go checkout.flow-ir.json > checkout.go
plaesy codegen --lang js checkout.flow-ir.json > checkout.js
```

## Verify

Check stderr for warnings, then compile or lint the output with the target toolchain (`go vet`, `node --check`). An invalid document exits with status 1.

`plaesy codegen --help` is authoritative.
