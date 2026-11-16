# `plaesy decisions`

Run typed structured decisions using Plaesy-managed local model files. This feature is separate from `plaesy advisor` (narrative provider-backed analysis) and `plaesy memory` (embedding/search). No Ollama server is used.

## Model support and limitations

The first native model is `laya-decision` 0.2.20 with the public `convaiinnovations/laya` checkpoint pinned to revision `06d956120490bbaebaf4bc0e23377a10be65171f`. Plaesy downloads four required files to its platform model cache, checks exact sizes and SHA-256 digests, stages and verifies files before promotion, and loads from that local path. CPU inference was exercised against this checkpoint; performance depends on hardware.

Only the listed Laya revision is supported. Other TypeSafe/System One models are not assumed compatible. Model download requires network access to Hugging Face; inference after installation is local. The initial command accepts state text (or a JSON state value) and candidate strings. It cannot yet author richer question kinds, compare scores, or emit narrative explanations. A choice result is structured and does not include generative rationale.

## Commands

```sh
plaesy decisions config show
plaesy decisions config set --model laya
plaesy decisions config reset
plaesy decisions model list
plaesy decisions model info laya
plaesy decisions model pull laya
plaesy decisions model remove laya
plaesy decisions choose --state 'Customer reports duplicate billing' \
  --prompt 'Which response best addresses the issue?' \
  --option 'Issue a refund' --option 'Escalate to billing support'
```

`model remove` deletes only the pinned Laya model directory; it retains configuration. `choose` requires at least two unique, non-empty alternatives and a non-empty state and prompt. Use `--json` for the full structured response.

## Artifact integrity manifest

Each pulled file is pinned to the revision and checked against its expected size and SHA-256 before it is promoted into the model directory. `model info` prints the pinned revision and artifact metadata. Installation data lives in Plaesy's platform cache under `plaesy/models/laya/<revision>`; the downloader does not use the Hugging Face cache.
