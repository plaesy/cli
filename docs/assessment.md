# Assessment: is plaesy-memory the best option?

Research checked: 2026-10-26. Implementation status updated: 2026-11-10. Scope: offline agent memory over a Markdown second brain, exposed over MCP and Hermes.

## Verdict

**Not the best overall. It is the best of this set on provenance and offline guarantees, and behind on evaluation breadth, measured secret protection, and lifecycle coverage.**

Which "best" depends on the goal. For an owner who needs to cite and verify what an agent reads from their own notes, plaesy is the strongest choice in the research set (see [comparison](comparison.md)). For a general agent that must answer broad questions over many sources, QMD or a hosted memory service is stronger on raw retrieval, at the cost of an LLM in the loop.

## Scorecard against practice

| Area | Practice | plaesy | Status |
|---|---|---|---|
| Provenance | Every result cites source, line range, revision, time | file, lines, revision, index time, FACT/INFERRED | Met, best in set |
| Offline | No network or API key at runtime | yes, after one-time model download | Met |
| Write scope | Agent writes should be bounded and visible | MCP offers note creation and query-gap resolution; it cannot replace or delete notes | Partial |
| Tool annotations | `readOnlyHint`, `openWorldHint` declared accurately | six read-only tools and two bounded write tools use distinct annotations; all declare `openWorldHint: false` | Met |
| Output schema | Declared and conformant when present (MCP 2025-06-18) | `memory_search` now declares `outputSchema`; a test checks a real result against it | Met (added 2026-10-26) |
| Server instructions | Tell the model how to use the tools | `instructions` in `initialize` | Met |
| Errors | Tool errors as `isError: true`, protocol errors as JSON-RPC | yes | Met |
| Legacy protocol negotiation | Negotiate handshake-era versions | negotiates supported legacy versions through `2025-11-25` | Met |
| Current MCP era | Support current discovery and stateless requests while retaining legacy clients | official `rmcp` SDK handles `2026-07-28` and handshake-era compatibility; HTTP remains loopback-only with host/origin validation | Met (2026-11-09) |
| Pagination | Required when lists can grow | `tools/list` is a fixed list of eight; search and maintenance reports have explicit limits | Not needed |
| Resources and prompts | Optional | `resources/list` answered; no resources exposed | Partial, by design |
| Secrets | Keep credentials out of memory (JetBrains 2026 survey guidance) | `memory hygiene` checks credential assignments plus common AWS, GitHub, Slack, Stripe, Google, and OpenAI token formats; reports only kind/path/line. A synthetic unit corpus reports precision 1.00 (11/11) and recall 1.00 (11/11) against 7 negative examples; this is a narrow rule regression check, not real-world coverage evidence. | Partial |
| Staleness | Mark unverified and obsolete facts | `last_verified`, STALE penalty, `status`, `valid_from`/`valid_to` | Met |
| Lifecycle | Write, update, forget are measured (MemOps, arXiv 2607.12893) | create and resolve are supported; update/forget lifecycle and evaluation are absent | **Gap** |
| Evaluation | Retrieval recall, end-to-end QA accuracy, and token cost reported separately | Recall@k on LongMemEval-S, 94 questions only; no QA accuracy; no token cost | **Gap** |
| Reproducibility | Others can rerun the numbers | `make lme`; full 470-question run not done | **Gap** |
| Multilingual default | Non-English notes retrieve well | `multilingual-e5-small` is the code and onboarding default; MiniLM remains selectable; current evaluation is too small to establish broad superiority | Partial |
| Maintenance | Hygiene is run regularly, not once | bounded `curate` report is available in CLI, MCP, and the in-process scheduler; `schedule install/uninstall` registers a per-user task on Windows, systemd Linux, and macOS | Partial: per-OS runtime coverage remains to verify |

## What the research says, and how it applies

- **MCP tool annotations are hints, not guarantees.** Clients must treat them as untrusted unless the server is trusted (MCP 2025-06-18). plaesy's read/write annotations are accurate; the two write tools are limited to creating a note and resolving a query gap. `memory_curate` is read-only and returns bounded review candidates. Source: [MCP tools specification](https://modelcontextprotocol.io/specification/2025-06-18/server/tools).
- **Provenance is a recognised direction.** Recent work frames memory as needing provenance and poisoning resistance (for example Eywa, arXiv 2605.30771). Titles only were read; the details are unverified. plaesy already does provenance; poisoning resistance is not addressed.
- **Secrets do not belong in agent memory.** The JetBrains 2026 guidance says so directly. plaesy indexes everything under the vault, so a pasted API key can still be returned to the model. The hygiene scan is a useful heuristic, not comprehensive secret protection. Source: [JetBrains, long-term memory in AI agents](https://www.jetbrains.com/pages/ai-agents/building/long-term-memory/).
- **LongMemEval and LoCoMo numbers are not comparable across vendors.** Vendor numbers cluster in the 90s, but they measure different things (Recall@k versus LLM-judged accuracy), use different harnesses, and LoCoMo has disputed answer-key errors (one audit reports 6.4% wrong). Source: [Mem0, AI memory benchmarks](https://mem0.ai/library/agent-memory/ai-memory-benchmarks-in-2026) (vendor, unverified); [LoCoMo audit](https://penfieldlabs.substack.com/p/we-audited-locomo-64-of-the-answer) (competitor, unverified).
- **Retrieval is measured far more than writing and forgetting.** Recommended reading: pair retrieval scores with token cost and lifecycle behaviour. plaesy reports only retrieval.
- **Second-brain systems fail on maintenance.** Practitioner reports say capture without revisiting produces an archive. plaesy's hygiene audit and read-only curation report support review; the CLI can register a per-user schedule, while operators still need to act on candidates. Source: [Nexasphere, 2026 guide](https://nexasphere.io/blog/ai-personal-knowledge-management-system-2026) (vendor opinion, unverified).

## Improvements, in order of value

1. **Measure secret-scan coverage.** A safe synthetic regression corpus now checks 11 positive signatures and 7 negative examples (precision/recall both 1.00 on that corpus). Expand it with reviewed, provider-documented edge cases and measure on representative notes; it is still not a replacement for a dedicated secret scanner.
2. **Evaluate end-to-end.** Add QA accuracy and token cost next to Recall@k on the same 94 questions, and run the full 470 set. Report all three.
3. **Validate the current default embedder.** The code and onboarding default is now E5; existing evaluation supports it for multilingual use, but the Indonesian probe has only eight queries. See [embedder-decision](eval/embedder-decision.md).
4. **Verify scheduled review setup on each OS.** Native `schedule install/uninstall` now registers Windows Task Scheduler, systemd user timers, and macOS LaunchAgents, with a saved snapshot of the effective vault configuration. Validate install, repeated install, run, and uninstall on all three operating systems. Headless Linux also requires systemd user lingering. See [operations](operations.md).
5. **Lifecycle operations.** Add safe, auditable update and archive/forget workflows before claiming full memory lifecycle coverage. `remember` is create-only.

## Not planned

- LLM in the read path (kept out of scope to preserve offline, source-cited retrieval).
- Write tools over MCP.
- Automatic capture of conversations.
- Automatic deletion or merging from curation reports; candidates require human review.

## Caveats

- Competitor claims come from READMEs and search summaries. Items marked "unverified" were not read in full.
- The probe for Indonesian is eight queries. The English comparison is 94 questions; one question is about 1.1 points.
