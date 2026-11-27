# plaesy decisions

Typed, structured choices from a locally installed decision model (Laya, pinned revision). Separate from `advisor` (narrative, provider-backed) and from `memory decisions` (decision notes in the vault).

## Syntax

```sh
plaesy decisions config show|reset
plaesy decisions config set --model laya
plaesy decisions model list
plaesy decisions model info [laya]
plaesy decisions model pull [laya]      # network: downloads from Hugging Face
plaesy decisions model remove [laya]    # destructive: deletes model files, keeps config
plaesy decisions choose --state "<case>" --prompt "<goal>" --option <A> --option <B> [--json]
```

- `choose` needs a non-empty state and prompt and at least two unique options. It returns a structured choice without a rationale; `--json` prints the full response.
- `model pull` checks each file's size and SHA-256 against the pinned revision before promoting it. Inference after install is local.

## Example

```sh
plaesy decisions model pull laya
plaesy decisions choose --state "Customer reports duplicate billing" \
  --prompt "Which response best addresses the issue?" \
  --option "Issue a refund" --option "Escalate to billing support" --json
```

## Verify

`plaesy decisions model list` shows the model as installed.

`plaesy decisions <sub> --help` is authoritative.
