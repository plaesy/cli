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

The FigCraft file has 85 frames. Home and the main area sidebars show the same six workspace destinations: Home, Agents, Work, Knowledge, Studio and Settings (the System capability area). Each workspace name is a destination; a separate chevron opens or closes its feature list. One workspace group is expanded at a time. The active workspace's features appear directly below it on the five area roots, the Agents and Work detail sidebars, and the conversation page. The linked decision journeys continue to review, confirmation, result or recovery. The replacement map and prototype guide remain secondary design-review aids. Semantic colors supply Dark and Light modes. Component sets cover a four-state button and a task detail panel with New, View and Edit modes. These screens are illustrative and do not prove that the underlying feature exists.

The navigation pattern adapts [Primer's nested NavList](https://primer.style/product/components/nav-list/), which uses a chevron to distinguish expanded groups and marks the selected child, and [Atlassian's navigation system](https://atlassian.design/components/navigation-system/migration-guide), which separates global destinations from product navigation. The separate workspace link preserves direct cross-area movement; the chevron controls disclosure without leaving the current page. In implementation, the disclosure must be a real button with `aria-expanded`, `aria-controls`, Enter/Space operation, and logical focus order as described by the [WAI-ARIA accordion pattern](https://www.w3.org/WAI/ARIA/apg/patterns/accordion/). FigCraft frames show the intended states but do not establish accessibility behavior.

| Route from Home | Feature pages reached through the product UI |
|---|---|
| Agents | Conversation and approval, organization and budgets, backends and plugins, schedules, developer review, remote sessions, profile and skills; create agent, start conversation, add backend, and create or edit a schedule |
| Work | Task board, timeline, reports, members and roles; create, view and update a task through one task detail component; create project, invite member and mobile task creation |
| Knowledge | Evidence search, source preview and task-linked citations; create note |
| Studio | Choose a new artifact, then open design and components, presentations and presenter view, documents and import/revision/conflict review, build comparison and refinement, or flow editor; choose a new slide layout |
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
| OpenProject | Task board, representative task detail and update review, timeline, project report and member/role visibility | Dependency editing, role mutation, group administration, report drill-down and data-bound task detail for every item |

The prototype also shows mobile task creation → review → receipt, an offline snapshot with an unsent draft, a document conflict, and a permission-denied recovery path. These examples establish a pattern; each product workflow still needs its own states and responsive layout. Similar applications should be assessed by the capability family they exercise, not by superficial screen resemblance.

### Prototype checks performed

- Visually inspected the new workflow screens in FigCraft and corrected visible overlaps and navigation duplication.
- At the 44-frame audit, inspected 1,362 text layers and 102 layers with prototype interactions. A structural check found no text layer outside its frame and no navigation action pointing to a missing frame. Four more frames and the design tokens/component set were added afterward.
- Checked text contrast against the nearest containing solid background and corrected five low-contrast cases; the static heuristic then found none. This does not inspect gradients, transparency, focus rendering, real platform fonts or browser output, and is not a WCAG conformance test.
- Expanded isolated text-only prototype hit areas to at least 32 px on desktop and 44 px on mobile. A clickable design layer does not supply a semantic button, keyboard behavior or screen-reader name.
- Bound shared semantic colors across the existing screens and created a four-state button component set. The Light mode and component behavior still require visual and implementation review.
- Inspected the prototype navigation graph after connecting the product areas: all 47 product and reference frames are reachable from Home while excluding the replacement map and prototype guide from the route. The Plaesy brand links back to Home on 35 screens. This checks prototype navigation, not task success with users.
- Added creation journeys with task, agent and schedule setup, check-answers review and outcome; project and note setup with outcome; backend setup and member invitation with outcome; a Studio artifact chooser; a new conversation entry; and a slide layout chooser. Editing an existing schedule also leads to a change review and outcome. These are static target states, not working form controls or persisted data.
- Grouped global destinations and local features in one sidebar tree. Added expanded and collapsed states for the five area roots, five Agents/Work detail pages, and agent conversation. Preserved recent sessions as contextual children. Visually inspected Home and representative expanded/collapsed root and detail screens.
- Reused one `Task / Details` component set across New, View and Edit screens; verified the instances point to its three variants. Linked a representative board card to View, then Edit, review, receipt and an updated View instance. The board's other task cards remain illustrative until task data is bound to the reusable component.
- On 2026-11-24, exposed `Task title`, `Description`, `Owner`, and `Due date` as text properties on `Task / Details`; its New, View, and Edit instances remain shared by desktop create, view, edit, and updated-detail screens. Added a responsive `Task / Mobile Details` set with New, View, and Edit variants for mobile create and review. The mobile Edit variant has no screen instance yet because the prototype has no mobile edit page.
- Reused `Task / Review summary` on both task review screens and `Task / Receipt` on both task creation/update receipts, with editable content properties. Structural and screenshot inspection found starter component instances covering product content (a generic form and sidebar on task creation, review card over review details, receipt panel over a completion receipt, and a full-screen mobile sheet over the review inbox); those misplaced instances are now hidden, and the screens render their intended content.
- Component placement lesson: keep a component set's master variants outside clipped product frames, then add instances inside the workflow screens. When the mobile variants were first created inside the mobile task frame, the extra variant copies appeared as duplicate form content in the screen; moving the set to a dedicated canvas area and inserting one variant instance cleared the overlap.
- Rechecked the visible prototype navigation graph: all 83 product frames are reachable from Home, with no destination pointing to a missing frame. Verified the eleven disclosure pairs in both directions and checked link versus disclosure targets across all 22 states. The two separate design-review frames are intentionally outside the product route. This checks reachability only; creation outcomes and editor state still need implemented behavior and task-based usability testing.

### Visual coherence pass, 2026-11-24

The editable FigCraft artifact now uses the same mint Plaesy brand mark in all 64 desktop frames with a topbar, and the 22 early desktop frames use the interface system's `#111B2B` Ground token. The platform map, interface system, and source audit map are explicitly labeled as design references and each has a prototype link back to Workspace. The task creation, review, and receipt screens show the same three-step progress pattern. These changes preserve the Studio editors' contextual panels and the product sidebar on the main area screens.

FigCraft inspection confirmed that all 64 branded topbars have the mark, the three reference links target the Home frame, and each of the three task screens has three step labels. Screenshots of the affected references, task flow, and document editor were reviewed after the edit. This is a visual and structural prototype check; it does not establish keyboard behavior or implemented product navigation.

### Task detail activity pass, 2026-11-24

The two desktop View Task states now show task scope and priority, two sample subtasks with completion status, and a compact activity summary that distinguishes a human comment, an agent run, and an audit change. Each View Task state links to its own activity screen, where a fuller timeline also shows linked evidence and a comment draft; posting leads to a sample posted state and returns to the corresponding task version. The existing shared `Task / Details` View instance and View → Edit → Review flow remain in place. The artifact now has 89 frames. All new records and interactions are illustrative target UX, not live task data or a validated persistence model.

The pattern draws from [Jira's activity filters](https://support.atlassian.com/jira-software-cloud/docs/what-are-the-different-types-of-activity-on-an-issue/) and [individually tracked subtasks](https://support.atlassian.com/jira-software-cloud/docs/create-a-work-item-and-a-subtask/), [ClickUp's task comments and activity](https://help.clickup.com/hc/en-us/articles/44421277995415-Task-comments-now-work-like-Chat) and [task sections](https://help.clickup.com/hc/en-us/articles/10552031987735-Intro-to-tasks), and [OpenProject's work-package activity](https://www.openproject.org/docs/user-guide/activity/) and [parent-child relations](https://www.openproject.org/docs/user-guide/work-packages/work-package-relations-hierarchies/). Plaesy's proposed separation is: comments for human discussion, agent runs for execution and evidence, and audit entries for actor, time, and field transitions. Runtime permissions, retention, redaction, and durable event identity still need the project decision and acceptance work before implementation claims.

### Work shell reconciliation, 2026-11-24

The Tasks board now uses the same Work topbar, breadcrumb, user-facing page title, and selected Tasks navigation state as its detail journey. The board's six CLI states and source label remain visible. Twelve desktop task frames—create, view, edit, review, receipts, and activity/comment states—now keep the expanded Work sidebar. Their content was fitted into the remaining canvas; the shared New, View, and Edit `Task / Details` variants were narrowed together, while review and receipt panels were recentered in the main area. Visual inspection covered the board's expanded and collapsed states and representative screens in every task step. This resolves the shell mismatch visible between the old task board and the newer task detail screens; it remains a static prototype.

### UX acceptance gate

Before calling a replacement experience complete, test a pinned source scenario and its Plaesy counterpart with a person performing the task. Confirm the result, permissions, local/remote scope, recovery, keyboard and assistive-technology route, 320 CSS px reflow, and file or state fidelity where applicable. Record the outcome in the [measurement framework](feature-measurement-framework.md). No FigCraft screen is implementation or feature-parity evidence.
