# plaesy workspace

Named groups of registered projects, stored in `workspaces.json` beside the project registry.

## Syntax

```sh
plaesy workspace create <NAME>
plaesy workspace add <NAME> [PATH]          # registers PATH first; default: current project root
plaesy workspace remove <NAME> <SELECTOR>   # project id or path
plaesy workspace list [NAME]                # groups, or one group's members
plaesy workspace delete <NAME>              # projects stay registered
```

`list <NAME>` marks a member `missing` when its root no longer contains `.plaesy/`. A missing member stays until removed.

## Example

```sh
plaesy workspace create plaesy
plaesy workspace add plaesy ~/repository/github.com/plaesy/cli
plaesy workspace list plaesy
```

## Safety

`delete` removes the group (ask first). It does not unregister projects or touch project files.

`plaesy workspace <sub> --help` is authoritative.
