# Task dispatch readiness

Use this gate before assigning one worker to a task. A valid `contracts.json` proves only that the file parses; it does not prove that the task is ready or safe to run in parallel.

## Required before dispatch

1. **Status and ownership:** The task is in `todo` (or an explicitly coordinated `doing` task), and its `issue.json.status` matches its status directory. Do not treat `backlog`, `review`, or `done` as dispatchable.
2. **Dependencies:** Every `depends_on` task exists and is complete, or the dependency is explicitly informational/decision-gated and the task says what work is safe while it remains open. A blocked dependency means do not dispatch dependent implementation.
3. **Decision gate:** Any owner decision that affects scope, architecture, provider choice, security boundary, or acceptance is approved. If not, keep the task in backlog or split out a bounded discovery task that records unknowns without assuming an answer.
4. **Exclusive write-set:** `task.md` names the exact exclusive paths the worker may create/edit, plus shared/integrator-only paths it must not edit. “Related files” or a broad repository root is not an adequate boundary. No two concurrent tasks may claim intersecting paths.
5. **Verification:** `contracts.json` has checks for the task's actual deliverables and focused behavior, not just file existence or syntax. Use the narrowest relevant test/build command; do not invoke workspace-wide gates for a delegated task unless the task owner approves it.
6. **Execution isolation:** Run each worker in its own worktree/branch. Do not use `plaesy tasks run --parallel` for tasks that share a working tree or have overlapping write-sets. Use task claims to prevent duplicate assignment; claims do not prevent path conflicts between different tasks.

## Shared paths

Treat workspace manifests/lockfiles, crate/module registries, binary entry points, CI workflows, toolchain pins, generated global indexes, and canonical cross-task inventories as integrator-owned unless a task explicitly receives a narrow exclusive ownership transfer. Workers should report required shared-file changes rather than editing them directly.

## Readiness outcome

Record one of `READY`, `BLOCKED`, or `NOT PARALLEL-SAFE` in the dispatch decision:

- `READY`: dependencies and decisions are resolved; write-set is explicit and non-overlapping; acceptance checks are executable and focused.
- `BLOCKED`: a dependency, owner decision, or required input is outstanding.
- `NOT PARALLEL-SAFE`: scope is broad/ambiguous, paths overlap, or required shared files have no integrator boundary.

Do not infer readiness from task count, status alone, or successful structural contract validation. Re-audit the exact task and current dependency states immediately before dispatch.

## Audit snapshot (2026-11-29)

This branch was audited against its current task tree. Structural validation passes for 57 task contracts, and the CLI board reports no duplicate IDs or integrity problems. These checks do not establish dispatch readiness.

- No task is in `todo`; therefore no task is currently ready for an immediate worker assignment.
- Three tasks remain in `doing`: identity/tenancy contracts, versioned task-run contracts, and durable task-run lifecycle. The two contract tasks have only placeholder-level structural checks (file/index/frontmatter), not checks that their deliverables satisfy their substantive acceptance criteria; neither names exact exclusive output paths. Durable task-run lifecycle depends on three tasks still in `review` (not `done`) and its own task text records incomplete acceptance work. None of these three is dispatch-ready; do not split the contract tasks into independent workers until exclusive paths and substantive checks are defined.
- Seven incumbent-audit tasks in `done` still have stale `issue.json.status: backlog` values. The three audit tasks formerly in `review` were moved back to `backlog` because their history snapshots record metadata as `backlog` and their parent/baseline dependencies remain open. The seven `done` tasks' ledger files exist, but the original review checklist has not established that each acceptance criterion was met; do not treat those records as parallel-ready work.
- `tasks issues --dependencies` currently fails collection validation because 19 legacy labels use uppercase `lane-*` values, while the current validator requires lowercase labels. No labels were rewritten as part of this audit.
- An independent scan found 32 task metadata files without a `status` field, 7 remaining folder/metadata status conflicts, and only 19 tasks with an explicit path-scope section. `plaesy tasks move` synchronizes the folder and task.md frontmatter but does not rewrite `issue.json`; no bulk metadata rewrite was applied. The required task-by-task review of dependency readiness, actual deliverables, and path intersections is not complete; no task is certified `READY` from this scan.

Re-run the audit after resolving the legacy labels and remaining status conflicts. Do not bulk-edit missing status fields: lifecycle directory status is canonical in the CLI, and status alone does not prove completion.
