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
| [Hermes scheduled tasks](https://hermes-agent.nousresearch.com/docs/user-guide/features/web-dashboard) | A scheduled job has a prompt, schedule, delivery target, state, last and next run, plus pause, edit, trigger and delete paths. | Review trigger, destination, actor and permissions before activation; keep a run receipt and a route to pause. |
| [DeepSeek plugin manager](https://github.com/deepseek-ai/deepseek-harness/blob/master/packages/client/ui-plugin-manager/README.md) | A plugin bundle is inspected before installation, installation output is visible and stoppable, configuration is separate, and enabling follows installation. | Keep review, install, configure and enable as separate states; make requested access and disabled status explicit. |
| [Orca remote worktrees](https://github.com/stablyai/orca/blob/main/docs/site/content/docs/recipes/remote-worktrees.mdx) and [Nimbalyst sessions](https://github.com/nimbalyst/nimbalyst/blob/main/README.md) | Agent sessions retain worktree and execution target context; a remote agent can continue during a client disconnect. | Keep remote host, worktree, last event and uncertain run state visible; preserve unsent input until reconnection is confirmed. |
| [DeckCraft presentation features](https://github.com/storytold/deckcraft/blob/main/README.md) and [WordCraft review/features](https://github.com/storytold/wordcraft/blob/main/README.md) | Presenter view and file exchange are distinct from slide editing; document review and format import/export extend beyond page text. | Provide presenter controls and a source-versus-import review before treating an imported document as editable. |
| [OpenProject members](https://www.openproject.org/docs/user-guide/members/) and [Herald OS system permissions](https://github.com/iamlukethedev/Herald-OS/blob/main/docs/SYSTEM-BRIDGE.md) | Membership visibility and inherited roles affect what can be shown or changed; system actions have permission tiers and audit requirements. | Explain denial from the actual policy without exposing hidden data; preview an external or system action and retain its receipt. |

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

## Design assessment, 2026-11-23

The FigCraft file has 49 frames. Home opens the product areas directly: Agents, Work, Knowledge, Studio and System. Each area opens its feature pages, and the linked decision journeys continue to review, confirmation, result or recovery. The replacement map and prototype guide remain secondary design-review aids. A semantic color collection supplies Dark and Light modes, and a real button component set has Default, Hover, Focus and Disabled variants. These screens are illustrative and do not prove that the underlying feature exists.

| Route from Home | Feature pages reached through the product UI |
|---|---|
| Agents | Conversation and approval, organization and budgets, backends and plugins, schedules, developer review, remote sessions, profile and skills |
| Work | Task board, timeline, reports, members and roles, mobile task creation and review |
| Knowledge | Evidence search, source preview and task-linked citations |
| Studio | Design and components, presentations and presenter view, documents and import/revision/conflict review, build comparison and refinement, flow editor |
| System | Instance control, desktop command and voice review, consent and receipt, offline and permission recovery, platform architecture |

| Source | Representative FigCraft workflow | Design work still required before UX acceptance |
|---|---|---|
| Hermes Agent | Conversation → human review → confirmation → receipt; schedule list → activation review → receipt; scoped profile and skills | Failure and retry of scheduled runs, session resume, memory controls and verified profile isolation |
| Paperclip | Agent organization, budget guardrails, approvals and project work | Delegation and goal changes, multi-organization scoping, detailed spend inspection and approval escalation |
| DeepSeek Harness | Backend catalog → plugin spec review → installed but disabled configuration | Installation progress/stop, plugin enable and rollback, provider secret validation and failure recovery |
| Nimbalyst | Developer diff review, worktree context and remote handoff | File and terminal editing, per-hunk decision, session search/resume and mobile diff review |
| Orca | Parallel session/worktree context, remote reconnect and diff review | SSH setup/test, remote command recovery, mobile handoff and branch merge lifecycle |
| Craft Anything | Study → Build → Check → Ready comparison; failed check → evidence → refinement instruction → run progress | Target capture, completed recheck, approval result history and actual build permissions |
| FigCraft | Design editor → component variants, variables and prototype inspector | Vector editing, actual auto-layout manipulation, asset import/export and collaboration conflicts |
| DeckCraft | Slide editor → presenter view with notes, timer and display controls | Animation authoring, playback failures, recording, export and PPTX fidelity review |
| WordCraft | Document editor, source-versus-import review and individual revision decision | Complex layout, DOCX round-trip, revision history and export comparison |
| Herald OS | Command search → action consent → receipt; voice transcript → consent | Live voice capture and retention choices, OS permission tiers, audit inspection and action failure recovery |
| OpenProject | Task board, timeline, project report and member/role visibility | Work item detail and dependency editing, role mutation, group administration and report drill-down |

The prototype also shows mobile task creation → review → receipt, an offline snapshot with an unsent draft, a document conflict, and a permission-denied recovery path. These examples establish a pattern; each product workflow still needs its own states and responsive layout. Similar applications should be assessed by the capability family they exercise, not by superficial screen resemblance.

### Prototype checks performed

- Visually inspected the new workflow screens in FigCraft and corrected visible overlaps and navigation duplication.
- At the 44-frame audit, inspected 1,362 text layers and 102 layers with prototype interactions. A structural check found no text layer outside its frame and no navigation action pointing to a missing frame. Four more frames and the design tokens/component set were added afterward.
- Checked text contrast against the nearest containing solid background and corrected five low-contrast cases; the static heuristic then found none. This does not inspect gradients, transparency, focus rendering, real platform fonts or browser output, and is not a WCAG conformance test.
- Expanded isolated text-only prototype hit areas to at least 32 px on desktop and 44 px on mobile. A clickable design layer does not supply a semantic button, keyboard behavior or screen-reader name.
- Bound shared semantic colors across the existing screens and created a four-state button component set. The Light mode and component behavior still require visual and implementation review.
- Inspected the prototype navigation graph after connecting the product areas: all 47 product and reference frames are reachable from Home while excluding the replacement map and prototype guide from the route. The Plaesy brand links back to Home on 35 screens. This checks prototype navigation, not task success with users.

### UX acceptance gate

Before calling a replacement experience complete, test a pinned source scenario and its Plaesy counterpart with a person performing the task. Confirm the result, permissions, local/remote scope, recovery, keyboard and assistive-technology route, 320 CSS px reflow, and file or state fidelity where applicable. Record the outcome in the [measurement framework](feature-measurement-framework.md). No FigCraft screen is implementation or feature-parity evidence.
