# plaesy uninstall

Removes the installed `plaesy` binary and its user PATH entry. It prompts `y/N` (default No). The memory database, configuration, managed home, and all project data are kept.

## Syntax

```sh
plaesy uninstall
```

## Safety

This is destructive for the install. Ask the user before running it. Because it prompts on stdin, a non-interactive tool call may read end-of-input and cancel.

## Verify

`plaesy version` from a source build reports that nothing is installed.

`plaesy uninstall --help` is authoritative.
