# `plaesy projects`

Manage the per-user list of Plaesy projects for local cross-project session discovery. Registration is explicit; Plaesy does not scan the filesystem. A project root must already contain `.plaesy/`.

```sh
plaesy projects register [PATH]
plaesy projects list
```

If `PATH` is omitted, the current project root is registered. The registry is stored in the platform's per-user Plaesy data directory, separate from synced project metadata. Each registered path is canonicalized and assigned a deterministic ID derived from that path. Missing or moved paths are omitted from listings; register the new root to discover it again.

This is a convenience registry for the current OS user and machine. It does not authenticate callers, coordinate separate hosts, or prove that a registered project/session is active. Session records remain in each project's `.plaesy/sessions.json` and contain no transcript.
