# plaesy approvals

Persisted approvals for external actions, used for example by `plaesy chat --supervised`. Records are appended to `.plaesy/approvals.log`; the latest record for an action id decides. Each approve, revoke, and check is also written to `.plaesy/audit.log`.

## Syntax

```sh
plaesy approvals approve <ACTION_ID>
plaesy approvals revoke <ACTION_ID>
plaesy approvals check <ACTION_ID>      # prints approved / not approved; exit 1 when not approved
```

## Example

A supervised chat refuses a mutating tool call with "approval required for <tool>; action id tool-<hash>". The id is a hash of the exact call, so approving it allows only that call:

```sh
plaesy approvals check tool-3f9a1c0b2d4e5f60 || echo "needs approval"
plaesy approvals approve tool-3f9a1c0b2d4e5f60
```

## Safety

Approving is the user's decision. An agent must not approve its own pending action unless the user said so for that action.

## Verify

`plaesy approvals check <ACTION_ID>` prints `approved`; `plaesy audit list --limit 5` shows the record.

`plaesy approvals <sub> --help` is authoritative.
