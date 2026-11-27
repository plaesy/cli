# plaesy mcp

Model Context Protocol servers: one for a project's `.plaesy/` folder (stdio), and a managed background server for the shared memory vault (HTTP on loopback).

## Syntax

```sh
plaesy mcp project [--root <PROJECT>]        # stdio; also the legacy form: plaesy mcp --root <PROJECT>
plaesy mcp memory serve --vault <DIR> [--port 9123]
plaesy mcp memory start [--vault <DIR>] [--port <PORT>]   # register autostart and run in background
plaesy mcp memory status
plaesy mcp memory stop                       # keeps the autostart registration
plaesy mcp memory uninstall                  # stops and removes autostart
```

`start` needs `--vault` the first time; the port defaults to 9123.

## Project server tools

| Tool | Effect |
|---|---|
| `plaesy_list` | List files under `.plaesy/` (optional `path`) |
| `plaesy_read` | Read one text file (md, json, yaml, yml, txt, csv, toml), up to 512 KB |
| `plaesy_write` | Create or replace one text file atomically, up to 512 KB; never deletes; refuses `analysis/` |

Paths are confined to `.plaesy/`; absolute paths, `..`, and escaping symlinks are refused.

## Register with a client

Use an absolute project root so every client sees the same folder:

```sh
claude mcp add plaesy-project -- plaesy mcp project --root /abs/path/to/project
codex mcp add plaesy-project -- plaesy mcp project --root /abs/path/to/project
plaesy memory config mcp --app claude-code      # snippet for the shared memory server
```

## Smoke test

```sh
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"check","version":"0"}}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' | plaesy mcp project --root .
```

A healthy server answers `initialize` with `serverInfo` and lists the three tools.

## Safety

- Stdout carries protocol messages only.
- On Windows a running server keeps `plaesy.exe` locked; stop it (`plaesy mcp memory stop`, or close the client) before `install`, `upgrade`, or `repair`.

`plaesy mcp <sub> --help` is authoritative.
