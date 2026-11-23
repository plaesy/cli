# Replacement UX study

**Status:** target experience design, not a claim of shipped capability or feature parity. Read with [the source inventory](replacement-source-inventory.md), [measurement framework](feature-measurement-framework.md), and [Web/Desktop requirements](experiences.md). The working visual artifact is [`../Plaesy.figcraft`](../Plaesy.figcraft).

## Design objective

Give a user one coherent Plaesy experience for the important workflows represented by the 11 named replacement targets. Preserve the strengths of each workflow without forcing every tool into one generic dashboard. The CLI remains the control surface; Web, Desktop, and Mobile expose the same identified instance and capability model. A visible design flow is a proposal until its source workflow, permissions, failures, and acceptance scenarios have been audited and implemented.

## Experience architecture

| Space | Primary user task | Main surfaces |
|---|---|---|
| Home | Choose project, instance, and next action | Project home, recent work, global search, command palette |
| Agents | Direct, observe, and review agent work | Conversation, sessions, runs, providers, skills, approvals, schedules, usage |
| Work | Coordinate people, agents, and delivery | Tasks, board, timeline, dependencies, goals, budgets, review, audit |
| Knowledge | Find and govern evidence | Search with citations, graph, capture, source preview, curation |
| Studio | Create and review artifacts | App recreation, interface design, slides, documents, shared asset/library browser |
| System | Manage a trustworthy instance | Local/remote target, identity, permissions, integrations, health, backup, upgrade, desktop actions |

The active instance, location, identity, connection state, and capabilities belong in a persistent shell. A local/remote switch is explicit and never occurs silently. Deep links retain their instance and project scope. On small screens, the navigation becomes a compact index, while the selected target and consequential action remain visible.

## Target workflow coverage

These are **design obligations**, not verified replacement claims. `DOCUMENTED` and `PARTIAL` in the inventory describe source evidence, not Plaesy completeness. Each row needs leaf-level acceptance work before a parity claim.

| Source product | Workflow that must have a usable Plaesy path | Proposed experience |
|---|---|---|
| Hermes Agent | Chat/session resume, model and skill selection, memory, scheduled work, approval, logs and usage | Agents conversation, session history, skill manager, operations and approval inbox |
| Paperclip | Agent organization, assignment, goals, approvals, budgets and cost visibility | Agents roster and organization, Work planning, approval and budget views |
| DeepSeek Harness | Extensible agent runtime and plugin configuration | Backend catalog, plugin lifecycle, capability and permission details |
| Nimbalyst | Coding sessions, worktrees, file/diff review, tasks, Git and terminal | Developer workspace with session rail, editor, terminal and review pane |
| Orca | Parallel agents/worktrees, diff annotation, remote execution and mobile continuation | Agent workspace, worktree selector, review queue, remote target and mobile handoff |
| Craft Anything | Study target, plan, build, compare, refine, approve | Studio project with evidence, build stages, comparison and approval checkpoint |
| FigCraft | Frames, components, vectors, auto layout, styles, variables and prototypes | Studio design editor with layer tree, canvas, inspector and version history |
| DeckCraft | Slide masters, layouts, media, playback and export | Studio presentation editor and presenter view |
| WordCraft | Structured document editing, styles, review and import/export | Studio document editor with outline, comments, revisions and page preview |
| Herald OS | Command bar, voice/system actions, permission tiers and audit | Desktop command center with action preview, consent and activity trail |
| OpenProject | Project membership, work packages, boards, timeline, reporting and permissions | Work project home, table/board/timeline views and member administration |

Claude Code and Codex are also named in product direction and may be explicit, replaceable execution backends; they do not erase the need for Plaesy-owned state and review. This is an architectural inference from [product direction](decisions.md), not a feature claim.

## Current UX signals from source products

Checked on 2026-11-23 as design research. These moving product pages complement the pinned inventory; they are not a replacement acceptance baseline.

| Source | Observed workflow signal | Plaesy design implication |
|---|---|---|
| [Hermes dashboard](https://hermes-agent.nousresearch.com/docs/user-guide/features/web-dashboard) | A profile switcher scopes management pages and chat; the documentation calls out the need to identify the managed profile. | Keep instance/profile scope visible in the shell and repeat it on consequential actions. |
| [Paperclip agents](https://docs.paperclip.ing/guides/org/agents/) and [approvals](https://docs.paperclip.ing/guides/day-to-day/approvals/) | Agent roster, run detail, budgets, and approval requests have distinct inspection paths. | Give roster, run, cost, and approval views their own task-focused layouts. |
| [Orca worktrees](https://github.com/stablyai/orca/blob/main/docs/site/content/docs/model/worktrees.mdx) | Parallel work is scoped to worktrees, with a separate diff review path. | Expose worktree identity beside each session and keep review attached to its branch. |
| [OpenProject Gantt](https://www.openproject.org/docs/user-guide/gantt-chart/) and [boards](https://www.openproject.org/docs/user-guide/agile-boards/) | Project work can be viewed and changed through both timeline and board views. | Provide table, board, and timeline projections of the same work items instead of treating a board as the whole project model. |

## Interaction rules

1. **Visible state.** Every run, save, sync, import, export, approval, and long operation shows progress, result, durable identifier when available, and next action. A delayed or failed dependency is not represented as an empty list.
2. **One clear target.** Before a consequential operation, show instance, project, account or profile, actor, affected data, and permission. Remote operations have a distinct visual marker.
3. **Review before commitment.** For deletion, deployment, credentials, external execution and high-impact edits, show a review step with effects, recovery or retention options, then a confirmation receipt. Provide cancel and back paths.
4. **Expert speed with novice visibility.** Important actions are discoverable in the interface. A command palette and keyboard shortcuts accelerate them without hiding the ordinary path.
5. **Progressive detail.** Show task status, owner, and next action first; reveal logs, JSON, provider details and advanced controls on demand. Do not conceal information required to make an approval decision.
6. **State fidelity.** Distinguish loading, genuine empty, permission denied, offline, reconnecting, degraded, conflict, failed, partially completed and completed. Preserve user input and identify the recovery path.
7. **Portable artifacts.** File format, source path, version, import/export fidelity, and unsaved changes are visible in each Studio editor. Design, slides, documents and app recreation remain distinct workspaces even if they share shell primitives.

These choices follow [Nielsen Norman Group's usability heuristics](https://www.nngroup.com/articles/ten-usability-heuristics/), [Primer navigation guidance](https://primer.style/product/ui-patterns/navigation/), [Primer degraded experience guidance](https://primer.style/product/ui-patterns/degraded-experiences/), and [GOV.UK's check-answers pattern](https://design-system.service.gov.uk/patterns/check-answers/). The research supports the design direction; it does not prove users can complete these workflows in Plaesy.

## Accessibility review floor

- Aim for WCAG 2.2 AA in Web and an equivalent platform review in Desktop. Do not claim conformance from mockups or automated scores.
- Specify text contrast of at least 4.5:1 for normal text, 3:1 for large text and UI boundaries, and a visible keyboard focus indicator. Check actual rendered adjacent colors, including muted labels, badges and disabled/inactive states.
- Give interactive targets at least 24 by 24 CSS pixels or qualifying spacing; favor 44 by 44 for touch actions. Ensure full workflows work without dragging, and preserve order and meaning at 320 CSS pixel reflow.
- Provide accessible names and roles, logical focus order, skip navigation, status announcements, field-level errors with text, and reduced-motion behavior in implementation.

Sources: [W3C WCAG 2.2](https://www.w3.org/TR/WCAG22/), [W3C target size](https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum), [W3C reflow](https://www.w3.org/WAI/WCAG22/Understanding/reflow), and [Primer focus management](https://primer.style/accessibility/design-guidance/focus-management/).

## FigCraft review checklist

For each primary workflow, inspect desktop and narrow layouts, keyboard route and visible focus state, local and remote scope, permissions, happy path, error/recovery path, empty/degraded state, and the resulting receipt or history. Link representative screens as a prototype. Mark illustrative values and unimplemented target surfaces clearly. Before calling any replacement path complete, connect it to pinned incumbent scenarios and Plaesy implementation evidence using the measurement framework.

## Current design gaps

The FigCraft file now includes distinct target screens for agent conversation and human review, organization and budgets, backend configuration, coding diff review, app recreation comparison, visual design, presentations, documents, project timeline, desktop command and consent, and offline recovery. The agent approval journey includes a decision, confirmation and receipt; the desktop command links to its consent preview. These screens complement Home, task board, evidence search, flow editor, instance overview, mobile review and the visual system.

The replacement audit map in FigCraft links each named source to a representative workflow screen. The schedule and skills screen shows trigger, agent, permission scope, next run and history summary; editing and activation remain to be designed.

This is broad **concept coverage**, not a validated replacement journey. The remaining design work includes skills configuration and schedule activation, presenter mode, document round-trip behavior, vector and component editing, remote execution handoff, project reporting and member permissions, voice interaction, mobile creation paths, and representative error, empty, conflict and permission-denied flows. The prototype still needs end-to-end navigation, responsive variants, keyboard and focus specification, user testing, and source-by-source acceptance evidence. None of these screens may be counted as implementation or feature parity.
