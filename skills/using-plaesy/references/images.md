# plaesy images

Generates an image file through a provider API (network; may bill the provider's account).

## Syntax

```sh
plaesy images create --prompt "<text>" --out <FILE> [--provider openai|gemini] [--size <WxH>]
```

`--prompt` and `--out` are required. The provider defaults to `openai`. The API key comes from the environment: `OPENAI_API_KEY` for `openai`, `GEMINI_API_KEY` for `gemini`. A missing key is an error that names the variable.

## Example

```sh
plaesy images create --prompt "flat monochrome logo of a relay baton" --out assets/logo.png --size 1024x1024
```

## Verify

The file at `--out` exists and opens as an image.

`plaesy images create --help` is authoritative.
