# Search evaluation

There is no user-facing `plaesy eval` command in the current CLI. The former command syntax in
this page was stale and did not match a registered command.

Use the supported evaluation entry point that matches the task:

- `plaesy validate evals` runs the repository's instruction cases and design-lint fixtures. Model
  briefs require a runner configured through `PLAESY_EVAL_RUNNER`.
- `cargo run --release --example longmemeval -- <dataset.json> --stride 5` runs the retrieval
  benchmark against a LongMemEval dataset. `make lme DATA=<dataset.json> STRIDE=5` is a convenience
  wrapper in a source checkout.

These workflows evaluate different things; the instruction cases are not a substitute for a
retrieval-quality benchmark.
