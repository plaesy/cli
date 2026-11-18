# Feature replacement measurement framework

**Status:** Draft measurement rubric for the replacement program. Use with [replacement-spec.md](replacement-spec.md), [replacement-plan.md](replacement-plan.md), and [replacement-backlog.md](replacement-backlog.md). This defines how completeness is measured; it does not claim any feature is implemented or any product has been fully audited.

## 1. Unit of measurement

Measure a *user-observable workflow capability*, not a marketing bullet, repository, or task. Give each feature a stable ID `F-<PRODUCT>-<NNN>` and each independently testable scenario `AC-<FEATURE>-<NN>`. Record subfeatures separately when they have separate user value, permissions, data, failure behavior, or acceptance. Group shared implementations under one canonical Plaesy capability ID and map all relevant source features to it; do not clone the same implementation task per product.

For every source feature, record:

- product/repository, exact release or commit, source permalink and date reviewed;
- feature name, user/actor, goal, preconditions, normal flow, alternate/error flows;
- dependencies, permissions, inputs, outputs/data formats, integrations and platform limits;
- observable acceptance scenarios and test fixture/evidence links;
- Plaesy capability/module and backlog task IDs (one-to-many mappings allowed);
- status, owner, scope decision, known gap, next action, and evidence freshness.

A source feature is not considered inventoried merely because a README, feature page, or search snippet mentions its category. Inspect the pinned repository documentation and relevant code/tests for the selected scope; verify user-visible behavior where possible. Mark unverified details `UNKNOWN`.

## 2. Feature status vocabulary

Each incumbent feature and Plaesy mapping receives exactly one status:

- `UNKNOWN`: insufficient evidence or ambiguous source/version.
- `DOCUMENTED`: described by primary source, not independently reproduced.
- `VERIFIED_INCUMBENT`: reproduced or confirmed in pinned upstream workflow.
- `PLAESY_NOT_STARTED`: scoped, not implemented.
- `PLAESY_IN_PROGRESS`: implementation exists but acceptance incomplete.
- `PLAESY_PARTIAL`: named subset passes; missing cases listed.
- `PLAESY_VERIFIED`: all applicable acceptance scenarios pass on declared profile; evidence attached.
- `OUT_OF_SCOPE_APPROVED`: explicitly excluded by product owner with rationale and date; does not count as replaced.
- `BLOCKED`: dependency or owner decision blocks verification/implementation.
- `DEPRECATED`: source capability removed/obsolete, with version evidence.

Do not translate absent search results into `NOT_SUPPORTED`. `PLAESY_VERIFIED` is version/profile-specific and expires for reevaluation when source versions, contracts, data schemas, or risk-bearing implementation changes.

## Creative authoring family audit candidates

Pinned snapshot audit evidence is summarized in `replacement-source-inventory.md`. The four creative repositories are independent products, not a single parity target. The path-level audit verified tracked-path inventory only, not source/test bodies; see `replacement-source-inventory.md` for exact SHAs and bounded claims. README/roadmap descriptions from a separate audit are documentation evidence, not behavior evidence, and selected roadmap claims can refer to older revisions. Focused incumbent test result on 2026-11-18: Craft Anything `cargo test -p craft-anything-engine --lib` passed (714 passed, 13 ignored); FigCraft `cargo test -p figcraft-engine --lib`, DeckCraft `cargo test -p deckcraft-engine --lib`, and WordCraft `cargo test -p wordcraft-doc --lib` did not finish with results within the 240-second per-command limit. FigCraft was waiting for a build-directory lock; DeckCraft also waited for that lock and timed out; WordCraft was compiling dependencies when its command timed out. These are not Plaesy tests or runtime parity evidence. An earlier batch exited 101 because it ran from the parent directory without a Cargo.toml; its results are invalid and superseded by the per-checkout run above. Before converting any lead into a requirement, choose an owner-approved first workflow and scope, then define leaf-level acceptance scenarios for that product only. Do not use roadmap labels such as “end-to-end” as Plaesy acceptance evidence.

## Paperclip issue-creation acceptance candidates (pinned discovery snapshot)

Source: Paperclip `7685b7a4fc00fb5469c4e1fb38b1d8ba534322af`, `server/src/__tests__/issue-create-deduplication-routes.test.ts`. The request assertions below were read from source and the focused Vitest suite was executed successfully on Node `v24.22.2` (repo requirement `>=24.11.0`): 1 test file passed, 8 tests passed. Dependencies were installed from the pinned lockfile and the local plugin SDK dependency was built in the detached audit checkout. These are observed incumbent behaviors, not yet owner-approved requirements or complete issue-create coverage.

| Acceptance ID | Candidate behavior | Source feature | Evidence |
|---|---|---|---|
| `AC-PAPERCLIP-ISSUE-001` | Repeating issue creation within the same company using the same idempotency key replays the original issue (including its original title), returns HTTP 200 with `deduplicated: true` and reason `idempotency_key`; the initial request returns HTTP 201. | `F-PAPERCLIP-002` | Test assertion read; not run |
| `AC-PAPERCLIP-ISSUE-002` | An idempotency key older than the configured retention window is expired before replay lookup, so a new issue is created and the key points to the new issue. | `F-PAPERCLIP-002` | Test assertion read; not run |
| `AC-PAPERCLIP-ISSUE-003` | A recent, non-hidden issue in the same company and parent scope whose status is not `done` or `cancelled`, and whose title matches after trim/whitespace-collapse/case normalization, is returned instead of creating a duplicate; source filters to the preceding 48 hours, and the route responds HTTP 200 with reason `recent_open_title`. The selected request test directly covers title normalization and sibling matching; other filters are implementation details read from source and are not independently asserted by that test. | `F-PAPERCLIP-002` | Test assertion read; not run |
| `AC-PAPERCLIP-ISSUE-004` | Concurrent keyed and title-only route creates for the same parent/title yield one HTTP 201 and one HTTP 200 deduplication response, reference the same issue, and persist the idempotency key; a subsequent replay resolves to that issue. | `F-PAPERCLIP-002` | Test assertion read; not run |
| `AC-PAPERCLIP-ISSUE-005` | Setting `allowDuplicate: true` on the route permits a second issue with the same title and returns HTTP 201 rather than deduplicating. | `F-PAPERCLIP-002` | Test assertion read; not run |
| `AC-PAPERCLIP-ISSUE-006` | Calling the internal issue service twice with identical titles and no explicit title-deduplication request creates distinct issues; the route's soft guard is not implicitly applied. | `F-PAPERCLIP-002` | Test assertion read; not run |
| `AC-PAPERCLIP-ISSUE-007` | A completed issue or an otherwise matching issue older than the 48-hour window does not block a new route create; the selected test expects HTTP 201 and a new issue ID for both cases. | `F-PAPERCLIP-002` | Test assertion read; not run |

These are narrow request-test candidates. The focused Vitest suite was executed on Node `v24.22.2`: 1 file, 8 tests passed. Other scoping, authorization, title normalization, retention and failure cases remain unknown until broader implementation audit and reproduction.

## 3. Acceptance scenario rubric

For each feature define Given/When/Then scenarios with deterministic input, environment, expected observable output/state, and failure handling. Include, as applicable:

1. Primary user success flow.
2. Empty/initial, loading/long-running, invalid-input, permission-denied, dependency-unavailable, timeout, cancellation, retry, restart/recovery and ambiguous external-effect cases.
3. Boundary/scale and supported-platform cases.
4. Data import/export, round-trip and backward-compatibility cases.
5. Security/privacy cases: unauthorized principal, cross-project/tenant access, revocation, secret redaction, malicious/untrusted input, auditability.
6. Accessibility/localization cases for user-facing features where required.

A scenario passes only with captured output/artifact or persisted state and a reproducible command/test/manual procedure tied to commit/build, environment and date. Agent prose, a green unit test unrelated to the user journey, static design, screenshots without interaction evidence, or upstream capability do not prove Plaesy acceptance.

## 4. Current candidate traceability crosswalk

This is a provisional crosswalk, not an acceptance approval. The eight DeepSeek Harness session-persistence candidates recorded in `replacement-source-inventory.md` map to shared lifecycle/recovery requirement RP-05 (and task RP-012) plus the source-audit task RP-038. Their acceptance status remains candidate/documented or test-assertion-read, not verified incumbent behavior. Before they become mandatory replacement contracts, verify the exact owner-approved incumbent version and reproduce the relevant scenarios; then map each to a replacement test/evidence artifact and update its status.

| Candidate scenarios | Source family | Spec requirement | Backlog task IDs | Current evidence |
|---|---|---|---|---|
| `AC-DSH-SESSION-001` | `F-DSH-SESSION-001` | RP-05 | RP-012, RP-046 | Focused run attempted: suite 110/190 passed; 80 failed after missing Linux x64 native binding; inconclusive |
| `AC-DSH-SESSION-002` | `F-DSH-SESSION-001` | RP-05 | RP-012, RP-046 | Focused run attempted: suite 110/190 passed; 80 failed after missing Linux x64 native binding; inconclusive |
| `AC-DSH-SESSION-003` | `F-DSH-SESSION-001` | RP-05 | RP-012, RP-046 | Focused run attempted: suite 110/190 passed; 80 failed after missing Linux x64 native binding; inconclusive |
| `AC-DSH-SESSION-004` | `F-DSH-SESSION-001` | RP-05 | RP-012, RP-046 | Focused run attempted: suite 110/190 passed; 80 failed after missing Linux x64 native binding; inconclusive |
| `AC-DSH-SESSION-005` | `F-DSH-SESSION-001` | RP-05 | RP-012, RP-046 | Focused run attempted: suite 110/190 passed; 80 failed after missing Linux x64 native binding; inconclusive |
| `AC-DSH-SESSION-006` | `F-DSH-SESSION-001` | RP-05 | RP-012, RP-046 | Pinned test assertion read; test not run |
| `AC-DSH-SESSION-007` | `F-DSH-SESSION-001` | RP-05 | RP-012, RP-046 | Pinned test assertion read; test not run |
| `AC-DSH-SESSION-008` | `F-DSH-SESSION-001` | RP-05 | RP-012, RP-046 | Pinned test assertion read; test not run |

These candidate rows are now traceable in this framework, but they are not executable replacement tests and have no Plaesy test/evidence artifact linked yet. The mapping records planning traceability only; it does not claim that existing Plaesy task contracts or implementation meet them.

### OpenProject permission candidates

The following candidates are derived from source/test assertions read at pinned snapshot `729734e478b19b009b856c96d53eaad89d436254`. They are not executed, owner-approved requirements, or proof of incumbent runtime behavior. The shared RP mapping below is provisional pending a verified task-contract crosswalk.

| Candidate scenario | Source assertion | Requirement | Backlog task | Evidence status |
|---|---|---|---|---|
| `AC-OPENPROJECT-PERM-001` | `spec/requests/work_packages/show_markdown_spec.rb`: inaccessible work package Markdown request returns 404 | RP-04 authorization; RP-05 lifecycle/API behavior | RP-046 source audit; RP-012 only if API contract work is in approved scope | Test assertion read; not run |
| `AC-OPENPROJECT-PERM-002` | `spec/requests/api/v3/projects/show_resource_spec.rb`: only visible custom fields are included; admin may see admin-only fields | RP-04 authorization | RP-046 source audit | Test assertion read; not run |
| `AC-OPENPROJECT-PERM-003` | `spec/requests/api/v3/projects/show_resource_spec.rb`: insufficient permission for project resource is not-found | RP-04 authorization | RP-046 source audit | Test assertion read; not run |
| `AC-OPENPROJECT-PERM-004` | `spec/requests/api/v3/work_packages/show_resource_spec.rb`: user without view permission receives not-found; anonymous behavior depends on login-required setting | RP-04 authorization | RP-046 source audit | Test assertion read; not run |

These rows make source assertions traceable only; before treating them as mandatory, confirm owner-approved baseline and product scope, run focused tests safely, and create replacement acceptance tests with observable outcomes. Do not infer that all OpenProject APIs use identical concealment or anonymous-access behavior.

### OpenProject membership and role visibility candidate assertions

Selected request-spec assertions read at pinned SHA `729734e478b19b009b856c96d53eaad89d436254`; tests were not run. See [`membership_resources_spec.rb`](https://github.com/opf/openproject/blob/729734e478b19b009b856c96d53eaad89d436254/spec/requests/api/v3/membership_resources_spec.rb) and [`role_resource_spec.rb`](https://github.com/opf/openproject/blob/729734e478b19b009b856c96d53eaad89d436254/spec/requests/api/v3/role_resource_spec.rb).

| Candidate scenario | Source assertion read | Candidate feature | Requirement / backlog | Evidence status |
|---|---|---|---|---|
| `AC-OPENPROJECT-MEMBER-001` | Membership collection returns only visible memberships. Membership detail returns 404 for an invisible member or missing view permission; create/update/delete requests without manage permission use the unauthorized-access assertion. | `F-OPENPROJECT-MEMBER-001` | RP-04 authorization; RP-046 provisional audit | Test assertions read; not run |
| `AC-OPENPROJECT-ROLE-001` | Role collection and detail return 403 without the tested member-view permissions; role detail returns 404 for the selected nonexistent-role case; collection filters by assignability and role unit. | `F-OPENPROJECT-MEMBER-002` | RP-04 authorization; RP-046 provisional audit | Test assertions read; not run |

| `AC-OPENPROJECT-MEMBER-002` | Updating a group membership role propagates to inherited user memberships: inherited roles are replaced/updated, while independently assigned roles remain; a user whose effective membership is unchanged keeps its timestamp and does not receive the update notification. | `F-OPENPROJECT-MEMBER-003` | RP-04 authorization; RP-046 provisional audit | Test assertions read; not run |
| `AC-OPENPROJECT-MEMBER-003` | Membership creation/update with an empty role list returns HTTP 422 with “Roles need to be assigned.”; assigning an unassignable role returns HTTP 422 with “Roles has an unassignable role.” | `F-OPENPROJECT-MEMBER-004` | RP-05 API validation; RP-046 provisional audit | Test assertions read; not run |
| `AC-OPENPROJECT-MEMBER-004` | Deleting a group membership returns 204 and removes the group membership; inherited memberships are removed from users, while an independently assigned copy of a role remains. The user's membership timestamp changes only when its effective roles change; mail is sent only to the user whose effective membership was updated, not removed. | `F-OPENPROJECT-MEMBER-005` | RP-04 authorization; RP-046 provisional audit | Test assertions read; not run |

DELETE group-member assertions: pinned [`membership_resources_spec.rb`](https://github.com/opf/openproject/blob/729734e478b19b009b856c96d53eaad89d436254/spec/requests/api/v3/membership_resources_spec.rb), `DELETE /api/v3/memberships/:id`, “with a group” context. Assertions are source-read only, not executed.

| `AC-OPENPROJECT-MEMBER-005` | Creating a project membership for a group returns the successful-create response, creates memberships for the group's users, and sends notification mail to those users in the selected default-notification case. | `F-OPENPROJECT-MEMBER-006` | RP-05 API; RP-046 provisional audit | Test assertions read; not run |

POST group-member assertions: same pinned spec, `POST /api/v3/memberships`, “for a group” context. This selected scenario does not cover custom message delivery or users added to the group later; assertions are source-read only, not executed.

| `AC-OPENPROJECT-MEMBER-006` | Creating a group membership with `sendNotifications: false` succeeds but sends no mail to the group principal in the selected case. | `F-OPENPROJECT-MEMBER-007` | RP-05 API; RP-046 provisional audit | Test assertions read; not run |
| `AC-OPENPROJECT-MEMBER-007` | Group-membership creation sends notifications to the group's users, and each delivered message includes the rendered configured custom notification text. | `F-OPENPROJECT-MEMBER-008` | RP-05 API; RP-046 provisional audit | Test assertions read; not run |

POST group-member custom-message assertion: same pinned spec's default “for a group” context sets `notificationMessage.raw`; shared `sends mails` assertions check recipients and rendered message body. Test assertions read, not run.

POST group-member notification suppression assertion: same pinned spec, “for a group” → “when deactivating notification sending” context. Source assertions read, not run.

| `AC-OPENPROJECT-MEMBER-008` | Creating a project membership for a placeholder user succeeds, and no notification email is sent to that principal in the selected case. | `F-OPENPROJECT-MEMBER-009` | RP-05 API; RP-046 provisional audit | Test assertions read; not run |
| `AC-OPENPROJECT-MEMBER-009` | Creating a membership for a user already in the project returns HTTP 422 with “User has already been taken.”; supplying a role URL where a principal URL is expected returns HTTP 422 with the supported principal-link types in the error. | `F-OPENPROJECT-MEMBER-010` | RP-05 validation; RP-046 provisional audit | Test assertions read; not run |

Placeholder and invalid-create assertions: pinned `membership_resources_spec.rb`, POST contexts “for a placeholder user”, “if providing an already taken user”, and “if providing an invalid principal link”. Tests read, not run.

| `AC-OPENPROJECT-MEMBER-010` | PATCH attempts to change a membership's project or principal return the shared read-only-field validation response in the selected cases. | `F-OPENPROJECT-MEMBER-011` | RP-05 API validation; RP-046 provisional audit | Test assertions read; not run |
| `AC-OPENPROJECT-MEMBER-011` | PATCH to a membership by a caller lacking manage permission returns the shared unauthorized response if they can view members; without view permission, the selected request returns not found. | `F-OPENPROJECT-MEMBER-012` | RP-05 API authorization; RP-046 provisional audit | Test assertions read; not run |

PATCH authorization assertions: pinned `membership_resources_spec.rb`, contexts “if lacking the manage permissions” (`view_members` only → shared unauthorized response) and “if lacking the view permissions” (no permissions → shared not-found response). Tests not run.

PATCH read-only assertions: pinned `membership_resources_spec.rb`, `PATCH /api/v3/memberships/:id`, “when attempting to switch the project” and “if attempting to switch the principal” contexts. Shared `read-only violation` expectations are asserted in source; tests not run.

| `AC-OPENPROJECT-MEMBER-012` | DELETE membership without `manage_members` permission returns the shared unauthorized response in the selected request case. | `F-OPENPROJECT-MEMBER-013` | RP-05 API authorization; RP-046 provisional audit | Test assertions read; not run |

DELETE permission assertion: pinned `membership_resources_spec.rb`, `DELETE /api/v3/memberships/:id`, “without permission to delete members” context. Tests not run.

| `AC-OPENPROJECT-MEMBER-013` | Membership collection pagination reports the total number of visible memberships while returning only the requested page slice; the selected `pageSize=1&offset=2` case returns one item and visible total two. | `F-OPENPROJECT-MEMBER-014` | RP-05 API collection behavior; RP-046 provisional audit | Test assertions read; not run |

Collection pagination assertion: pinned `membership_resources_spec.rb`, `GET api/v3/memberships`, “with pageSize, offset and sortBy” context. Tests not run.

| `AC-OPENPROJECT-MEMBER-014` | Membership collection filters by user name, project, or principal return matching visible memberships; an invalid filter yields HTTP 400 `InvalidQuery`. Filtering by a visible group with no memberships returns an empty collection, while filtering by a group the caller cannot see yields HTTP 400 `InvalidQuery`. | `F-OPENPROJECT-MEMBER-015` | RP-05 API query behavior; RP-046 provisional audit | Test assertions read; not run |

Collection filter assertions: pinned `membership_resources_spec.rb`, `GET api/v3/memberships` contexts for user name, project, principal, visible group with no memberships, invisible group, and invalid filter. Assertions read, not run.

| `AC-OPENPROJECT-MEMBER-015` | POST membership creation without `manage_members` permission returns the shared unauthorized response in the selected request case. | `F-OPENPROJECT-MEMBER-016` | RP-05 API authorization; RP-046 provisional audit | Test assertions read; not run |

POST permission assertion: pinned `membership_resources_spec.rb`, `POST api/v3/memberships`, “if lacking the manage permissions” context. Tests not run.

| `AC-OPENPROJECT-MEMBER-016` | Membership collection still accepts the legacy `created_on` sort key (renamed to `created_at`) with descending order in the selected paginated request case. | `F-OPENPROJECT-MEMBER-017` | RP-05 API compatibility; RP-046 provisional audit | Test assertions read; not run |
| `AC-OPENPROJECT-MEMBER-017` | GET of an invisible membership, or a membership when the caller lacks the necessary permission, returns HTTP 404 in the selected cases. | `F-OPENPROJECT-MEMBER-018` | RP-05 API authorization; RP-046 provisional audit | Test assertions read; not run |

Compatibility and detail-visibility assertions: pinned `membership_resources_spec.rb`, GET collection “with the outdated created_on sort by” and GET `/:id` contexts “if querying an invisible member” / “without the necessary permissions”. Assertions read, not run.

| `AC-OPENPROJECT-MEMBER-018` | PATCH membership with an empty roles array returns HTTP 422 and message `Roles need to be assigned.` in the selected case. | `F-OPENPROJECT-MEMBER-019` | RP-05 API validation; RP-046 provisional audit | Test assertions read; not run |

PATCH role-validation assertion: pinned `membership_resources_spec.rb`, “if attempting to empty the roles” context. Assertions read, not run.

| `AC-OPENPROJECT-MEMBER-019` | GET membership collection with no permissions returns an empty collection with `total=0` in the selected case; unlike detail reads, the collection is not asserted to return 404 here. | `F-OPENPROJECT-MEMBER-020` | RP-05 API authorization/filtering; RP-046 provisional audit | Test assertions read; not run |

Collection no-permission assertion: pinned `membership_resources_spec.rb`, GET collection “without permissions” context. Assertions read, not run.

| `AC-OPENPROJECT-MEMBER-020` | GET membership collection includes memberships whose principals are groups and placeholder users in the selected cases; each appears alongside the user membership entries and contributes to the returned total. | `F-OPENPROJECT-MEMBER-021` | RP-05 membership collection; RP-046 provisional audit | Test assertions read; not run |

Collection principal-type assertions: pinned `membership_resources_spec.rb`, GET collection “with a group” and “with a placeholder_user” contexts. Assertions read, not run.

| `AC-OPENPROJECT-MEMBER-021` | POST membership with an empty roles array returns HTTP 422 and message `Roles need to be assigned.` in the selected case. | `F-OPENPROJECT-MEMBER-022` | RP-05 API validation; RP-046 provisional audit | Test assertions read; not run |

POST role-validation assertion: pinned `membership_resources_spec.rb`, “if providing no roles” context. Assertions read, not run.

| `AC-OPENPROJECT-MEMBER-022` | POST membership with a principal link pointing to a role URL returns HTTP 422; the error states that a user property must link to a group, user, or placeholder-user resource and reports the invalid URL. | `F-OPENPROJECT-MEMBER-023` | RP-05 API validation; RP-046 provisional audit | Test assertions read; not run |

POST malformed-link assertion: pinned `membership_resources_spec.rb`, “if providing erroneous hrefs” context. Assertions read, not run.

| `AC-OPENPROJECT-MEMBER-023` | POST membership for a user who is already a member of that project returns HTTP 422 with `User has already been taken.` in the selected case. | `F-OPENPROJECT-MEMBER-024` | RP-05 API validation; RP-046 provisional audit | Test assertions read; not run |

POST duplicate-principal assertion: pinned `membership_resources_spec.rb`, “if providing an already taken user” context. Assertions read, not run.

| `AC-OPENPROJECT-MEMBER-024` | A non-admin POST for a global membership (project link `nil`) returns HTTP 422 with `Project can't be blank.` in the selected case. | `F-OPENPROJECT-MEMBER-025` | RP-05 API validation; RP-046 provisional audit | Test assertions read; not run |

POST global-membership assertion: pinned `membership_resources_spec.rb`, “for a global membership” → “as a non admin” context. Assertions read, not run; this records the observed validation response, not a general authorization policy.

**Membership acceptance evidence caveat:** all twenty-four candidates are selected source assertions at the pinned discovery snapshot, not approved requirements, executed tests, or exhaustive membership contracts.

Source evidence: pinned `membership_resources_spec.rb` asserts empty-role rejection for POST and PATCH, and unassignable-role rejection for PATCH. These tests were read, not run.

POST/PATCH validation examples are in the pinned [`membership_resources_spec.rb`](https://github.com/opf/openproject/blob/729734e478b19b009b856c96d53eaad89d436254/spec/requests/api/v3/membership_resources_spec.rb); acceptance assertions were read, not run.

These selected assertions do not establish all membership filters, all role types, anonymous behavior, every group propagation edge case, or whether the same 403/404 distinctions apply to other resources.

Group role propagation assertions are in the pinned [`membership_resources_spec.rb`](https://github.com/opf/openproject/blob/729734e478b19b009b856c96d53eaad89d436254/spec/requests/api/v3/membership_resources_spec.rb); this scenario is derived from the PATCH group-membership example and has not been executed.

### OpenProject work-package API source assertion candidates

These scenarios are extracted from request-spec assertions at the same pinned SHA; they are not executed, approved requirements, or incumbent runtime verification. Link them to `F-OPENPROJECT-WP-001`–`006` in the inventory and RP-046 audit. RP-012 is provisional only if API/work-package contracts enter approved scope.

| Candidate scenario | Source assertion read | Candidate feature | Requirement / backlog | Evidence status |
|---|---|---|---|---|
| `AC-OPENPROJECT-WP-001` | `create_resource_spec.rb`: authorized create returns 201 and persists requested subject/project/type; permission denial returns forbidden; empty/bogus input does not create. | `F-OPENPROJECT-WP-002` | RP-04, RP-05; RP-046; RP-012 provisional | Test assertions read; not run |
| `AC-OPENPROJECT-WP-002` | `update_resource_spec.rb`: invisible resource is concealed as not-found; visible but non-editable resource is unauthorized; allowed subject update returns updated representation; invalid/readonly fields reject. | `F-OPENPROJECT-WP-002` | RP-04, RP-05; RP-046; RP-012 provisional | Test assertions read; not run |
| `AC-OPENPROJECT-WP-003` | `delete_resource_spec.rb`: authorized delete returns 204 and removes record; lack of visibility is not-found; lack of delete permission is unauthorized and preserves record. | `F-OPENPROJECT-WP-002` | RP-04, RP-05; RP-046; RP-012 provisional | Test assertions read; not run |
| `AC-OPENPROJECT-WP-004` | `pdf_export_spec.rb`: permitted export returns PDF attachment with `%PDF` body; denied export unauthorized; hidden/missing item not-found; unknown/disabled template returns 400. | `F-OPENPROJECT-WP-006` | RP-04, RP-05; RP-046; RP-012 provisional | Test assertions read; not run |
| `AC-OPENPROJECT-WP-005` | `project_specific_custom_values_spec.rb`: no-longer-available stored value renders as undisclosed; an undisclosed link preserves it on patch; available option can replace it; enterprise-plan-disabled case still preserves value as undisclosed. | `F-OPENPROJECT-WP-003`, `F-OPENPROJECT-WP-004` | RP-04, RP-05; RP-046; RP-012 provisional | Test assertions read; not run |

No candidate above closes the source audit or proves behavior outside the named cases. Request-spec filenames for forms, schemas, workspace routes, candidate selection, and index/show remain unmapped assertions; inspect them before adding additional acceptance claims.

### OpenProject work-package form/schema assertion candidates

Assertions read at pinned SHA `729734e478b19b009b856c96d53eaad89d436254`. These are provisional acceptance candidates, linked to the inventory source-mapped leads and RP-046; tests were not executed and owner scope is unresolved.

| Candidate scenario | Source assertion read | Candidate feature | Requirement / backlog | Evidence status |
|---|---|---|---|---|
| `AC-OPENPROJECT-WP-006` | `create_form_resource_spec.rb`: form returns 200, exposes allowed project link and field-level validation; invalid required custom field has no commit link, valid input clears errors and enables commit. Pending labels are echoed but not persisted during form validation. | `F-OPENPROJECT-WP-003` | RP-05; RP-046; RP-012 provisional | Assertions read; not run |
| `AC-OPENPROJECT-WP-007` | `form/work_package_form_resource_spec.rb`: edit form is concealed without view/edit permissions, contains payload and lockVersion when authorized; stale lockVersion returns conflict; pending labels do not persist before commit. | `F-OPENPROJECT-WP-003` | RP-04, RP-05; RP-046; RP-012 provisional | Assertions read; not run |
| `AC-OPENPROJECT-WP-008` | `work_packages_schemas_resource_spec.rb`: valid schema returns 200 and weak ETag; matching ETag returns 304 with empty body; permissions control writable attributes; invalid filters return 400, unauthorized returns 403, anonymous behavior follows login-required setting. | `F-OPENPROJECT-WP-003`, `F-OPENPROJECT-WP-005` | RP-04, RP-05; RP-046; RP-012 provisional | Assertions read; not run |
| `AC-OPENPROJECT-WP-009` | `form_configuration_exclusions_spec.rb`: excluded custom fields are absent from schema/values and emptied groups are removed, while excluded fields remain visible when configuration is disabled. | `F-OPENPROJECT-WP-003` | RP-04, RP-05; RP-046; RP-012 provisional | Assertions read; not run |
| `AC-OPENPROJECT-WP-010` | `available_projects_on_create_api_spec.rb`, `available_projects_on_edit_api_spec.rb`: project candidates require add/edit permission and support filters; missing permission is unauthorized. | `F-OPENPROJECT-WP-005` | RP-04, RP-05; RP-046; RP-012 provisional | Assertions read; not run |
| `AC-OPENPROJECT-WP-011` | `available_relation_candidates_resource_spec.rb`: results depend on project access, archive state, relation type/direction and existing links; inaccessible work package is concealed as not-found. | `F-OPENPROJECT-WP-005` | RP-04, RP-05; RP-046; RP-012 provisional | Assertions read; not run |

These scenarios are extracted from assertions, not independently reproduced; they are not claims that all field types, permission roles, filters, or relation classes are covered. Remaining unmapped work-package request specs include assignee candidates, the broader global index query semantics, and any module-dependent extensions. Workspace create/list and show have only the bounded candidates below; this is not exhaustive coverage.

### OpenProject workspace and resource read candidates

Assertions read at pinned SHA `729734e478b19b009b856c96d53eaad89d436254`. Candidate scope only; request specs were not executed and acceptance is not owner-approved.

| Candidate scenario | Source assertion read | Candidate feature | Requirement / backlog | Evidence status |
|---|---|---|---|---|
| `AC-OPENPROJECT-WP-012` | `by_workspace_create_resource_spec.rb`: authorized workspace create returns 201 and persists attributes; missing workspace visibility is not-found, missing add permission is forbidden; empty, bogus, and invalid input do not create records. Spec exercises project and portfolio paths. | `F-OPENPROJECT-WP-002` | RP-04, RP-05; RP-046; RP-012 provisional | Assertions read; not run |
| `AC-OPENPROJECT-WP-013` | `by_workspace_index_resource_spec.rb`: workspace list succeeds for authorized viewer, conceals inaccessible workspace as not-found, returns forbidden without work-package view permission; sorting and filters affect returned elements. | `F-OPENPROJECT-WP-005` | RP-04, RP-05; RP-046; RP-012 provisional | Assertions read; not run |
| `AC-OPENPROJECT-WP-014` | `show_resource_spec.rb`: visible work package returns HAL+JSON; nonexistent or unauthorized work package is not-found; semantic identifiers resolve to the correct item. | `F-OPENPROJECT-WP-002` | RP-04, RP-05; RP-046; RP-012 provisional | Assertions read; not run |

These do not establish complete list query semantics, pagination, every HAL relation, permission matrix, or semantic-ID edge cases. The focused global-index assertions below remain bounded; the broader `index_resource_spec.rb` and assignee-candidate spec are not fully audited.

### OpenProject global index query assertion candidates

Selected assertions read at pinned SHA `729734e478b19b009b856c96d53eaad89d436254`; no tests executed. These do not claim complete coverage of the query language or API.

| Candidate scenario | Source assertion read | Candidate feature | Requirement / backlog | Evidence status |
|---|---|---|---|---|
| `AC-OPENPROJECT-WP-015` | `index_resource_spec.rb`: collection returns only visible work packages; user with no visible items gets an empty collection, while user lacking general view permission is unauthorized; typeahead can match across visible projects and exact semantic-ID match ranks ahead of prefix match. | `F-OPENPROJECT-WP-001` | RP-04, RP-05; RP-046; RP-012 provisional | Assertions read; not run |
| `AC-OPENPROJECT-WP-016` | `index_resource_spec.rb`: semantic-ID filter resolves aliases and numeric IDs; including an alias for an item the caller cannot see rejects the query as InvalidQuery/bad request. | `F-OPENPROJECT-WP-001` | RP-04, RP-05; RP-046; RP-012 provisional | Assertions read; not run |
| `AC-OPENPROJECT-WP-017` | `index_resource_spec.rb`: encoded `eprops` filter/sort/pageSize returns only matching visible items and honors page size; invalid uncompressed, non-JSON, non-Base64, non-hash, or over-limit decompression payload is rejected. | `F-OPENPROJECT-WP-001` | RP-04, RP-05, RP-09; RP-046; RP-012 provisional | Assertions read; not run |

Do not infer that these candidates cover every filter/operator, sort key, pagination edge, encoding error, response size, or security property. The source test assertions were read only; tests were not run in the incumbent environment.

### OpenProject available-assignee assertion candidates

Selected assertions read in `spec/requests/api/v3/work_packages/available_assignees_api_spec.rb` and its shared examples `spec/support/api/v3/shared_available_principals_examples.rb` at pinned SHA `729734e478b19b009b856c96d53eaad89d436254`. Tests were not run.

| Candidate scenario | Source assertion read | Candidate feature | Requirement / backlog | Evidence status |
|---|---|---|---|---|
| `AC-OPENPROJECT-WP-018` | Available assignees include eligible principals with work-package assignment permission: current/eligible users, users granted at project or work-package scope, groups and placeholder users; email is omitted for other users unless caller has global `view_user_email`. | `F-OPENPROJECT-WP-005` | RP-04, RP-05; RP-046; RP-012 provisional | Shared assertions read; not run |
| `AC-OPENPROJECT-WP-019` | Caller without required work-package visibility receives not-found for the available-assignees endpoint; caller with visibility but without assignment permission receives an empty collection. | `F-OPENPROJECT-WP-005` | RP-04; RP-046 | Shared assertions read; not run |

These candidates reflect the shared test context and do not claim all principal types, roles, privacy fields, or endpoint variants are covered.

### OpenProject work-package representation and history candidates

Selected assertions read at pinned SHA `729734e478b19b009b856c96d53eaad89d436254` in `show_resource_spec.rb` and `by_workspace_index_resource_spec.rb`. The historical-comparison assertions are explicitly gated by the source's Enterprise Edition feature flag. Tests were not run.

| Candidate scenario | Source assertion read | Candidate feature | Requirement / backlog | Evidence status |
|---|---|---|---|---|
| `AC-OPENPROJECT-WP-020` | `show_resource_spec.rb`: authorized detail response uses HAL+JSON; description renders headings, resolves work-package quick-info macro/TOC, derives dates from children, embeds direct relations but not transitive relations, and exposes remaining-time fields. | `F-OPENPROJECT-WP-001` | RP-04, RP-05; RP-046; RP-012 provisional | Assertions read; not run |
| `AC-OPENPROJECT-WP-021` | `show_resource_spec.rb` under Enterprise Edition `baseline_comparison`: historical timestamps embed baseline attributes/links, omit unchanged attributes, and omit update links for past-only states. Treat as edition-specific candidate, not universal/community behavior. | `F-OPENPROJECT-WP-001` | RP-03, RP-04, RP-05; RP-046; RP-012 provisional | EE-gated assertions read; not run |

| `AC-OPENPROJECT-WP-022` | `show_markdown_spec.rb`: authorized `.md` and `Accept: text/markdown` requests return inline `text/markdown; charset=utf-8`; project-scoped URL does not redirect; caller-supplied `current_user` query parameter is ignored. Visibility denial is separately tracked in `AC-OPENPROJECT-PERM-001`. | `F-OPENPROJECT-WP-001` | RP-04, RP-05; RP-046; RP-012 provisional | Assertions read; not run |

This pass does not establish exhaustive representation, relation, rendering, historical, Markdown export, or edition coverage.

### OpenProject relation-candidate assertion candidates

Selected assertions read in `spec/requests/api/v3/work_packages/available_relation_candidates_resource_spec.rb` at pinned SHA `729734e478b19b009b856c96d53eaad89d436254`. Tests were not run.

| Candidate scenario | Source assertion read | Candidate feature | Requirement / backlog | Evidence status |
|---|---|---|---|---|
| `AC-OPENPROJECT-WP-023` | Candidate work packages respect hierarchy/circular-dependency rules and `cross_project_work_package_relations`; case-insensitive query matches and exact semantic-ID match can outrank a newer prefix match. | `F-OPENPROJECT-WP-005` | RP-04, RP-05; RP-046; RP-012 provisional | Assertions read; not run |
| `AC-OPENPROJECT-WP-024` | Candidate set varies by relation type/direction and excludes already-related/invalid parent-child targets; archived-project items are excluded; non-admin only sees items in accessible projects; inaccessible target returns not-found. | `F-OPENPROJECT-WP-005` | RP-04; RP-046; RP-012 provisional | Assertions read; not run |

These assertions do not establish all relation types, permission combinations, hierarchy depths, or query/sort behavior.

### OpenProject available-project assertion candidates

Selected assertions read at pinned SHA `729734e478b19b009b856c96d53eaad89d436254` in `available_projects_on_create_api_spec.rb` and `available_projects_on_edit_api_spec.rb`. Tests were not run.

| Candidate scenario | Source assertion read | Candidate feature | Requirement / backlog | Evidence status |
|---|---|---|---|---|
| `AC-OPENPROJECT-WP-025` | Creation project choices require `add_work_packages`; optional type and favored filters constrain the returned project collection. | `F-OPENPROJECT-WP-004` | RP-04, RP-05; RP-046; RP-012 provisional | Assertions read; not run |
| `AC-OPENPROJECT-WP-026` | Move-target project choices require edit permission on the source work package and move permission on target projects; missing source edit permission is unauthorized. | `F-OPENPROJECT-WP-004` | RP-04; RP-046; RP-012 provisional | Assertions read; not run |

These selected cases do not define all project filters, membership roles, or move-policy combinations.

### OpenProject create-form assertion candidate

Selected assertions read in `spec/requests/api/v3/work_packages/create_form_resource_spec.rb` at pinned SHA `729734e478b19b009b856c96d53eaad89d436254`. Tests were not run.

| Candidate scenario | Source assertion read | Candidate feature | Requirement / backlog | Evidence status |
|---|---|---|---|---|
| `AC-OPENPROJECT-WP-027` | Create form returns a form/schema with available-project link; empty payload reports required field errors; minimum valid payload chooses the project's default enabled type; target-version links are echoed without persistence; pending label is not created and label creation permission controls validation. | `F-OPENPROJECT-WP-002` | RP-04, RP-05; RP-046; RP-012 provisional | Assertions read; not run |

This selected form pass does not establish the full validation/schema matrix, every custom field, or all project/type/label permission combinations.

### OpenProject update conflict and validation-atomicity candidates

Selected assertions read in `spec/requests/api/v3/work_packages/update_resource_spec.rb` at pinned SHA `729734e478b19b009b856c96d53eaad89d436254`. Tests were not run.

| Candidate scenario | Source assertion read | Candidate feature | Requirement / backlog | Evidence status |
|---|---|---|---|---|
| `AC-OPENPROJECT-WP-028` | Missing or stale `lockVersion` yields update conflict after a competing update; stale write must not silently overwrite. | `F-OPENPROJECT-WP-001` | RP-04, RP-05; RP-046; RP-012 provisional | Assertions read; not run |
| `AC-OPENPROJECT-WP-029` | Invalid required custom-field update returns 422 and leaves `updated_at` and the prior field value unchanged. | `F-OPENPROJECT-WP-001` | RP-04, RP-05; RP-046; RP-012 provisional | Assertions read; not run |

These selected assertions do not establish all conflict, retry, rollback, or validation combinations.

### OpenProject update notification and read-only field candidates

Selected assertions read in `spec/requests/api/v3/work_packages/update_resource_spec.rb` at pinned SHA `729734e478b19b009b856c96d53eaad89d436254`. Tests were not run.

| Candidate scenario | Source assertion read | Candidate feature | Requirement / backlog | Evidence status |
|---|---|---|---|---|
| `AC-OPENPROJECT-WP-030` | PATCH emits a notification by default; `notify=false` suppresses it; another non-false query value still emits a notification. | `F-OPENPROJECT-WP-001` | RP-04, RP-05; RP-046; RP-012 provisional | Assertions read; not run |
| `AC-OPENPROJECT-WP-031` | Client writes to `createdAt` or `updatedAt` are rejected as read-only; submitting both yields two `PropertyIsReadOnly` errors identifying both attributes. | `F-OPENPROJECT-WP-001` | RP-04, RP-05; RP-046; RP-012 provisional | Assertions read; not run |

These selected cases do not define notification delivery timing/retry guarantees or the full read-only-property set.

### OpenProject attribute-level update authorization candidates

Selected assertions read in `spec/requests/api/v3/work_packages/update_resource_spec.rb` at pinned SHA `729734e478b19b009b856c96d53eaad89d436254`. Tests were not run.

| Candidate scenario | Source assertion read | Candidate feature | Requirement / backlog | Evidence status |
|---|---|---|---|---|
| `AC-OPENPROJECT-WP-032` | `edit_work_packages` and attribute-specific permissions are independent: without edit permission, changing subject is rejected read-only; with `assign_versions`, setting the version is allowed; without it, version changes are rejected. | `F-OPENPROJECT-WP-001` | RP-04, RP-05; RP-046; RP-012 provisional | Assertions read; not run |
| `AC-OPENPROJECT-WP-033` | Moving a work package requires `move_work_packages` permission on the source and membership with move permission on the target; successful move returns the target project and updated lock version. | `F-OPENPROJECT-WP-001` | RP-04, RP-05; RP-046; RP-012 provisional | Assertions read; not run |

These selected examples do not define the complete permission matrix for every field, project membership, or move combination.

### OpenProject assignee and multi-version update candidates

Selected assertions read in `spec/requests/api/v3/work_packages/update_resource_spec.rb` at pinned SHA `729734e478b19b009b856c96d53eaad89d436254`. Tests were not run.

| Candidate scenario | Source assertion read | Candidate feature | Requirement / backlog | Evidence status |
|---|---|---|---|---|
| `AC-OPENPROJECT-WP-034` | Assignee/responsible accept users, groups, and placeholder users; null clears assignment; invalid, invisible, or wrong-resource links return validation errors. | `F-OPENPROJECT-WP-005` | RP-04, RP-05; RP-046; RP-012 provisional | Assertions read; not run |
| `AC-OPENPROJECT-WP-035` | Target-version updates support clearing and multiple versions only when `work_package_multiple_versions` is enabled; when disabled, multiple links return 422 without assigning any version. | `F-OPENPROJECT-WP-001` | RP-04, RP-05; RP-046; RP-012 provisional | Assertions read; not run |

These selected examples do not cover every principal visibility, assignment eligibility, version permission, or feature-flag combination.

### OpenProject label-update assertion candidate

Selected assertions read in `spec/requests/api/v3/work_packages/update_resource_spec.rb` at pinned SHA `729734e478b19b009b856c96d53eaad89d436254`. Tests were not run.

| Candidate scenario | Source assertion read | Candidate feature | Requirement / backlog | Evidence status |
|---|---|---|---|---|
| `AC-OPENPROJECT-WP-036` | Label PATCH supports assigning/clearing labels, reuses case-insensitive existing names, creates new labels attributed to the caller, and does not create/assign labels when another field is invalid or the request is unauthorized. | `F-OPENPROJECT-WP-001` | RP-04, RP-05; RP-046; RP-012 provisional | Assertions read; not run |

These selected assertions do not cover label length/normalization, concurrency, or the full label permission matrix.

### OpenProject attachment claiming candidate

Selected assertions read in `spec/requests/api/v3/work_packages/create_resource_spec.rb` at pinned SHA `729734e478b19b009b856c96d53eaad89d436254`. Tests were not run.

| Candidate scenario | Source assertion read | Candidate feature | Requirement / backlog | Evidence status |
|---|---|---|---|---|
| `AC-OPENPROJECT-WP-037` | Create claims explicitly linked and caller-owned unbound attachments, including attachments referenced in description; attachments owned by another user or already attached elsewhere are not claimed. Description-referenced claims appear in the creation journal. | `F-OPENPROJECT-WP-002` | RP-04, RP-05; RP-046; RP-012 provisional | Assertions read; not run |

These selected assertions do not establish all attachment permissions, upload lifecycle, file-link behavior, or failure/retry semantics.

### OpenProject external file-link claims

Selected assertions read in `spec/requests/api/v3/work_packages/create_resource_spec.rb` at pinned SHA `729734e478b19b009b856c96d53eaad89d436254`. Tests were not run.

| Candidate scenario | Source assertion read | Candidate feature | Requirement / backlog | Evidence status |
|---|---|---|---|---|
| `AC-OPENPROJECT-WP-038` | Creating a work-package with external file links requires `manage_file_links` and matching project storage for the file-link storage; denied/mismatched storage requests create no work-package; authorized creation attaches links and exposes the relation. | `F-OPENPROJECT-WP-002` | RP-04, RP-05; RP-046; RP-012 provisional | Assertions read; not run |

These assertions do not establish provider-specific sync, unlink, credential, or failure/retry semantics.

### OpenProject attachment replacement on update

Selected assertions read in `spec/requests/api/v3/work_packages/update_resource_spec.rb` at pinned SHA `729734e478b19b009b856c96d53eaad89d436254`. Tests were not run.

| Candidate scenario | Source assertion read | Candidate feature | Requirement / backlog | Evidence status |
|---|---|---|---|---|
| `AC-OPENPROJECT-WP-039` | PATCH with caller-owned unbound attachment replaces the work-package's existing attachment set with the supplied attachment set. | `F-OPENPROJECT-WP-002` | RP-04, RP-05; RP-046; RP-012 provisional | Assertion read; not run |

This selected assertion does not establish authorization, invalid-link atomicity, or attachment deletion semantics.

### OpenProject notification opt-out assertions

Selected assertions read in `spec/requests/api/v3/work_packages/create_resource_spec.rb` and `update_resource_spec.rb` at pinned SHA `729734e478b19b009b856c96d53eaad89d436254`. Tests were not run.

| Candidate scenario | Source assertion read | Candidate feature | Requirement / backlog | Evidence status |
|---|---|---|---|---|
| `AC-OPENPROJECT-WP-040` | Create/update enqueue notifications by default; `notify=false` suppresses them; explicit `notify=true` on create sends them, and a non-`false` update value also sends them. | `F-OPENPROJECT-WP-002` | RP-04, RP-05; RP-046; RP-012 provisional | Assertions read; not run |

This candidate does not specify every notification recipient, channel, event, or delivery failure/retry rule.

### OpenProject work-package watcher assertions

Selected assertions read in `spec/requests/api/v3/watcher_resource_spec.rb` at pinned SHA `729734e478b19b009b856c96d53eaad89d436254`. Tests were not run.

| Candidate scenario | Source assertion read | Candidate feature | Requirement / backlog | Evidence status |
|---|---|---|---|---|
| `AC-OPENPROJECT-WP-041` | Watcher list is permission-gated and concealed when the work-package is invisible; add requires permission unless caller watches self, rejects nonexistent/ineligible/locked users, and adding an existing watcher is idempotent; delete requires permission, sends notification, and deleting a non-watcher returns 204. | `F-OPENPROJECT-WP-005` | RP-04, RP-05; RP-046; RP-012 provisional | Assertions read; not run |

The selected cases do not establish complete watcher roles, notification delivery/retry, or concurrency behavior.

| `AC-OPENPROJECT-WP-042` | `watcher_resource_spec.rb`: self-unwatch is allowed without delete permission; available-watcher candidate listing/search requires permission, returns eligible candidates, supports name filtering and reduced `select` fields. | `F-OPENPROJECT-WP-005` | RP-04, RP-05; RP-046; RP-012 provisional | Assertions read; not run |

These cases do not establish exhaustive search/filter semantics or candidate authorization.

### OpenProject work-package activity and journal assertions

Selected assertions read in `spec/requests/api/v3/activities_by_work_package_resource_spec.rb` and `activities_api_spec.rb` at pinned SHA `729734e478b19b009b856c96d53eaad89d436254`. Tests were not run.

| Candidate scenario | Source assertion read | Candidate feature | Requirement / backlog | Evidence status |
|---|---|---|---|---|
| `AC-OPENPROJECT-WP-043` | Work-package activity listing hides the collection when the package is not visible; internal journal entries are included only when the project feature and caller permission allow them; create rejects unauthorized internal comments and internal comments disabled at project level. | `F-OPENPROJECT-WP-006` | RP-04, RP-05; RP-046; RP-012 provisional | Assertions read; not run |
| `AC-OPENPROJECT-WP-044` | Activity read conceals inaccessible/nonexistent entries and internal journals from callers without permission; comment edits distinguish own vs other-user public/internal journals and enforce the corresponding edit permissions. | `F-OPENPROJECT-WP-006` | RP-04, RP-05; RP-046; RP-012 provisional | Assertions read; not run |

These candidates do not establish complete activity pagination/order, journal rendering, attachment claiming, reaction behavior, or comment sanitization.

| `AC-OPENPROJECT-WP-045` | `activities_by_work_package_resource_spec.rb`: creating an activity whose comment embeds eligible attachment URLs associates the referenced attachments with the created journal. | `F-OPENPROJECT-WP-006` | RP-04, RP-05; RP-046; RP-012 provisional | Assertions read; not run |
| `AC-OPENPROJECT-WP-046` | `activities_api_spec.rb`: reading a commented activity includes the associated emoji reaction identifier and emoji payload. | `F-OPENPROJECT-WP-006` | RP-04, RP-05; RP-046; RP-012 provisional | Assertions read; not run |

These selected assertions do not establish attachment ownership/authorization, reaction mutation/removal, or exhaustive activity representation.

### OpenProject saved-query API candidate assertions

Selected assertions read at pinned SHA `729734e478b19b009b856c96d53eaad89d436254` in `spec/requests/api/v3/queries/create_query_spec.rb`, `update_query_spec.rb`, `queries_by_workspace_resource_spec.rb`, `create_form_api_spec.rb`, and `update_form_api_spec.rb`. Tests were not run.

| Candidate scenario | Source assertion read | Candidate feature | Requirement / backlog | Evidence status |
|---|---|---|---|---|
| `AC-OPENPROJECT-QUERY-001` | Create/update saved queries preserve configured project/public state, filters/operators/values, columns, sorting and grouping; invalid project/operator/filter is rejected without silently accepting unknown references. | `F-OPENPROJECT-QUERY-001` | RP-04, RP-05; RP-046; RP-012 provisional | Assertions read; not run |
| `AC-OPENPROJECT-QUERY-002` | Query creation requires `save_queries`; a visible project without that permission is forbidden; workspace-scoped query reads deny callers without visibility permission. | `F-OPENPROJECT-QUERY-001` | RP-04, RP-05; RP-046; RP-012 provisional | Assertions read; not run |
| `AC-OPENPROJECT-QUERY-003` | Create/update form responses expose validation errors, commit links only when valid, and round-trip submitted query properties in the form payload. | `F-OPENPROJECT-QUERY-002` | RP-04, RP-05; RP-046; RP-012 provisional | Assertions read; not run |

These candidates do not establish complete query filter/operator semantics, public-query visibility/edit authorization, deletion, concurrency, global/workspace listing behavior, or the full form schema.

| Candidate scenario | Source assertion read | Candidate feature | Requirement / backlog | Evidence status |
|---|---|---|---|---|
| `AC-OPENPROJECT-QUERY-004` | Filter/operator and filter-instance schema reads require work-package visibility permission; unknown IDs return 404. Global filter-instance schemas include `project` but exclude project-only `category` and `subprojectId`; project/workspace schemas include all three. | `F-OPENPROJECT-QUERY-003` | RP-04, RP-05; RP-046; RP-012 provisional | Assertions read; not run |

This candidate does not establish every filter's values/operators or exhaustive per-project schema behavior. Source files: [`query_filters_resource_spec.rb`](https://github.com/opf/openproject/blob/729734e478b19b009b856c96d53eaad89d436254/spec/requests/api/v3/queries/filters/query_filters_resource_spec.rb), [`query_filter_instance_schema_resource_spec.rb`](https://github.com/opf/openproject/blob/729734e478b19b009b856c96d53eaad89d436254/spec/requests/api/v3/queries/schemas/query_filter_instance_schema_resource_spec.rb), [`query_schema_resource_spec.rb`](https://github.com/opf/openproject/blob/729734e478b19b009b856c96d53eaad89d436254/spec/requests/api/v3/queries/schemas/query_schema_resource_spec.rb), and [`query_operators_resource_spec.rb`](https://github.com/opf/openproject/blob/729734e478b19b009b856c96d53eaad89d436254/spec/requests/api/v3/queries/operators/query_operators_resource_spec.rb). Tests were not run.

| Candidate scenario | Source assertion read | Candidate feature | Requirement / backlog | Evidence status |
|---|---|---|---|---|
| `AC-OPENPROJECT-QUERY-005` | Query collection supports project/global/updated-at/ID filters; returned queries are constrained by caller-visible scope, and inaccessible private queries are concealed on direct access. | `F-OPENPROJECT-QUERY-001` | RP-04, RP-05; RP-046 provisional | Assertions read; not run |
| `AC-OPENPROJECT-QUERY-006` | Query deletion requires public-query management permission and returns no-content on success; starring/un-starring is idempotent for authorized public queries and own private queries, while unauthorized/private-other-user attempts are rejected. | `F-OPENPROJECT-QUERY-004` | RP-04, RP-05; RP-046 provisional | Assertions read; not run |

These candidates do not establish all query collection filters, deletion semantics for every query type, or the full starred-query listing behavior.

### OpenProject saved-view API candidate assertions

Selected assertions read at pinned SHA `729734e478b19b009b856c96d53eaad89d436254` in [`index_resource_spec.rb`](https://github.com/opf/openproject/blob/729734e478b19b009b856c96d53eaad89d436254/spec/requests/api/v3/views/index_resource_spec.rb), [`show_resource_spec.rb`](https://github.com/opf/openproject/blob/729734e478b19b009b856c96d53eaad89d436254/spec/requests/api/v3/views/show_resource_spec.rb), and [`create_resource_spec.rb`](https://github.com/opf/openproject/blob/729734e478b19b009b856c96d53eaad89d436254/spec/requests/api/v3/views/create_resource_spec.rb). Tests were not run.

| Candidate scenario | Source assertion read | Candidate feature | Requirement / backlog | Evidence status |
|---|---|---|---|---|
| `AC-OPENPROJECT-VIEW-001` | View collection returns views associated with visible public queries or the caller's private queries; it supports project/type filters, while no visible queries yields an empty collection. | `F-OPENPROJECT-VIEW-001` | RP-04, RP-05; RP-046 provisional | Assertions read; not run |
| `AC-OPENPROJECT-VIEW-002` | Direct access conceals an inaccessible view with 404; creation requires access to its query and returns a validation error when the query is not visible. | `F-OPENPROJECT-VIEW-001` | RP-04, RP-05; RP-046 provisional | Assertions read; not run |
| `AC-OPENPROJECT-VIEW-003` | Calendar view creation additionally requires `view_calendar`; unknown view type returns 404. | `F-OPENPROJECT-VIEW-002` | RP-04, RP-05; RP-046 provisional | Assertions read; not run |

These candidates do not establish view update/delete behavior, all view types, listing pagination/sorting, or complete role and permission matrices.

| Candidate scenario | Source reviewed | Candidate feature | Requirement / backlog | Evidence status |
|---|---|---|---|---|
| `AC-OPENPROJECT-VIEW-004` | `lib/api/v3/views/view_representer.rb` renders view type, query/project links, ID, public/starred/name, and created/updated timestamps; project is represented as absent when the query has no project. This is source inspection, not a request-test assertion. | `F-OPENPROJECT-VIEW-003` | RP-04, RP-05; RP-046 provisional | Source reviewed; not run |

This candidate does not establish exact serialization behavior under every embedding/permission context or the full persisted view configuration contract.

### OpenProject query configuration resource candidate assertions

Selected assertions read at pinned SHA `729734e478b19b009b856c96d53eaad89d436254` in `spec/requests/api/v3/queries/columns/query_columns_resource_spec.rb`, `group_bys/query_group_bys_resource_spec.rb`, `sort_bys/query_sort_bys_resource_spec.rb`, and `order/query_order_api_spec.rb`. Tests were not run.

| Candidate scenario | Source assertion read | Candidate feature | Requirement / backlog | Evidence status |
|---|---|---|---|---|
| `AC-OPENPROJECT-QUERY-007` | Column/group-by/sort-by resource reads are permission-gated; unknown identifiers, non-groupable fields, invalid sort directions, and non-sortable fields return 404. Column availability can change with the multiple-versions setting. | `F-OPENPROJECT-QUERY-005` | RP-04, RP-05; RP-046 provisional | Assertions read; not run |
| `AC-OPENPROJECT-QUERY-008` | Manual work-package order is readable only when the query is visible; updates to public-query order require `manage_public_queries`, support insert/remove deltas, and return revision timestamps that distinguish consecutive writes. Tests assert same-millisecond writes get strictly increasing revisions and a legacy query without a revision initializes one on update. | `F-OPENPROJECT-QUERY-006` | RP-04, RP-05; RP-046 provisional | Assertions read; not run |

The order revision edge cases are asserted in [`query_order_api_spec.rb`](https://github.com/opf/openproject/blob/729734e478b19b009b856c96d53eaad89d436254/spec/requests/api/v3/queries/order/query_order_api_spec.rb). These candidates do not define all valid columns/groups/sort keys or order conflict/replay semantics beyond the selected revision assertions.

### OpenProject query iCalendar-sharing candidate assertions

Selected assertions read at pinned SHA `729734e478b19b009b856c96d53eaad89d436254` in [`query_ical_url_api_spec.rb`](https://github.com/opf/openproject/blob/729734e478b19b009b856c96d53eaad89d436254/spec/requests/api/v3/queries/ical_url/query_ical_url_api_spec.rb). Tests were not run.

| Candidate scenario | Source assertion read | Candidate feature | Requirement / backlog | Evidence status |
|---|---|---|---|---|
| `AC-OPENPROJECT-QUERY-009` | Issuing a tokenized absolute iCalendar URL requires global iCalendar enablement, project visibility, `share_calendars`, and ownership or a public query; private queries belonging to another user and inaccessible/missing queries are concealed. Response links identify the token resource and associated query. Token model assertions require a query assignment and name, allow multiple differently named tokens per user/query, and reject a duplicate name for the same user/query. | `F-OPENPROJECT-QUERY-007` | RP-04, RP-05; RP-046 provisional | Assertions read; not run |

The token model additionally asserts that deleting a token destroys its assignment and deleting an assignment destroys its token. The user access-token controller exposes a revoke-all flow scoped to the current user's token collection. These are separate source-level lifecycle leads, not evidence that QUERY-009 API issuance supports revoke/rotation; tests were not run. Sources: [`ical_token_spec.rb`](https://github.com/opf/openproject/blob/729734e478b19b009b856c96d53eaad89d436254/spec/models/token/ical_token_spec.rb) and [`access_tokens_controller.rb`](https://github.com/opf/openproject/blob/729734e478b19b009b856c96d53eaad89d436254/app/controllers/my/access_tokens_controller.rb).

This candidate does not establish token expiry, token rotation, rate limits, or calendar feed data behavior.

### OpenProject iCalendar token revocation candidate assertions

Selected source and feature assertions read at pinned SHA `729734e478b19b009b856c96d53eaad89d436254` in [`access_tokens_controller_spec.rb`](https://github.com/opf/openproject/blob/729734e478b19b009b856c96d53eaad89d436254/spec/controllers/my/access_tokens_controller_spec.rb), [`access_tokens_spec.rb`](https://github.com/opf/openproject/blob/729734e478b19b009b856c96d53eaad89d436254/spec/features/users/my/access_tokens_spec.rb), and [`my_spec.rb`](https://github.com/opf/openproject/blob/729734e478b19b009b856c96d53eaad89d436254/spec/routing/my_spec.rb). Tests were not run.

| Candidate scenario | Source assertion read | Candidate feature | Requirement / backlog | Evidence status |
|---|---|---|---|---|
| `AC-OPENPROJECT-QUERY-011` | A signed-in user can revoke one named iCalendar token; the revoked URL is reported invalid, that token disappears, and other tokens for the same or different queries remain. Controller lookup is scoped through the current user's token association; the route uses DELETE. | `F-OPENPROJECT-QUERY-007` | RP-04, RP-05; RP-046 provisional | Assertions read; not run |

This candidate does not establish authorization under cross-user forged IDs, CSRF behavior, token expiry, or feed responses after revocation; do not infer these from the selected examples.

### OpenProject default workspace-query execution candidate assertions

Selected assertions read at pinned SHA `729734e478b19b009b856c96d53eaad89d436254` in [`queries_by_workspace_resource_spec.rb`](https://github.com/opf/openproject/blob/729734e478b19b009b856c96d53eaad89d436254/spec/requests/api/v3/queries/queries_by_workspace_resource_spec.rb) and the shared [`shared_get_individual_query_examples.rb`](https://github.com/opf/openproject/blob/729734e478b19b009b856c96d53eaad89d436254/spec/support/queries/shared_get_individual_query_examples.rb). Tests were not run.

| Candidate scenario | Source assertion read | Candidate feature | Requirement / backlog | Evidence status |
|---|---|---|---|---|
| `AC-OPENPROJECT-QUERY-010` | Project, portfolio, and workspace default-query routes return embedded work-package results, apply valid filter/timestamp parameters, return `InvalidQuery` for malformed filters/timestamps, require `view_work_packages`, and gate timestamp ranges older than one day on the `baseline_comparison` EE feature. | `F-OPENPROJECT-QUERY-008` | RP-04, RP-05; RP-046 provisional | Assertions read; not run |

This candidate does not establish execution behavior for all query routes, the complete timestamp grammar, or edition-specific behavior outside the selected baseline-comparison scenario.

## 5. Completeness gates

A capability family is complete only when all of these hold:

- **Inventory coverage:** all features in the approved family have source/version and status; unknowns and exclusions are explicit. Report both number and scope of features reviewed; never claim exhaustive coverage from sampling.
- **Requirement coverage:** each in-scope feature maps to requirement(s), scenarios and one or more implementation/evidence tasks. No orphan requirements/tasks and no duplicate outcomes.
- **Behavioral acceptance:** every mandatory scenario passes on the declared supported profile; partial cases are visible and block a full-replacement claim for that feature.
- **Security and reliability:** applicable authorization, abuse, recovery, concurrency and fault cases pass; no unresolved release-blocking findings.
- **Data portability:** representative source data imports with reconciliation, export remains usable, and rollback preserves source data.
- **Operational readiness:** installation/configuration, upgrade, backup/restore, diagnostics, limits and support procedures pass for the declared profile.
- **User acceptance:** target users complete the named workflow against agreed usability criteria; owner records approval.
- **Evidence quality:** primary source pinned; tests/evidence reproducible and reviewed; limitations and residual risk named.

One failed mandatory scenario means the feature is not fully replaced. An out-of-scope feature remains explicitly unreplaced; it cannot be hidden in aggregate percentages.

## 5. Product replacement gate

A product/workflow may be called *replaced* only for a named user cohort and declared boundary when:

1. Owner has approved the exact workflow set and scope exclusions.
2. Inventory is complete for that approved scope (every row reviewed; no `UNKNOWN` for mandatory capability; no undocumented exclusions).
3. Every mandatory feature is `PLAESY_VERIFIED`, with all acceptance, security, migration and operation gates passed.
4. Critical user journeys pass side-by-side against an exact incumbent version using equivalent starting data and rubric; differences are either fixed or explicitly owner-accepted without misrepresenting parity.
5. Migration reconciliation, backup/restore, rollback and observation-period exit criteria pass.
6. Named product owner approves cutover; support, monitoring and rollback ownership are assigned.

“Replaces all functionality of product X” requires the approved inventory to include *all in-scope user-visible and operator workflows* of X, not merely the selected subset. Any explicit exclusions narrow the claim to “replaces these workflows,” never total replacement.

## 6. Aggregate reporting

Do not use a single percentage to claim parity. Report per product and workflow:

- inventory denominator and inspection boundary;
- counts by status (`UNKNOWN`, documented, verified, partial, out-of-scope, blocked);
- mandatory acceptance scenarios passed / total, with links;
- migration/recovery/security/operation gate states;
- open blockers, approved exclusions, source and evidence dates.

Optional planning progress may use weighted coverage only if the owner approves weights before scoring. Show weights and denominator, keep UNKNOWN separate, and never let high-scoring easy features mask a missing mandatory workflow. No numerical targets are assumed by this framework.

## 7. Rust and general engineering gate

For every Plaesy-owned Rust capability, record toolchain and build target; pass repository format/lint/locked-test gates; document public interfaces and error behavior; test domain invariants, integration boundaries and fault cases; verify migrations and resource bounds; review unsafe code and security-sensitive changes; scan dependencies/licenses/secrets; and capture reproducible release evidence. Apply the concrete rules in the spec's “Rust engineering rules” section. Thresholds (coverage, latency, throughput, availability, RPO/RTO, supported OS matrix) must be approved and recorded before release; the framework does not invent values.

## 8. Task generation and duplicate control

Generate tasks only after feature inventory and canonical capability mapping. One task may cover a shared capability mapped to several products. Split tasks when independently verifiable outcomes, owners, safety boundaries, or file/module ownership differ. Each task must point back to feature IDs and acceptance IDs, list dependencies and non-goals, and state concrete evidence required to close it. Before creation, search the backlog by outcome, feature ID, owned modules/files and acceptance scenarios. Reuse/extend a canonical task when it already covers the outcome. A task completion closes only its stated acceptance criteria; it never automatically closes sibling features or declares product replacement.

## 8A. R15 design-code traceability (candidate; not yet acceptance evidence)

R15 is indexed in `.plaesy/spec/plaesy-agent-platform/spec.md` and tracked by `high_chore-decide-design-code-ir-schema-and-target-adr` and `high_feature-implement-design-code-ir-round-trip`. These rows make the missing evidence explicit; they do not imply an approved target, implemented behavior, or pass. The design workflow/source/target and quantitative fidelity threshold remain `UNKNOWN` until the owner records a decision.

| Candidate scenario | Observable acceptance | Owner/task mapping | Required executable evidence | State |
|---|---|---|---|---|
| `AC-R15-IR-001` | Given an owner-approved design source, importing a representative document yields a versioned IR with a stable ID for each mapped node; when exported to the selected code target, every mapped node has a source-map entry. | ADR task: schema/node/ID decisions; implementation task: importer/codegen. | Focused corpus test asserts schema version, stable IDs and total mapped-node/source-map coverage; fixture and output retained. | UNKNOWN — source/target and implementation absent. |
| `AC-R15-IR-002` | Given unsupported constructs in a design fixture, when import/export runs, output reports each loss explicitly and never silently drops it. | ADR task: loss-report contract; implementation task: importer/exporter. | Fixture-driven test asserts exact loss IDs/types and no unreported input nodes. | UNKNOWN — loss contract not approved/implemented. |
| `AC-R15-IR-003` | Given a code document generated from the IR, when parsed back under the ADR edit policy, the measured round-trip score is reported against the owner-approved threshold. | Owner decision + ADR task: target, edit policy, threshold; implementation task: parser/corpus. | Versioned corpus, deterministic scoring command, checked threshold, emitted result and losses; do not mark pass before threshold approval. | BLOCKED — threshold and targets UNKNOWN. |
| `AC-R15-IR-004` | Given malformed or over-limit input, when importer/parser processes it, the operation returns a typed error without panic or unbounded resource use. | ADR task: input/limit contract; implementation task: parser. | Focused negative tests for malformed and oversized input, bounded execution assertions, typed error match. | UNKNOWN — not implemented/tested. |

The existing task `contracts.json` commands are shallow presence/keyword checks (ADR task) or generic Rust test/Clippy checks (implementation task); they do not establish these four scenarios. Do not validate/close the R15 task based on keyword grep, task status, or generic suite success. Implement criterion-specific checks after the owner decisions and code exist; do not start a build during this evidence-only specification update.

## 9. Current measurement state

The current spec/plan/backlog are program-level scaffolding. The current product matrix is a discovery baseline, not a complete feature inventory. Candidate scenarios exist for Hermes API and DeepSeek session persistence; the 2026-11-18 bounded audit delta in `replacement-source-inventory.md` adds further documented/test-assertion leads, not verified behavior. Focused incumbent tests on the four creative snapshots produced one completed result (Craft Anything: 714 passed, 13 ignored); FigCraft, DeckCraft, and WordCraft timed out before test summaries, so those outcomes remain unknown. No exhaustive source-to-feature audit or per-feature Plaesy acceptance run has been established. Therefore no full replacement claim is currently supported. Next evidence-producing step: inspect implementations and focused tests for the audit delta, then incrementally inventory remaining source features, populate feature IDs and acceptance matrices, map them against existing Rust/Plaesy implementation, and adjust deduplicated tasks from verified gaps.
