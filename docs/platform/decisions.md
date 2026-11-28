# Open decisions and decision log

No items below should be treated as approved solely because they are documented. The product owner and engineering owner should record consequential choices with rationale, alternatives, consequences, date, and supersession history before implementation.

## Product and governance

1. **Platform scope:** Which concrete business capabilities belong in Plaesy Platform beyond the current memory/knowledge engine? Define module boundaries and measurable outcomes.
2. **Tenant and ownership model (implementation gate):** choose and document one hierarchy (single-user instance or organization/workspace/project), resource ownership/transfer, principal identities, and minimum authorization rules together. This blocks shared/remote schema, API and collaboration implementation; local-only work may proceed only if it does not assume or expose the unresolved model.
3. **First-release scope:** product owner selects the first deliverable and explicit exclusions before acceptance baselines are frozen; target-platform breadth is not a release commitment.
4. **Licensing and edition:** open-source/community/enterprise split, contribution policy, support model and feature availability.
5. **Telemetry and hosted services:** default telemetry (recommended: none/content-free opt-in), update check behavior, hosted registry/identity services, offline guarantees.
6. **Supported profile:** exact OS, CPU architecture, VPS distributions and support window.

## Architecture

1. **API framework/protocol:** HTTP/JSON, gRPC or other; schema format and compatibility guarantees.
2. **Local runtime/supervisor:** foreground-only initially or managed background service; per-OS service manager approach and privilege boundaries.
3. **Web/Desktop stack:** packaging, frontend delivery, Desktop framework, signing/notarization and update channels.
4. **Persistence:** database choice per profile, object/blob storage, search/index backend, transaction boundaries and migration tooling.
5. **Current memory engine integration:** embedded module/process, API service, or separately managed local daemon. Validate single-user SQLite constraints against desired concurrency and remote tenancy.
6. **Remote control:** direct authenticated API, SSH-based provisioning adapter, remote agent, or provider API; define trust bootstrap and credential rotation.
7. **Deployment packaging:** approve or reject Docker Compose as the first-class single-VPS profile; supported alternatives and provider matrix.
8. **Extensions:** plugin registry, package signing, sandbox standard, permission manifest and review/governance model.
9. **Multi-tenancy and scaling:** resolve consistently with the tenant/ownership implementation gate above; do not freeze shared-instance schema or cross-project behavior until the hierarchy is approved.

## Security and operations

1. **Identity and access control (implementation gate):** choose identity authority and bootstrap/recovery model, principal types and minimum resource authorization together with the tenant/ownership hierarchy. Specify MFA/SSO/SCIM scope if applicable; do not infer it from account identifiers.
2. **Encryption and keys:** at-rest coverage, key storage/rotation/recovery, external KMS support and backup key custody.
3. **Retention/deletion:** audit retention, user data export/deletion, backups/versioned copies and erasure statements.
4. **SLOs and support:** supported scale, service objectives, severity response, security update cadence and maintenance policy.
5. **Backup service:** included tooling only vs supported external destinations; consistency hooks, retention and restore target.

## CLI contract

1. **Command grammar:** implementation gate for a stable public contract. Choose and document one collaboration namespace (`plaesy project ...` or `plaesy collab ...`) and align it with existing commands; keep remote Web provisioning under `plaesy web deploy ...` if approved. Confirm exact verbs, target selection, help and error/exit behavior before freezing CLI/API compatibility.
2. **Config precedence:** names and precedence for flags, environment, profiles, config files, secrets and project-level config.
3. **Compatibility:** API/CLI/server support window, JSON schema stability, deprecation period and migration strategy.

## Agent collaboration

1. **Scope and storage:** decide whether cross-session agent collaboration is local-only, self-hosted shared service, or both.
2. **Agent runtime integration:** clarify whether Plaesy runs agents or coordinates external agent runtimes.
3. **Shared state:** approve task, message, session-summary and presence/lease resources and their access rules.
4. **Concurrent writes (implementation gate):** name the engineering owner and define the enforced write boundary before implementing shared mode: which agent/editor/tool writers are mediated, how expected-revision/CAS is atomic, how out-of-band writers are fenced or detected, and what fail-closed behavior applies if the coordinator is unavailable. Shared mode must be disabled or explicitly degraded with a visible warning whenever its approved guarantee cannot be enforced; no silent last-writer-wins. Preserve external-writer races as an explicit residual risk if they cannot be fenced.
5. **Delivery/runtime (release gate):** select supported adapters and verify safe interrupt points, queued delivery, acknowledgement, idle/terminated session handling, deduplication, authorization recheck and revocation behavior. Do not promise live injection until conformance tests pass for each supported adapter/profile.
6. **Git and leases (implementation gate):** choose serialization/fencing for shared index, HEAD/branch/worktree operations, monotonic lease generations, expiry/recovery, offline behavior and authorized override. Do not discard unintegrated edits on expiry/crash.
7. **Conflict UX (implementation gate):** explicitly shared same-file work requires registered intent and visible notification; same-line/semantic conflicts preserve both proposals and block automatic application. State whether user confirmation is required before overlapping edits; default to confirmation unless a project owner authorizes the shared-file collaboration policy. Isolated worktrees remain the default otherwise.
8. **Deterministic test set (implementation gate):** engineering owner must select and automate at least same-base concurrent writes, overlapping patch, coordinator-outage/fail-closed, external-writer race at apply, crash between write and event, stale lease fencing, event loss/reconnect/idempotent retry, Git index collision, permission revocation with queued delivery, and safe-point vs queued/idle/terminated runtime behavior. Record hashes, delivery states, audit evidence and residual limitations. Convert remaining failure simulations to release tests before advertising support.
9. **Privacy and retention:** define what session summaries/artifacts are shared, message retention, project ACLs, notifications and deletion.
10. **Automation authority:** decide if Plaesy can create branches/commits/merge, and which roles must approve integration.

## Direction recorded from product owner (2026-11-16)

The owner wants Plaesy to be better than, and ultimately able to replace, Hermes Agent, Claude Code, Codex, Paperclip, Orca, and similar agent/coding/orchestration products. The owner explicitly permits Plaesy to use Claude Code, Codex, and comparable agent harnesses as execution backends. This establishes product direction and a provider/harness-integration permission, not an implementation claim, approved release scope, or permission to terminate existing services before replacement capability and migration are verified.

1. **Product ownership boundary:** Plaesy should own the user experience/control plane, orchestration and durable workflow state, policy and approvals, project/task context, memory/provenance, audit, and backend selection. It may delegate execution to external harnesses (including Claude Code and Codex) or model APIs through replaceable adapters. Backend use must be explicit, observable, permission-scoped, and not silently treated as Plaesy-native capability. Plaesy should remain useful with supported alternatives; avoid hardwiring core state or user data to one backend.
2. **Backend support strategy:** decide which backends and versions are supported first, their authentication/credential boundaries, tool/filesystem permissions, cancellation and event semantics, cost/usage reporting, and degraded/offline behavior. Verify each adapter against its real contract; do not claim equivalent behavior across backends without conformance evidence.
3. **Replacement and migration acceptance:** inventory the user's real workflows for each named tool, define observable success/quality/reliability/cost criteria, data/config import needs, cutover order, parallel-run period, rollback, and a no-loss migration policy. Do not recommend disabling an incumbent before its critical workflows pass in Plaesy.
4. **First usable slice:** select one end-to-end workflow and target profile for an initial non-production vertical slice; name a primary user, permitted action boundary, and success evidence. Use synthetic data and keep production access/cutover gated.

## Recommended decision sequence

Resolve product/module scope and tenant/ownership hierarchy with identity/authorization together first. These are implementation gates for shared/remote APIs and data contracts. Next approve enforced shared-write boundary and fail-closed/degraded mode, overlap/confirmation semantics, Git/lease protocol, one CLI namespace and the deterministic pre-implementation test set. Those gates block implementation of the affected shared collaboration, schema/API and stable CLI contracts; isolated local-only work may proceed only if it does not bake in unresolved assumptions. Runtime adapter conformance, full fault-injection coverage, recovery drills, accessibility and data lifecycle evidence are release gates for the capabilities they cover; live injection/shared collaboration must not be advertised before required conformance evidence exists. Every gate needs named accountable product/engineering/security owner, chosen option, rationale, affected components, test/evidence, and approval date. Record decisions in dated ADRs and link them here. Until authorized owners approve, unresolved decisions remain open; recommendations here do not silently select policy on their behalf.

## Design-code IR (ADR 0009 and 0010)

Status: ADR 0001 (design-code IR) and ADR 0007 (design IR) are superseded by ADR 0010 (design IR and design-code conversion, proposed). Cross-IR conventions are in ADR 0009 (proposed). Spec R15 and task `high_chore-decide-design-code-ir-schema-and-target-adr` govern this record. The ADR index and reading order are in `docs/adr/README.md`.

- **IR schema version:** `design-ir/1` (proposal, ADR 0010). Version rules are in ADR 0009: MAJOR for breaking changes, MINOR for additive fields.
- **Node set:** frame, rect, text, image, vector, instance, and opaque (ADR 0010). Auto-layout and style are on the node. Unsupported constructs become `opaque` nodes with their source location.
- **Reuse:** the token model in `crates/plaesy/support/designlint/src/designlint/` is the starting point for token types. Decide per type whether it is reused, extended, or replaced when Phase 3 of `docs/adr/implementation-plan.md` starts.
- **Lowerings:** design → IR, IR → design, IR → code, and code → IR. Each direction reports lost constructs in the loss-report form defined in ADR 0009.
- **Edit policy:** guarded merge (option B), inherited from ADR 0001 under the owner's delegation of 2026-11-18. Edits outside managed regions are preserved and not parsed.
- **Fidelity:** structural round trip must be 100% on a corpus of at least 20 fixtures (node IDs, hierarchy, token values). Corpus content is still open with the owner and QA. No round-trip result may be reported as passing before the corpus exists.
- **Targets:** source design format and first code target are still open with the owner. No design tool or code framework is approved yet.
- **Rust:** the IR and converters are core logic and must be Rust, per R14.

## Visual flow IR (ADR 0002, partly accepted)

Status: accepted on 2026-11-18 in chat by the owner for the node catalogue, CEL expressions, retry formula, loop limits, approval channel, and Rust as the first codegen adapter. The JSON document format and validation rules were accepted under the delegation below on 2026-11-21. Version `flow-ir/1`. Spec R16 and tasks `high_chore-decide-visual-flow-ir-and-codegen-adr` and `high_feature-implement-visual-flow-editor-and-codegen` govern this record.

## Delegated architecture decisions (2026-11-21, revocable by the owner)

The owner's standing delegation of 2026-11-19 covers open technical choices in these records. Each item below is recorded as delegated, not as personal owner approval. The owner may revoke any item; a revocation reopens the affected ADR and its phase in `docs/adr/implementation-plan.md`.

1. **Flow JSON format and validation rules (ADR 0002, "Format"):** accepted as proposed. Reopen if the owner wants a different serialization or rule set.
2. **Cross-IR conventions (ADR 0009):** `extensions` on every closed object, closed root schemas, prefix table, single loss-report shape, input limits 5 MB and depth 64. Lenient reader mode is not implemented.
3. **Design token format (ADR 0010):** W3C DTCG 2025.10. The schema and examples move to DTCG in Phase 3; until then the repo examples still use the internal form and are treated as lagging.
4. **Design node kinds (ADR 0010):** frame, rect, text, image, vector, instance, opaque.
5. **First design source (ADR 0010):** the Plaesy JSON design format (`design-ir/1` itself). No third-party design tool is selected; this avoids a vendor format dependency.
6. **First code target (ADR 0010):** Rust with Dioxus components, consistent with the first codegen adapter in ADR 0002.

7. **Structure and scope ADRs (0003, 0004, 0005, 0006, 0008, 0011):** accepted as proposed, with their open items kept open. ADR 0003 (crate layering), 0004 (data IR, SQL dialect still open), 0005 (API IR, OpenAPI 3.1 output), 0006 (app IR), 0008 (code-to-visual import, first language and parser open), 0011 (client structure, WebSocket library and diff format open).

Implemented in `crates/plaesy/adapter/design-ir/src/designir/` (design-ir/1) and `crates/plaesy/adapter/design-ir/src/irvalidate/` (semantic validators for flow, data, api, and app; lenient reading for all five IRs): semantic validator, Rust module codegen, structural round trip (IDs, hierarchy, kinds, scope, token values), and lenient reading. Covered by `cargo test --locked --lib designir` (10 tests). Added 2026-11-21 with owner permission ("ya boleh, gunakan versi yang stable"): Dioxus 0.7.10 stable, optional feature `designir-dioxus`, for codegen of components with `rsx!`. Rendering of the Dioxus components is tested with dioxus-ssr 0.7.10 (same version). Still open: CI for the checks (deferred by the owner) and a corpus from real designs. The 20-fixture corpus in `docs/adr/corpus/` is synthetic.

## Architecture ADR index (2026-11-21)

Full index, status meanings, and reading order: `docs/adr/README.md`. Summary of status for the IR and crate decisions:

- ADR 0003 (crate structure): accepted under delegation (2026-11-21, revocable). Client structure split to ADR 0011.
- ADR 0004 (data IR), 0005 (API IR), 0006 (app IR), 0008 (code-to-visual import), 0011 (client structure): accepted under delegation (2026-11-21, revocable); open items remain in each ADR.
- ADR 0009 (cross-IR conventions): accepted under delegation (2026-11-21, revocable). Includes the `extensions` rule and closed root schemas.
- ADR 0010 (design IR): accepted under delegation (2026-11-21, revocable): DTCG token format, node kinds, JSON design source, Rust/Dioxus first target. DTCG implementation and the 20-fixture corpus are Phase 3 work.
- ADR 0011 (client structure): proposed.
- ADR 0002 (flow IR): accepted in chat for the decisions; JSON format and validation rules accepted under delegation (2026-11-21, revocable).

## Owner decisions for the first replacement slice (2026-11-19)

Source: product owner, answered in the Nimbalyst decision prompt on 2026-11-19. These are authorized decisions, not recommendations. Recommendations offered in the prompt were not all selected.

1. **First workflow (spec open question 1):** agent execution and learning workflow. Links: R3, R4, R9. Task `high_feature-deliver-selected-agent-execution-and-learning-workflow` is the workflow task. Its child spec is still required (see the vertical-slice task).
2. **Deployment profile (spec open question 2):** shared self-hosted. Links: R6, R7 (shared messaging is in scope). Local-first and offline requirements from `docs/platform/product.md` still apply. Remote trust boundaries must be defined explicitly. Lane C (shared messaging) is therefore in scope for the first slice.
3. **Runtime strategy (spec open question 3):** deterministic fake adapter first, then a bounded native runtime. External harness orchestration (Claude Code, Codex) is not enabled in the first slice. Links: R4, R9. Tasks: `high_feature-define-runtime-adapter-and-fake-conformance-suite`, `high_feature-implement-bounded-supervised-runtime-execution-and-usage-reporting`.
4. **Identity hierarchy (spec open question 4):** single organization. Agent principals are scoped per project, with grants and revocation. Links: R5. Multi-tenant hierarchy is out of scope for the first slice.

**Remaining UNKNOWN at the time of this entry (2026-11-19):** target cohort and success outcome; incumbent baseline workflow and pinned version; NFR thresholds (performance, reliability, security, retention); migration and cutover appetite, rollback authority; support model; exact DeepSeek Harness and Orca identity and version. These are superseded only where explicitly delegated below; other items remain UNKNOWN.

## Owner decision status: delegated decisions and remaining unknowns (2026-11-19)

The owner delegated these choices to the assistant, revocable at any time. Record them as delegated defaults, not as personal owner approval. The detailed values and rationale are in [Delegated decisions](#delegated-decisions-2026-11-19-revocable-by-the-owner).

| Item | Status | Delegated value | Reopen trigger |
| --- | --- | --- | --- |
| First-slice target cohort | Delegated, revocable | 3–5 people already running agent workflows on the same project; one operator is primary | Owner revokes or changes cohort |
| Success outcome | Delegated, revocable | 100 synthetic runs, zero false successes, all ten failure injections pass, and one real recovery after injected restart | Owner revokes or changes success criteria |
| Incumbent baseline for agent-execution comparison | Delegated, revocable | Hermes Agent at commit `40d1e95f37e409f9f618f32c170661c66b86a4f4` | Owner names a different incumbent or revision |
| Control-plane availability | Delegated, revocable | 99.9% over 28 days | Owner revokes or changes NFR |
| Read latency | Delegated, revocable | p95 below 200 ms, using threshold-count SLIs for error budget | Owner revokes or changes NFR |
| Run-state recovery | Delegated, revocable | RPO ≤1 hour; RTO ≤4 hours; monthly restore drill | Owner revokes or changes NFR |
| Parallel run | Delegated, revocable | Two weeks; pre-written rollback criteria and variance limits; reconcile each batch and before cutover | Owner revokes or changes cutover plan |
| Replacement/integration/exclusion for other named products | UNKNOWN until each audit task closes | None | At each `high_chore-audit-*` close |
| Support model | Delegated, revocable | Business hours, one named owner; Sev 1 response within 4 working hours; Sev 2 next business day | Owner revokes or changes support model |
| Exact DeepSeek Harness and Orca identity/version | Delegated, revocable in section below | DeepSeek Harness `deepseek-ai/deepseek-harness`, npm `@deepseek-ai/dsh`, `v0.2.2-rc.5`; Orca `stablyai/orca`, desktop `v1.4.263` | Evidence contradicts identity/version or owner revokes |
| Legal/compliance retention | UNKNOWN | No delegated choice | Counsel input required |
| UI and Rust boundary beyond decided Dioxus web surface | UNKNOWN | No delegated choice | Before desktop/mobile planning |

Decided by the owner on 2026-11-19: first workflow, deployment profile, runtime strategy, and identity direction (section above). The delegated choices do not authorize incumbent shutdown, production cutover, or irreversible data migration; those remain gated by the cutover criteria and explicit authorization.

**Catatan pencatatan (2026-11-26):** Item berstatus "Delegated, revocable" di atas dipakai berdasarkan delegasi pemilik yang tercatat di bagian "Delegated architecture decisions" dan "Delegated decisions". Gerbang pencatatan di "Open decisions" (baris 64) meminta pemilik, pilihan, alasan, komponen terdampak, bukti tes, dan tanggal. Kolom-kolom itu belum lengkap untuk setiap item; melengkapinya adalah pekerjaan berikutnya, bukan syarat untuk memakai nilai delegasi.

Item yang masih UNKNOWN (baris 133, 136, 137) tidak diisi oleh assistant. Butuh keputusan pemilik atau masukan legal.

## Comparison identity evidence (2026-11-19, evidence only, not a decision)

Gathered for spec open question "exact DeepSeek Harness and Orca product/version". The owner still picks the comparison target.

- **Orca:** the GitHub Releases page for `stablyai/orca` (fetched 2026-11-19) lists desktop tags up to `v1.4.263` (18 Nov 2026). An earlier secondary source cited `v1.4.180` (Aug 2026). That is superseded. Name clash: `araa47/orca` is a different CLI project. Source: https://github.com/stablyai/orca/releases
- **DeepSeek Harness:** a news article (RuntimeWire, 14 Aug 2026) calls it "Harness v0.1". A separate article describes it as an open-source, plugin-based coding agent. The MIT license and the `dsh` / `@deepseek-ai/dsh` package name appear only in a search summary and are not confirmed by the articles fetched. Sources: https://runtimewire.com/article/deepseek-open-sources-agent-harness-v4-pro-api-price-hike and https://www.mindstudio.ai/blog/deepseek-harness-coding-agent/
- **Next step (owner):** confirm the exact repository and version to compare. Until then the item stays UNKNOWN.

## Default technical decisions applied (best practice, 2026-11-19, changeable by owner)

The owner asked for best-practice defaults for open technical choices. These defaults are engineering choices, not owner approvals. At the time, business items (cohort, success outcome, baseline, NFR thresholds, cutover, support, DeepSeek/Orca) were not covered and stayed UNKNOWN; the later delegated decisions below supersede those statuses where specified.

- **Native runtime:** bounded child-process adapter. Only programs on an explicit allowlist run. The environment is cleared except PATH, SYSTEMROOT, TEMP, TMP. stdin is closed. stdout and stderr are capped at 64 KiB each. Timeout and cancellation kill the process. External effects are off by default. This is a host process, not a sandbox. Container or VM isolation is required before real tools run.
- **Key distribution:** one HMAC key per deployment and key id, stored in the OS credential store under one service name (`KeyringKeySource`). Keys never appear in envelopes, logs, or the repository. Rotation adds a new key id and retires the old one.
- **Envelope freshness:** every envelope carries a send time and a random nonce. The receiver refuses nonces seen in the window and envelopes older or newer than 300 seconds. 300 seconds is a proposed default.
- **Transport:** HTTPS at the edge (TLS 1.2 or later, terminated by a proxy or the rustls stack), with the HMAC envelope on top as defense in depth. Implemented in `crates/plaesy/adapter/relay/src/relay/transport.rs`: one route `POST /v1/messages`, loopback bind by default, and a non-loopback bind is refused unless the caller states TLS is terminated upstream. Recipient is signed in the envelope. Presence is not observed, so messages are queued, never reported as read.

## Delegated decisions (2026-11-19, revocable by the owner)

The owner delegated all remaining decisions to the assistant: "semuanya otomatis, tidak ada yang menunggu keputusan user". Each decision below is recorded as delegated, not as a personal approval. The owner may revoke any of them, and a revocation reopens the affected work.

1. **Target cohort:** 3 to 5 people who already run agent workflows on the same project, with one operator as primary user. Source of the proposal: `docs/platform/proposed-defaults-2026-11-19.md`, section 1.
2. **Success outcome:** 100 synthetic runs with zero false successes, all ten failure injections passing, and one real recovery after an injected restart. Section 2 of the same document.
3. **Incumbent baseline:** Hermes Agent, pinned at commit `40d1e95f37e409f9f618f32c170661c66b86a4f4`. Basis: the repository already holds the most complete evidence for this product (`docs/platform/hermes-capability-audit.md`, `hermes-api-acceptance.md`), and it is listed first in the spec. The repository cannot show which product the owner uses day to day. If that differs, revoke this decision and name the product.
4. **Comparison targets:** Orca `stablyai/orca` at desktop tag `v1.4.263` (18 Nov 2026). DeepSeek Harness at `deepseek-ai/deepseek-harness`, npm `@deepseek-ai/dsh`. The latest published tag seen on 19 Nov 2026 is `v0.2.2-rc.5` (pre-release, 18 Nov). The baseline pins the exact tag when it is taken.
5. **NFR thresholds:** control-plane availability 99.9% over 28 days. Read latency under 200 ms at p95, using threshold-count SLIs for the error budget. RPO at most 1 hour and RTO at most 4 hours for run state. Monthly restore drill. Run-log and evidence retention 30 days.
6. **Cutover:** no big-bang cutover. Parallel run for 2 weeks. Rollback criteria and variance limits are written before the run starts. Reconcile after each batch and before cutover. Two or three rehearsals. Incumbent decommissioned 10 business days after a clean exit.
7. **Support model:** business hours for the first slice, one named owner. Sev 1 gets a response within 4 hours on working days. Sev 2 gets the next business day.

Not decided here: legal or compliance retention, which needs counsel input, and any product or pricing choice not listed above.

## Keputusan teknis default (2026-11-22, dapat diubah pemilik)

Keputusan ini diambil tanpa persetujuan pemilik, sebagai default teknis. Pemilik dapat mengubahnya kapan saja.

- **Web (interface/client):** Dioxus 0.7.10, dijalankan dengan `dx serve --platform web --package plaesy-interface-client --example web --features web` dari root workspace. Titik masuk web adalah `main` contoh yang memanggil `platform::web::run()`. Verifikasi render dilakukan di browser (DOM berisi judul, modul, alur data, dan masalah lokal).
- **Model embedding vault:** `multilingual-e5-small` (`model.onnx` dan `tokenizer.json` dari `intfloat/multilingual-e5-small` di Hugging Face, sumber yang sama dengan `onboard/mod.rs`), disimpan di `<cache>/plaesy/models/multilingual-e5-small`. Semantic search aktif setelah indeks vektor dibangun dengan `plaesy memory index`.
- **Mode tulis vault:** `plaesy-vault` didaftarkan tanpa `--read-only` agar daemon memegang lock tulis. Penulisan diverifikasi pada vault scratch, bukan vault pemilik.
- **OpenAI:** diaktifkan atas persetujuan pemilik (2026-11-22). Ini melanggar aturan CLAUDE.md "runtime offline": penyedia `openai` sekarang memanggil jaringan saat dijalankan. Aturan CLAUDE.md perlu direvisi agar konsisten.

## Target pengganti tambahan (disetujui pemilik, 2026-11-22)

- **Orchestrator.inc (Agent Orchestrator):** https://github.com/OrchestratorInc/agent-orchestrator, lisensi Apache-2.0. Target pengganti.
- **Orca (Stably AI):** https://github.com/stablyai/orca. Target pengganti, sudah tercatat sebagai incumbent.

Ejaan "orcharstrator.inc" dari pemilik dipetakan ke Agent Orchestrator. Kepemilikan dan entitas hukumnya belum diverifikasi (lihat `docs/platform/feature-inventory/orchestrator-candidates.md`).

## Daftar target pengganti (diperbarui pemilik, 2026-11-22)

Target pengganti yang berlaku, dengan inventaris fitur di `docs/platform/feature-inventory/`:

- Hermes Agent: https://github.com/NousResearch/hermes-agent
- DeepSeek Harness: https://github.com/deepseek-ai/deepseek-harness
- Orca: https://github.com/stablyai/orca
- Agent Orchestrator (Orchestrator.inc): https://github.com/OrchestratorInc/agent-orchestrator
- OpenBot: https://github.com/nightly-labs/openbot
- OpenProject: https://github.com/opf/openproject
- Paperclip, Nimbalyst, Craft Anything, FigCraft, DeckCraft, WordCraft, Herald OS: sesuai ADR 0003 dan inventarisnya.

## Aturan runtime jaringan (disetujui pemilik, 2026-11-23)

Runtime boleh memakai jaringan jika sebuah fitur membutuhkannya. Aturan offline di CLAUDE.md digantikan. Setiap jalur jaringan harus eksplisit, aktif hanya lewat konfigurasi, dan tercatat di dokumen ini. Fitur lokal (memori, task, board, ekspor) tetap lokal secara default.

## Workflow pertama: pengganti Orca (disetujui pemilik, 2026-11-24)

- Pemilik memilih Orca sebagai incumbent pertama yang diganti. Alasannya: pemilik perlu memindahkan skill dan konfigurasi per provider tanpa menunggu keputusan tambahan.
- Langkah pertama: inventaris skill dan konfigurasi per provider. Hasilnya: Orca tidak menyimpan skill sendiri (`~/.orca` hanya berisi `agent-hooks/` dan `keybindings.json`). Skill ada di `~/.claude/skills`, `~/.codex/skills`, `~/.agents/skills`, dan `~/.hermes/skills`. Plaesy belum punya fitur sinkron skill antar provider.
- Langkah berikutnya: task `plaesy skills` (inventaris dan sinkron skill antar provider). Implementasi ada di crate baru dengan `cargo check -p`.
- Batas: penggantian Orca penuh tetap bertahap. Inventaris `docs/platform/feature-inventory/orca.md` mencatat 437 dari 451 perilaku belum ada di Plaesy. Incumbent tetap berjalan sampai gerbang cutover lulus.

## Lokasi skill Plaesy (disetujui pemilik, 2026-11-24)

- Skill Plaesy disimpan di `~/.plaesy/skills` secara default. Override, prioritas tertinggi dulu: flag `--dir` pada `plaesy skills list|sync`, lalu env `PLAESY_SKILLS_DIR`, lalu `skills.dir` di `~/.plaesy/settings.json`, lalu default.
- `plaesy skills sync` tidak menimpa skill yang sudah ada di target. Tanpa `--apply` hanya menampilkan rencana. Symlink tidak disalin.
- Status: kode di `crates/plaesy/interface/cli/src/cli/skills.rs`, commit `0686f60` di `main`, sudah di-push ke `origin/main` (`e7e0d60`). `cargo check` dan `clippy -p plaesy-interface-cli --locked` lulus. Tes unit `skills` ada di `skills_test.rs` (13 tes lulus, `cargo test -p plaesy-interface-cli skills`). Resolusi `--dir` > `PLAESY_SKILLS_DIR` > `settings.json` > default diuji sebagai fungsi murni `resolve_plaesy_dir`.
- Status `plaesy chat` (slice 1 dari rencana pengembangan): `cli/chat.rs` dan `chat_test.rs` ada, 8 tes lulus, clippy `-D warnings` lulus. Belum di-commit dan belum diuji manual terhadap provider nyata. Riwayat dirender ke prompt karena trait `Provider` single-shot. Provider belum mendukung multi-turn native.
- Slice 2 (multi-turn native): trait `Provider` punya `complete_messages` dengan default yang meratakan pesan, jadi provider CLI tidak berubah. Anthropic, OpenAI-compatible, dan Ollama (sekarang `/api/chat`) mengirim seluruh riwayat sebagai `messages`. Body dan parsing balasan diuji sebagai fungsi murni. Belum diuji dengan server nyata. `plaesy chat` memakai `run_messages`.
- Slice 4a: `/decision <judul> | <pilihan> | <alasan>` di `plaesy chat` menyimpan catatan memori bertipe `decision` di folder `decisions` lewat `plaesy memory remember` (flag `--vault` opsional). Diuji lewat perekam yang bisa diganti (15 tes chat lulus) dan lewat binary yang dibangun ulang, dengan vault sementara: catatan tertulis di `decisions/`. Di-commit di `feat/chat-decision` (`c75d272`). `--type decision` mewajibkan `--id` tidak kosong; sudah diberi id dari hash judul dan waktu. `/pending` dan gerbang kebijakan belum dibuat.
- Binary terpasang `%LOCALAPPDATA%\Plaesy\bin\plaesy.exe` (22 Nov 2026) belum memuat `skills` maupun `chat`. Tidak ada `target/debug/plaesy.exe` di checkout ini. Untuk mencoba perubahan ini, build `plaesy` lalu pakai binary dari `target/`. Jangan timpa binary terpasang sebelum `plaesy mcp` dihentikan (lihat CLAUDE.md).

## Jaringan di `plaesy chat` (dipindah ke settings atas persetujuan pemilik)

- `web_fetch` aktif hanya jika `"chat": {"web": true}` ada di `.plaesy/settings.json` (repo) atau `~/.plaesy/settings.json`. Default mati. Repo menang atas pengguna. Aturan URL sama dengan webhook: `https` untuk host mana pun, `http` hanya loopback. Tidak ada web search; hanya ambil satu URL.
- Ini satu-satunya alat `chat` yang memakai jaringan. `--tools` sekarang aktif secara default; `--no-tools` mematikannya.

## Format konfigurasi: TOML (disetujui pemilik, 2026-11-25)

- Konfigurasi proyek pindah dari `.plaesy/settings.json` ke `.plaesy/config.toml`. Pemilik memilih TOML, bukan mengganti nama tetap JSON, karena TOML mendukung komentar.
- Pembacaan: `config.toml` didahulukan. `settings.json` dibaca hanya bila `config.toml` belum ada. Helper `delegate::config_path` dan `delegate::settings_path` adalah satu-satunya tempat path ini dibentuk.
- Penulisan pertama (`/model`) menyalin `settings.json` ke `config.toml`, dan `settings.json` tidak diubah. Nilai `null` tidak bisa dikonversi dan ditolak dengan pesan jelas.
- Status: baru `chat` dan `delegate` yang memakai helper ini. Crate lain (memory, skills, telemetry, reach, schedule, dan sebagainya) masih membaca `settings.json`. Sampai migrasi itu selesai, mengedit `settings.json` tidak memengaruhi `delegate` setelah `config.toml` ada.

## Agent-to-agent transport default (2026-11-26, didelegasikan, bisa dicabut pemilik)

- Default: pesan antar agent tetap lewat relay lokal (`plaesy relay`) dengan listener di `plaesy chat` (mode line dan `--tui`). Relay dan listener sudah diuji dengan dua sesi chat dan dengan pseudo-terminal.
- Plaesy tidak menjadi server A2A untuk sekarang. A2A (Agent Card, JSON-RPC/REST, task streaming) dicatat sebagai opsi untuk jalur antar-mesin. Belum ada keputusan untuk mengadopsinya.
- Pemilik dapat mencabut default ini. Pembukaan A2A mengubah endpoint, autentikasi, dan Agent Card, jadi perlu keputusan terpisah.

## Token MCP per klien (didelegasikan, 2026-11-27, bisa dicabut pemilik)

- Desain di `docs/platform/mcp-multi-token.md`. Satu token per label (`plsy_<id>_<secret>`), disimpan sebagai `sha256(salt || token)`, tidak pernah plaintext. Berkas di `~/.config/plaesy/mcp-tokens.toml`, di luar vault.
- Muat ulang saat mtime atau ukuran berubah, dicek paling sering tiap 2 detik. Tanpa restart. Tidak memakai SIGHUP karena klien laptop berjalan di Windows.
- Masa tenggang rotasi default 24 jam. Berkas yang gagal diurai mempertahankan himpunan token terakhir. Berkas yang hilang atau kosong setelah pernah terlihat menolak semua permintaan.
- Tanpa penguncian global, karena server berada di belakang tunnel loopback dan semua klien terlihat dari alamat yang sama.
- Pilihan yang dapat dibalik: perintah berada di `plaesy memory token`, bukan `plaesy mcp token`, karena `plaesy mcp` sudah dipakai server proyek.
