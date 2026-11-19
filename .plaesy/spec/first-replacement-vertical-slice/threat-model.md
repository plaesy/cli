# Threat model: first replacement vertical slice (draft for owner review)

Status: draft written 2026-11-19 from the approved child spec. Owner review is required before this is treated as the threat model for the shared profile.
Scope: agent execution with the shared self-hosted profile, single organization, project-scoped principals (see spec `first-replacement-vertical-slice`).

| # | Threat | Asset | Mitigation in code | Test |
| --- | --- | --- | --- | --- |
| T1 | An agent acts on a project it does not belong to | Project data | Principal bound to one project; every action checks project | identity_policy_other_project_cannot_act_on_this_project |
| T2 | A revoked tool grant is still used | Tools | Revocation takes effect on the next check; denial reason grant_revoked | identity_policy_revoked_grant_is_denied_on_next_action |
| T3 | Denied or allowed actions leave no trace | Audit | Every decision is appended to the audit log, including denials | identity_policy_denies_tool_without_grant_and_audits_it |
| T4 | Secret values leak through logs or exports | Credentials | Secrets are referenced by name; redaction before log and export | approval_secret_redacts_values_from_logs, approval_secret_audit_export_redacts_secret_values |
| T5 | External side effects happen without approval or while disabled | External systems | External effect gate: flag off blocks; flag on still needs approval | approval_secret_external_blocked_when_flag_off, approval_secret_external_requires_explicit_approval, bounded_runtime_external_call_blocked_when_flag_off |
| T6 | A run loops or never ends | Compute | Step budget and cancellation checked before each step | bounded_runtime_budget_exhaustion_stops_before_next_step, bounded_runtime_cancel_stops_before_next_step |
| T7 | A crash repeats a completed step or hides a failure | Run state | Write-ahead log; restart requeues within retry budget; retry_exhausted when out of attempts | task_run_restart_while_running_is_requeued_within_policy, task_run_retry_budget_exhausted_fails_after_restart |
| T8 | A torn or tampered log is read as valid | Run history | Per-line checksum; torn tail reported; corruption is an error | runstore_torn_tail_is_reported_and_not_a_record, runstore_corruption_before_tail_is_an_error |
| T9 | An evidence artifact is altered after success | Artifacts | Content-addressed store; reads re-verify hash | task_run_evidence_tampered_bytes_are_not_served |
| T10 | A message is forged or altered in transit | Messages | HMAC-SHA256 envelope with constant-time tag check | shared_messaging_envelope_round_trips_and_rejects_tampering, shared_messaging_hmac_matches_rfc4231_vector |
| T11 | A retried message has a second effect | Messages | Idempotency key per message; repeated send adds no effect | shared_messaging_duplicate_delivery_is_one_effect, shared_messaging_lost_ack_stays_unknown_until_acknowledged |
| T12 | The service claims a message was read or a session was woken | Delivery truth | Only observed states are reported; offline recipients are queued | shared_messaging_offline_recipient_is_queued_not_read |

## Not covered in this slice

- Deployment key management and rotation. The HMAC key is a shared deployment secret in this slice. Owner or operations decision.
- Network transport security (TLS and authentication of the connection). No transport exists yet.
- Tenancy beyond a single organization. Out of scope by the owner's decision.
- Real external tools and real provider credentials. Disabled in this slice.
- Denial-of-service limits on message size and queue length. Numeric limits are UNKNOWN until the owner sets them.
