# LongMemEval-S full retrieval results: multilingual-e5-small

| Run detail | Value |
|---|---|
| Completed | 2026-11-11 |
| Repository revision | `1b34912` (`docs: document verified Linux server updates`) |
| Dataset | LongMemEval-S cleaned, 470 questions |
| Embedding model | `multilingual-e5-small` |
| Workers | 1 |

## Results

| Retrieval mode | Questions | R@1 | R@3 | R@5 | R@10 | MRR |
|---|---:|---:|---:|---:|---:|---:|
| keyword | 470 | 0.819 | 0.930 | 0.951 | 0.977 | 0.878 |
| semantic | 470 | 0.885 | 0.962 | 0.977 | 0.989 | 0.925 |
| hybrid | 470 | 0.891 | 0.955 | 0.972 | 0.996 | 0.928 |

### Recall@5 by question type

| Question type | Questions | Keyword | Semantic | Hybrid |
|---|---:|---:|---:|---:|
| knowledge-update | 72 | 1.000 | 1.000 | 1.000 |
| multi-session | 121 | 0.975 | 1.000 | 0.967 |
| single-session-assistant | 56 | 1.000 | 1.000 | 1.000 |
| single-session-preference | 30 | 0.667 | 0.867 | 0.867 |
| single-session-user | 64 | 1.000 | 0.984 | 0.984 |
| temporal-reasoning | 127 | 0.921 | 0.953 | 0.969 |

## Method and limits

The full dataset was covered using the resumable checkpoint: the final checkpoint has 470 question results, and this run processed the 450 questions missing when it resumed. The harness reported session-level retrieval. This is a retrieval-only measurement; it does not include generated-answer accuracy, token cost, or a full-set MiniLM comparison. Do not compare these numbers directly with answer-quality scores from other benchmark harnesses.

The run used the release `longmemeval` example with one worker. A fresh run can be reproduced after obtaining the dataset:

```sh
cargo run --release --locked --example longmemeval -- <DATASET> --workers 1 --checkpoint docs/eval/lme-full/multilingual-e5-small.jsonl --out docs/eval/lme-full/multilingual-e5-small.json
```

The benchmark output and checkpoint were verified on the Linux server. The checked-in report preserves the metrics and reproducibility command; the raw checkpoint and execution log are not included.
