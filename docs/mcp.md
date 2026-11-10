# MCP server

`plaesy memory serve --mcp` speaks the [Model Context Protocol](https://modelcontextprotocol.io) over **two transports**:

- **stdio** (default): MCP over stdin/stdout.
- **http**: Streamable HTTP at `/mcp`.

Both transports sync/watch the notes directory in the background. Five tools are read-only; two
bounded write tools create a new note or resolve a query gap. No tool replaces or deletes a note.
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
plaesy memory serve --mcp --transport http --port 8080
```

The server binds to `127.0.0.1`. The SDK validates the request host and rejects non-loopback browser
origins. It is only reachable from the same machine by default; use an SSH tunnel when a remote client needs access.
Do not expose the endpoint directly to a network. This is useful for HTTP-based clients running on
the same machine, including n8n, Dify, Kilo Code, Cursor, and Claude Code.

## Client configuration

### Claude Code / Claude Desktop

```bash
# HTTP transport (recommended)
claude mcp add plaesy-memory \
  -- /path/to/plaesy memory serve --mcp --transport http --port 8080

# Stdio alternative (single command, no HTTP server)
claude mcp add plaesy-memory \
  -e MEMORY_WATCH_DIR=/path/to/notes -e MEMORY_DB_PATH=/path/to/index/state.db \
  -- /path/to/plaesy memory serve --mcp
```

### Kilo Code

```bash
# HTTP transport
kilo mcp add plaesy-memory \
  -- /path/to/plaesy memory serve --mcp --transport http --port 8080

# Stdio alternative
kilo mcp add plaesy-memory \
  -e MEMORY_WATCH_DIR=C:/path/to/notes -e MEMORY_DB_PATH=C:/path/to/index/state.db \
  -- /path/to/plaesy memory serve --mcp
```

### n8n

**Option A - MCP Server Trigger (n8n hosts the MCP server):**

1. Start the memory server: `plaesy memory serve --mcp --transport http --port 8080`
2. In n8n, add "MCP Server Trigger" node
3. Available tools: `memory_search`, `memory_backlinks`, `memory_hygiene`, `memory_missing`,
   `memory_stats`, `memory_remember`, and `memory_resolve_gap`

**Option B - MCP Client (n8n calls remote MCP server):**

- Set "Server URL" to: `http://127.0.0.1:8080/mcp`
- Available tools: `memory_search`, `memory_backlinks`, `memory_hygiene`, `memory_missing`,
  `memory_stats`, `memory_remember`, and `memory_resolve_gap`

### Dify

In Dify: Knowledge / MCP Server / Add MCP Server → select "Streamable HTTP".
Server URL: `http://127.0.0.1:8080/mcp`
Available tools: `memory_search`, `memory_backlinks`, `memory_hygiene`, `memory_missing`,
`memory_stats`, `memory_remember`, and `memory_resolve_gap`

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

### Hermes (external provider)

Start the MCP server (stdio or HTTP), then add Plaesy as an external provider in Hermes settings,
pointing at the stdio command or HTTP URL.

### Print all snippets

```bash
plaesy memory config mcp --list
```

### Print one app's snippet

```bash
plaesy memory config mcp --app <name>
# Available: claude-code, crewai, n8n, dify, kilocode, paperclip
```

Searches issued before the first background sync finishes only see what is indexed so far.
