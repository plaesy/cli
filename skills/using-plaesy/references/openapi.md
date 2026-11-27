# plaesy openapi

Generates an OpenAPI 3.1 document from an `api-ir/1` JSON document. JSON goes to stdout; errors go to stderr with exit code 1.

## Syntax

```sh
plaesy openapi <FILE>
```

## Example

```sh
plaesy import --from python --name billing service.py > billing.api-ir.json
plaesy openapi billing.api-ir.json > billing.openapi.json
```

## Verify

The command exits 0 and the output parses as JSON with `"openapi": "3.1.x"`. Check it with an OpenAPI validator when one is available.

`plaesy openapi --help` is authoritative.
