# MCP server

`plaesy memory serve --mcp` speaks the [Model Context Protocol](https://modelcontextprotocol.io) over **two transports**:

- **stdio** (default): MCP over stdin/stdout.
- **http**: Streamable HTTP at `/mcp`.

Both transports sync/watch the notes directory in the background. Six tools are read-only; three
bounded write tools create a new note, save a session summary, or resolve a query gap. `memory_curate` combines health,
unanswered-query, duplicate, unlinked-note, and topic-index candidates into one bounded review; it
does not modify vault files. No tool replaces or deletes a note.

The default transport, HTTP port, read-only mode, and bearer-token environment variable can be
stored in the effective `config.toml` under `[mcp]`:

```toml
[mcp]
transport = "http"
port = 9123
read_only = false
auth_token_env = "PLAESY_MCP_TOKEN"
```

Command-line flags override these values. The token itself should remain in the environment (or
the adjacent local `.env` file), not in the TOML file.
The server uses the official [Rust MCP SDK](https://github.com/modelcontextprotocol/rust-sdk). Both
transports support the current `2026-07-28` protocol; stdio also supports handshake-era protocol
versions through `2024-11-05`. HTTP supports Streamable HTTP clients, including handshake-era
versions through `2025-11-25`; the older two-endpoint HTTP+SSE transport is not provided. Modern HTTP
requests are stateless; compatible legacy clients may use the SDK's session behavior.

## Transports

### `stdio` (default)

```bash
plaesy memory serve --mcp
```

Used by any MCP client supporting stdio JSON-RPC 2.0.

### `http` (Streamable)

```bash
plaesy memory serve --mcp --transport http --port 9123
```

The server binds to `127.0.0.1`. To require bearer authentication, set `PLAESY_MCP_TOKEN` in the
server environment or pass `--auth-token`; tokens must be at least 32 bytes. Clients send
`Authorization: Bearer <token>`. Keep the token out of shell history and public configuration.

For one token per client, use named tokens instead of a single shared value. Create one per client
with `plaesy memory token add laptop`, and the token is printed once. Rotate with
`plaesy memory token rotate laptop` (the old token keeps working for `--grace-hours`, default 24) and
cut off one client with `plaesy memory token revoke laptop`. The server stores only salted SHA-256
hashes, in `~/.config/plaesy/mcp-tokens.toml` (or `PLAESY_MCP_TOKENS_FILE`), which must not be inside
the synced vault. The file is re-read when it changes, so no restart is needed. `PLAESY_MCP_TOKEN`
still works as the single token labelled `default`. See
[the multi-token design](../platform/mcp-multi-token.md) for the failure rules.
Authentication is checked before the MCP route. The SDK validates the request host and rejects
non-loopback browser origins. For remote access, use a reverse proxy or secure tunnel that runs on
the same machine and forwards requests to `127.0.0.1:9123/mcp`. Ensure the upstream `Host` header
is `localhost:9123` and that the `Authorization` header is forwarded. Terminate TLS at the proxy,
and do not expose Plaesy's listener directly to the network.

## Client configuration

For several clients sharing one vault, run **one** Streamable HTTP server and point each
client at the same `/mcp` URL. A stdio registration launches a separate server process
per client; Plaesy's vault ownership lock prevents multiple writers from serving the
same vault at once. Start the HTTP server with the vault selected explicitly:

```bash
plaesy memory --vault /path/to/vault serve --mcp --transport http --port 9123
```

Then register `http://127.0.0.1:9123/mcp` in each client. The URL must include `/mcp`.
For a different port, change both the server command and every client URL.
For example, use `--port 9234` and `http://127.0.0.1:9234/mcp` when port 9123 is occupied;
pass `--http-url http://127.0.0.1:9234/mcp` to `plaesy memory config mcp` so its snippets match.

## Process lifecycle

A server using stdio is started and stopped by its client. It needs no OS service.
For multiple clients sharing one writable vault, run one HTTP memory server as a long-lived
process. On Linux, a systemd user service with `Restart=on-failure` can supervise that
process. On Windows, use a user logon task if it must start automatically. Select the
vault explicitly and keep exactly one writable server for that vault. The separate
`plaesy memory schedule install` task runs periodic maintenance; it does not keep
the HTTP MCP server alive.

The project-scoped `plaesy mcp --root <project>` server remains an stdio process per
client and project. Its project file tools should not be exposed through the shared
vault endpoint without explicit project access controls.

### Codex

```bash
codex mcp add plaesy-memory --url http://127.0.0.1:9123/mcp
```

Check with `codex mcp get plaesy-memory`. The CLI can print a TOML snippet with
`plaesy memory config mcp --app codex`.

### Hermes Agent

Add this entry to `~/.hermes/config.yaml` under the existing `mcp_servers` map:

```yaml
mcp_servers:
  plaesy-memory:
    url: http://127.0.0.1:9123/mcp
```

Reload MCP in Hermes and check its connection with `hermes mcp test plaesy-memory`.
The CLI can print the snippet with `plaesy memory config mcp --app hermes`.

### Claude Code / Claude Desktop

```bash
# HTTP transport (shared server)
claude mcp add --transport http plaesy-memory http://127.0.0.1:9123/mcp

# Stdio alternative (single command, no HTTP server)
claude mcp add plaesy-memory \
  -e MEMORY_WATCH_DIR=/path/to/notes -e MEMORY_DB_PATH=/path/to/index/state.db \
  -- /path/to/plaesy memory serve --mcp
```

### Kilo Code

```bash
# Kilo CLI: add to kilo.jsonc or .kilo/kilo.jsonc
{
  "mcp": {
    "plaesy-memory": {
      "type": "remote",
      "url": "http://127.0.0.1:9123/mcp",
      "enabled": true
    }
  }
}
```

Run `plaesy memory config mcp --app kilocode` for the local stdio alternative.

### n8n

**Option A - MCP Server Trigger (n8n hosts the MCP server):**

1. Start the memory server: `plaesy memory serve --mcp --transport http --port 9123`
2. In n8n, add "MCP Server Trigger" node
3. Available tools: `memory_search`, `memory_backlinks`, `memory_hygiene`, `memory_missing`, `memory_curate`,
   `memory_stats`, `memory_remember`, `memory_remember_session`, and `memory_resolve_gap`

**Option B - MCP Client (n8n calls remote MCP server):**

- Set "Server URL" to: `http://127.0.0.1:9123/mcp`
- Available tools: `memory_search`, `memory_backlinks`, `memory_hygiene`, `memory_missing`, `memory_curate`,
  `memory_stats`, `memory_remember`, `memory_remember_session`, and `memory_resolve_gap`

### Dify

In Dify: Knowledge / MCP Server / Add MCP Server → select "Streamable HTTP".
Server URL: `http://127.0.0.1:9123/mcp`
Available tools: `memory_search`, `memory_backlinks`, `memory_hygiene`, `memory_missing`, `memory_curate`,
`memory_stats`, `memory_remember`, `memory_remember_session`, and `memory_resolve_gap`

### Claude Code (quick reference)

Run `plaesy memory config mcp --app claude-code` to see the exact JSON snippet.

### CrewAI

```bash
pip install crewai-tools[mcp]

from crewai_tools import MCPServerAdapter
with MCPServerAdapter({"command": "plaesy", "args": ["memory", "serve", "--mcp"]}) as mcp:
    agent = Agent(role="Researcher", tools=mcp.tools)
    ...
```

### Paperclip / Hindsight

```bash
# Configure in paperclip.config.ts
import { HindsightPaperclip } from "@vectorize-io/hindsight-paperclip";

export default {
  plugins: [
    new HindsightPaperclip({
      mcpCommand: "memory",
      mcpArgs: ["serve", "--mcp"],
    }),
  ],
}
```

### Print all snippets

```bash
plaesy memory config mcp --list
```

### Print one app's snippet

```bash
plaesy memory config mcp --app <name>
# Available: claude-code, codex, hermes, cursor, crewai, n8n, dify, kilocode, paperclip
```

Searches issued before the first background sync finishes only see what is indexed so far.
