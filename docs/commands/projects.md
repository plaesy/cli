# `plaesy projects`

Manage the per-user list of Plaesy projects for local cross-project session discovery. Registration is explicit; Plaesy does not scan the filesystem. A project root must already contain `.plaesy/`.

```sh
plaesy projects register [PATH]
plaesy projects list
```

If `PATH` is omitted, the current project root is registered. The registry is stored in the platform's per-user Plaesy data directory, separate from synced project metadata. Each registered path is canonicalized and assigned a deterministic ID derived from that path. Missing or moved paths are omitted from listings; register the new root to discover it again.

## Workspaces

A workspace is a named group of registered projects. Groups are stored in `workspaces.json` beside the registry and share its lock.

```sh
plaesy workspace create NAME
plaesy workspace add NAME [PATH]
plaesy workspace remove NAME SELECTOR
plaesy workspace list [NAME]
plaesy workspace delete NAME
```

`add` registers `PATH` (default: the current project root) before adding it, and does nothing if it is already a member. `remove` takes the project ID or path. `list` without a name shows each group and its size; with a name it shows each member and marks it `missing` when its root no longer contains `.plaesy/`. A missing member stays in the group until you remove it, so nothing is dropped without being reported.

This is a convenience registry for the current OS user and machine. It does not authenticate callers, coordinate separate hosts, or prove that a registered project/session is active. Session records remain in each project's `.plaesy/sessions.json` and contain no transcript.
