# `plaesy files`

Local project-scoped advisory declarations of which agent label claims to be working on a file. This is **not** an OS process lookup, authenticated session identity, filesystem lock, write fence, or TOCTOU mitigation. Other processes can edit claimed files.

Claims are stored under `.plaesy/presence/claims.json` and synchronized among cooperating Plaesy CLI invocations using a local advisory lock. Do not rely on coordination semantics on unsupported/network filesystems. The `--agent` value is self-declared; use distinct labels per session if needed. Claims expire automatically; the default TTL is 1800 seconds and the maximum is 86400 seconds.

```sh
plaesy files claim src/foo.rs --agent hermes-session-1 --task TASK-123 --ttl 1800
plaesy files who src/foo.rs
plaesy files list
plaesy files release src/foo.rs --agent hermes-session-1 --lease <LEASE_ID>
```

A conflicting live claim is rejected. An expired claim is omitted from `who` and `list`; a new claim may replace stale records. This first version does not deliver messages into another running session, discover OS processes holding a file, or stop non-cooperating writers.
