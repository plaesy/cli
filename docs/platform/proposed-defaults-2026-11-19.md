# Proposed defaults for owner items (2026-11-19)

Status: ADOPTED as delegated decisions on 2026-11-19 (see `docs/platform/decisions.md`, section "Delegated decisions"). The owner may revoke any item. The original reasoning is kept below.

Why these are proposals: the cohort, the incumbent in daily use, and the business meaning of success depend on facts only the owner has. Research gives patterns and ranges, not the owner's answer.

## 1. Target cohort

Proposal: 3 to 5 people who already run agent-driven workflows in the same project, with one operator as the primary user. Start with 1 to 2 deep relationships rather than many shallow ones.

Reasoning: a16z asks whether a design partner's needs resemble the wider market, and Amplify warns that pilots fail when partners join as a favor or stay merely curious. Feedback is only useful if the partner gives it regularly.

Sources: https://a16z.com/a-framework-for-finding-a-design-partner/ , https://amplifypartners.com/blog-posts/common-mistakes-with-early-pilots , https://startupstrategies.substack.com/p/who-are-ideal-design-partners

## 2. Success outcome

Proposal (measurable, from spec R3 and R9, not from market data):

- 100 synthetic runs with zero false successes (a success is reported only with stored evidence).
- All ten failure injections in the child spec pass: denial, timeout, cancel, crash, duplicate delivery, lost ack, restart recovery, torn tail, revoked grant, budget exhaustion.
- One real recovery after an injected restart, with the run requeued within the retry budget.

Reasoning: the spec requires no false success under injected failures. A success count alone would hide that.

## 3. Incumbent baseline

Proposal: the incumbent is whichever product the owner uses today for the agent-execution workflow. Pin its version on the day the baseline is taken. The assistant cannot identify that product from the repository.

Comparison targets confirmed from evidence (not yet ratified):

- Orca: `stablyai/orca`, latest desktop tag `v1.4.263` (18 Nov 2026). Source: https://github.com/stablyai/orca/releases
- DeepSeek Harness: reported as "Harness v0.1" (August 2026). The MIT license and package name are not verified. Confirm the official repository before comparing. Source: https://runtimewire.com/article/deepseek-open-sources-agent-harness-v4-pro-api-price-hike

## 4. NFR thresholds

Proposal (threshold-based SLIs, as Grafana recommends, not percentile SLIs alone):

- Availability of the control plane: 99.9% over 28 days (internal target). Source: https://grafana.com/docs/grafana-cloud/observe-and-act/alert-and-measure-reliability/slo/best-practices/
- Control-plane read latency: 95% of requests under 200 ms over 7 days, reported as p95 with a threshold count for the error budget. Source: https://loadforge.com/blog/how-to-set-performance-slos
- Recovery: RPO at most 1 hour and RTO at most 4 hours for run state. Reasoning: 24 hours (nightly) is common for small businesses but is too loose for run state that a crash can interrupt. Restore drill once a month. Sources: https://proginter.com/blog/server-backup-strategy-rpo-rto-and-actually-testing-your-restores , https://aicybr.com/blog/self-hosted-backup-architecture-restore-guide
- Run-log and evidence retention: 30 days proposed. This is an engineering default, not a legal retention rule. Legal or compliance retention needs owner or counsel input.

Caveat: the sources are vendor and consultancy blogs. Validate these numbers against measured baselines before any gate uses them.

## 5. Cutover appetite and rollback

Proposal:

- No big-bang cutover. Run the incumbent and Plaesy in parallel for a fixed period, proposed at 2 weeks.
- Write rollback criteria and acceptable variance before the parallel run starts, not after a discrepancy.
- Reconcile data after each batch and again just before cutover.
- Rehearse the cutover two or three times before live.
- Decommission the incumbent only after a time-boxed exit with no open incidents, proposed at 10 business days. Missing the date counts as an incident.
- Rollback authority: the owner, with a named on-call contact.

Sources: https://www.reworked.co/information-management/what-legacy-migration-looks-like-today/ , https://www.easy.bi/blog/parallel-run-legacy-migration-playbook/ , https://www.testdevlab.com/blog/legacy-system-modernization-qa-migration-checklist , https://us.fitgap.com/stack-guides/plan-a-phased-migration-with-clear-cutover-gates-and-fast-decommissioning , https://ferdinebi.github.io/software-engineering-with-ai/deployment-launch/data-migration-cutover/

## 6. Support model

Proposal: business-hours support for the first slice, with one named owner. Sev 1 (no run can start, or possible data loss) gets a response within 4 hours on working days. Sev 2 gets the next business day. Keep a restore runbook and the on-call contact in the repository.

Caveat: the research found no source for response times. These numbers are the assistant's proposal, not an industry standard.

## What the owner needs to answer

For each item above: accept, change, or reject. Only accepted items become owner decisions. The incumbent name (item 3) is the one thing the assistant cannot propose.
