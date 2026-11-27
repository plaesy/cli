# plaesy files

Advisory, project-local declarations of which agent label is working on a file. Stored in `.plaesy/presence/claims.json` under a local advisory lock. A claim is not a filesystem lock: other processes can still edit the file.

## Syntax

```sh
plaesy files claim <PATH> --agent <LABEL> [--task <TASK>] [--ttl 1800]
plaesy files who <PATH>
plaesy files list
plaesy files watch [--interval-ms 500] [--max-polls 0]
plaesy files release <PATH> --agent <LABEL> --lease <LEASE_ID>
```

- `claim` prints a lease id; keep it for `release`. A conflicting live claim is rejected. TTL defaults to 1800 seconds, maximum 86,400.
- `who` and `list` omit expired claims.
- `watch` prints `claimed`/`released` changes as JSON Lines; it can miss changes between polls.

## Example

```sh
plaesy files claim src/relay.rs --agent worker-a --task high_feature-relay-lease
# ... edit ...
plaesy files release src/relay.rs --agent worker-a --lease <LEASE_ID>
```

## Safety

Labels are self-declared. Use a distinct label per session. Claims only help agents that check them.

## Verify

`plaesy files who <PATH>` shows your label after `claim` and nothing after `release`.

`plaesy files <sub> --help` is authoritative.
