# Embedder decision: all-MiniLM-L6-v2 vs multilingual-e5-small

Evaluation date: 2026-11-11. Implementation status checked: 2026-11-11. Scope: compare these two models only; Indonesian-specific models are outside this decision.

Current implementation: `multilingual-e5-small` is the default in `EmbeddingConfig` and onboarding. `all-MiniLM-L6-v2` remains selectable for English-focused vaults. The full LongMemEval-S run below gives broader retrieval evidence for E5, but does not compare it with MiniLM on all 470 questions or prove broad superiority. Changing the default requires an explicit decision and a full re-index.

## Evidence and implementation history

The earlier decision record kept MiniLM because the English and Indonesian samples were too small for a general claim. The implementation has since moved to E5; this document records the observed behavior and evidence limits rather than treating the old decision as current code.

## English: LongMemEval-S, every 5th question (94 questions), no reranker

| Mode | MiniLM R@1 | e5 R@1 | MiniLM R@5 | e5 R@5 | MiniLM MRR | e5 MRR |
|---|---|---|---|---|---|---|
| keyword | 0.798 | 0.798 | 0.936 | 0.936 | 0.861 | 0.861 |
| semantic | 0.840 | 0.862 | 0.957 | 0.968 | 0.889 | 0.903 |
| hybrid | 0.840 | 0.883 | 0.968 | 0.947 | 0.891 | 0.908 |

Source: `docs/eval/lme-subset/all-MiniLM-L6-v2.txt`, `docs/eval/lme-subset/multilingual-e5-small.txt`.

- One question is about 1.1 points at n = 94, and the 95% interval on R@5 is about ±3.5 points (README). The hybrid R@5 gap (0.968 vs 0.947, two questions) is inside that interval.
- e5 is higher on R@1 and MRR. Those differences are also within noise at this size.
- Verdict on English: no reliable difference.

## English: LongMemEval-S, full set (470 questions), E5

The full 470-question retrieval run for `multilingual-e5-small` completed on 2026-11-11. The checkpoint contains one result per dataset question. This run reports retrieval recall and MRR only; it does not measure answer accuracy or token cost. It also does not provide a full-set MiniLM comparison.

| Mode | R@1 | R@3 | R@5 | R@10 | MRR |
|---|---:|---:|---:|---:|---:|
| keyword | 0.819 | 0.930 | 0.951 | 0.977 | 0.878 |
| semantic | 0.885 | 0.962 | 0.977 | 0.989 | 0.925 |
| hybrid | 0.891 | 0.955 | 0.972 | 0.996 | 0.928 |

Hybrid leads at R@1 and MRR; semantic leads at R@3 and R@5. These results alone do not establish which mode is best for answer quality. The benchmark output, including R@5 by question type, is recorded in [`lme-full/multilingual-e5-small.md`](lme-full/multilingual-e5-small.md).

## Indonesian: probe of 8 hand-labelled queries (`docs/eval/id-probe`)

| Model | Semantic top-1 / top-3 | Hybrid top-1 / top-3 |
|---|---|---|
| all-MiniLM-L6-v2 | 4/8 · 6/8 | 6/8 · 7/8 |
| multilingual-e5-small | 7/8 · 8/8 | 6/8 · 8/8 |

- Eight queries: one query is 12.5 points. The direction is consistent (e5 ahead on semantic), but this is a probe, not a benchmark.
- Also measured: all-indo-e5-small-v4 (semantic 7/8 · 8/8, hybrid 6/8 · 8/8) and indonesian-embedding-small q8 (semantic 7/8 · 8/8, hybrid 7/8 · 8/8). Not part of this decision, recorded for the owner.

## Cost and risk of switching

- e5 has 12 layers and MiniLM has 6. Per question e5 is roughly twice as slow in the LongMemEval harness (MiniLM about 37 s per question, e5 about 60 s with two workers).
- Model file: e5 is about 470 MB, MiniLM about 90 MB.
- Changing the model re-embeds every document. Keep one database per model (SETUP.md).

## Open items

- Run MiniLM on the same 470 questions with matching retrieval settings before drawing a full-set embedder comparison.
- Tests that must pass before switching: `cargo test --test relevance_eval` with the chosen default.
- Prefix behaviour: `multilingual-e5-small` gets `query: `/`passage: ` automatically (name contains "e5"), which matches its training.
