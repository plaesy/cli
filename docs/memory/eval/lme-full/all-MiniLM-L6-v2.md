# LongMemEval-S full retrieval results: all-MiniLM-L6-v2

| Run detail | Value |
|---|---|
| Completed | 2026-11-11 |
| Repository revision | `356430b` (`docs: preserve full LongMemEval result artifacts`) |
| Dataset | LongMemEval-S cleaned, 470 questions |
| Embedding model | `all-MiniLM-L6-v2` |
| Workers | 2 |
| Reranker | Skipped (`LME_SKIP_RERANK=1`) |

## Results

| Retrieval mode | Questions | R@1 | R@3 | R@5 | R@10 | MRR |
|---|---:|---:|---:|---:|---:|---:|
| keyword | 470 | 0.819 | 0.930 | 0.951 | 0.977 | 0.878 |
| semantic | 470 | 0.887 | 0.966 | 0.977 | 0.989 | 0.927 |
| hybrid | 470 | 0.896 | 0.972 | 0.987 | 0.994 | 0.936 |

### Recall@5 by question type

| Question type | Questions | Keyword | Semantic | Hybrid |
|---|---:|---:|---:|---:|
| knowledge-update | 72 | 1.000 | 1.000 | 1.000 |
| multi-session | 121 | 0.975 | 0.983 | 1.000 |
| single-session-assistant | 56 | 1.000 | 1.000 | 1.000 |
| single-session-preference | 30 | 0.667 | 0.967 | 0.967 |
| single-session-user | 64 | 1.000 | 0.969 | 0.984 |
| temporal-reasoning | 127 | 0.921 | 0.953 | 0.969 |

## Method and limits

This run resumed from a checkpoint with 240 of the 470 question results and completed the remaining 230. The final checkpoint has 470 unique question IDs, and its question ID set matches the full `multilingual-e5-small` run. Both runs used the same cleaned LongMemEval-S dataset and retrieval harness with the cross-encoder reranker disabled. The harness reports session-level retrieval ranks.

This is a matched retrieval comparison only. It does not measure generated-answer accuracy, token cost, or end-to-end task success. The retrieval difference does not establish broad model superiority or justify changing the default on its own; the Indonesian probe and multilingual use case remain relevant. See the [embedder decision](../embedder-decision.md).

The run used the release `longmemeval` example with two workers. To reproduce after obtaining the dataset, from a POSIX shell:

```sh
MEMORY_MODEL_NAME=all-MiniLM-L6-v2 LME_SKIP_RERANK=1 cargo run --release --locked --example longmemeval -- <DATASET> --workers 2 --checkpoint docs/eval/lme-full/all-MiniLM-L6-v2.jsonl --out docs/eval/lme-full/all-MiniLM-L6-v2.json
```

The [machine-readable summary](all-MiniLM-L6-v2.json) and [470-question checkpoint](all-MiniLM-L6-v2.jsonl) preserve aggregate metrics and per-question ranks. The server execution log is not included because it contains host-specific paths.
