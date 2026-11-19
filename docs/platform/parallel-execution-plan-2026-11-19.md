# Parallel execution plan (2026-11-19)

Status: plan, prepared by the assistant under the owner's delegation. It changes how work is scheduled, not what is decided. Revocable by the owner.

## Why the board waits today

- Most "blocked" cards wait on dependency links that are not needed for the code. Example: each product audit waits for other audits, though each writes its own file.
- Many implementation cards describe modules that already exist in the repository with tests (identity, authz, adapter, runtime, runstore, task_run, policy, secrets, relay shared/auth/keys/transport). Their remaining work is acceptance evidence, not new code.
- Owned paths are disjoint across the active implementation cards, so they can run at the same time.

## Lanes that can run in parallel now

| Lane | Cards | Owned paths | Conflict risk | Resource |
| --- | --- | --- | --- | --- |
| A. Audits (11 products) | one card per product | `docs/platform/audits/<product>.md` (new) | none | docs only |
| B. Slice verification | identity, adapter, runtime, runstore, task_run, policy/secrets, relay | existing module paths | none | cargo, serialised |
| C. Install lifecycle | install lifecycle card | `tests/install_e2e.rs`, `scripts/install.*` | none | host, no build |
| D. Hardware baseline | hardware card | `docs/platform/measurements/` (new) | none | host measurement |
| E. Quality gates | security/release gates card | `rust-toolchain.toml`, `deny.toml` (new) | none | small |
| F. Visual flow | IR core, codegen adapter, Dioxus editor | `src/visual_flow/`, `crates/editor/flow/` | codegen waits for IR core | cargo, serialised |

Lanes A, C, D, and E do not compile Rust and can run together. Lane B and F compile Rust and must share one build cache.

## Cards to leave alone for now

- Deferred, out of the first slice: provenance, migration, the 9 "deliver selected" workflows except agent execution, design-code round trip, workspace isolation (single writer in this slice).
- Owner-gated: threat model review for identity, real backend choice for the runtime, and the operational restore drill.

## Resource rules

- The server has 2 vCPU and about 14 GB free. Run at most two cargo jobs at once.
- Share one build cache: `CARGO_TARGET_DIR=/home/ubuntu/.cache/plaesy-target`. Do not create a `target` directory per worktree.
- Worktrees stay under `~/cli/.worktrees/`, because the sync to the PC is intentional.

## Coordination between agents

- Cards communicate through comments on the board, addressed to the card id.
- A card that finishes unblocks its children through the board links. Remove links that do not reflect a real dependency.
- Each agent records its evidence (test output, commit hash) in its card before it marks the card done.

## Next steps

1. Remove links between product audits, keeping only the link to research.
2. Move lane A, C, D, E cards to todo with their own profile. Give each profile one lane.
3. Run lane B and F after the build cache is shared.
4. Keep the owner-gated cards blocked, with the reason in a comment.
