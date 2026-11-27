# plaesy onboard

First-run setup for memory: choose the vault folder, write configuration, download the embedding model, and build the first index. It replaces the old `plaesy memory onboard`.

## Syntax

```sh
plaesy onboard [--vault <DIR>] [--root <DIR>] [-y] [--no-model] [--rerank] [--no-index] [--config <FILE>] [--db <FILE>]
```

| Option | Meaning |
|---|---|
| `--vault` | Folder with the notes to index |
| `--root` | Folder that holds `.plaesy/config.toml` (default: home folder) |
| `-y`, `--yes` | Accept every default; needs `--vault` unless settings exist |
| `--no-model` | Skip the embedding model download (keyword search only until a model exists) |
| `--rerank` | Enable reranking and download the reranker model |
| `--no-index` | Skip the first index |

## Example

```sh
plaesy onboard --vault ~/vault -y
plaesy memory config
plaesy memory --vault ~/vault stats
```

## Side effects

Writes configuration under `--root`, downloads models over the network unless `--no-model`, and builds a local index outside the vault.

`plaesy onboard --help` is authoritative.
