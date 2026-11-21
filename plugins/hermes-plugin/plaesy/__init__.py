"""Hermes plugin for Plaesy: vault memory search and relay messaging.

Memory search connects to a shared Plaesy service that is started outside Hermes:

    plaesy memory --vault <vault> serve --mcp --transport http --port 9123

That process owns the vault (one writer), and every Hermes session connects to it over
MCP Streamable HTTP. The plugin never starts a service itself. Every hit keeps its
citation (file path and line range). The memory tools are read-only; notes are created
with the CLI (``plaesy memory remember``).

Relay (``plaesy_relay_send`` and the pre/post LLM hooks) lives in ``relay.py`` and calls
the ``plaesy relay`` CLI.

Settings in Hermes config.yaml, under ``memory.plaesy`` (optional):

    memory:
      provider: plaesy
      plaesy:
        url: http://127.0.0.1:9123/mcp   # default
        cli_path: C:/path/plaesy.exe     # relay only; PLAESY_CLI_BIN takes precedence

The bearer token is read from the ``PLAESY_MCP_TOKEN`` environment variable (put it in
``~/.hermes/.env``, not in config.yaml).
"""

from __future__ import annotations

import json
import logging
import os
import shutil
import sys
import threading
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any, Optional

try:  # Inside Hermes these come from the agent runtime; standalone tests use fallbacks.
    from agent.memory_provider import MemoryProvider
    from tools.registry import tool_error
except ImportError:  # pragma: no cover - exercised only outside Hermes
    class MemoryProvider:  # type: ignore[no-redef]
        """Fallback base so the module imports without Hermes installed."""

    def tool_error(message: str, **extra: Any) -> str:  # type: ignore[no-redef]
        return json.dumps({"error": str(message), **extra}, ensure_ascii=False)


logger = logging.getLogger(__name__)

_EXECUTABLE = "plaesy.exe" if sys.platform == "win32" else "plaesy"
_DEFAULT_URL = "http://127.0.0.1:9123/mcp"
_TOKEN_ENV = "PLAESY_MCP_TOKEN"
_REQUEST_TIMEOUT = 30.0
_MCP_PROTOCOL_VERSION = "2025-03-26"
_MAX_LIMIT = 20
_DEFAULT_LIMIT = 5
_PREFETCH_LIMIT = 3
_PREFETCH_MIN_CHARS = 3
_SEARCH_MODES = ("hybrid", "semantic", "keyword")


def _find_binary(configured: Optional[str] = None) -> Path:
    """Resolve the plaesy binary for relay: PLAESY_CLI_BIN, then ``configured``, then beside this plugin, then PATH."""
    explicit = os.environ.get("PLAESY_CLI_BIN") or configured
    if explicit:
        path = Path(explicit).expanduser()
        if path.is_file():
            return path
        raise FileNotFoundError(f"plaesy CLI path does not exist: {path}")

    candidates = (
        Path(__file__).parent / "bin" / _EXECUTABLE,
        Path(sys.executable).parent / _EXECUTABLE,
    )
    for path in candidates:
        if path.is_file():
            return path

    resolved = shutil.which(_EXECUTABLE)
    if resolved:
        return Path(resolved)
    raise FileNotFoundError(
        f"Could not find {_EXECUTABLE}; set PLAESY_CLI_BIN or memory.plaesy.cli_path"
    )


# Kept for the plugin bootstrap checks. The memory provider itself no longer runs the CLI.
BIN = str(_find_binary())
BIN_ARGS = ["memory", "serve"]


def _load_plugin_config() -> dict[str, Any]:
    """Read ``memory.plaesy`` from Hermes config; empty when Hermes or the block is absent."""
    try:
        from hermes_cli.config import load_config

        memory_config = load_config().get("memory", {})
    except Exception:
        return {}
    block = memory_config.get("plaesy", {}) if isinstance(memory_config, dict) else {}
    return dict(block) if isinstance(block, dict) else {}


def _clamp_int(value: Any, default: int, low: int, high: int) -> int:
    try:
        number = int(value)
    except (TypeError, ValueError):
        return default
    return max(low, min(high, number))


class _SessionExpired(RuntimeError):
    """The service no longer knows our MCP session (it restarted or the session timed out)."""


def _first_text(result: dict[str, Any]) -> str:
    for item in result.get("content") or []:
        if isinstance(item, dict) and item.get("type") == "text":
            return str(item.get("text", ""))
    return ""


def _decode_reply(text: str) -> Any:
    """Decode a JSON-RPC reply. The service may answer with plain JSON or with an SSE stream of ``data:`` events."""
    stripped = text.strip()
    if stripped.startswith(("{", "[")):
        candidates = [stripped]
    else:
        candidates = [line[5:].strip() for line in text.splitlines() if line.startswith("data:") and line[5:].strip()]
    for candidate in reversed(candidates):
        try:
            return json.loads(candidate)
        except ValueError:
            continue
    raise RuntimeError("plaesy service returned an unreadable reply")


def _tool_payload(reply: Any) -> dict[str, Any]:
    """Extract the JSON object a tool returned, or raise with the service's message."""
    if not isinstance(reply, dict):
        raise RuntimeError("plaesy service returned an empty reply")
    if "error" in reply:
        error = reply["error"]
        message = error.get("message") if isinstance(error, dict) else error
        raise RuntimeError(str(message or "plaesy service error"))
    result = reply.get("result") or {}
    if result.get("isError"):
        raise RuntimeError(_first_text(result) or "plaesy tool failed")
    structured = result.get("structuredContent")
    if isinstance(structured, dict):
        return structured
    text = _first_text(result)
    if not text:
        return {}
    try:
        data = json.loads(text)
    except ValueError as error:
        raise RuntimeError("plaesy tool returned non-JSON content") from error
    return data if isinstance(data, dict) else {}


class _McpHttpClient:
    """Calls the shared ``plaesy memory serve --mcp --transport http`` service (MCP Streamable HTTP).

    One MCP session is opened lazily and reused. Calls are serialised within this process.
    """

    def __init__(self, url: str, token: str) -> None:
        self._url = url
        self._token = token
        self._session_id: Optional[str] = None
        self._next_id = 0
        self._lock = threading.Lock()

    def _request(self, method: str, params: dict[str, Any]) -> dict[str, Any]:
        self._next_id += 1
        return {"jsonrpc": "2.0", "id": self._next_id, "method": method, "params": params}

    def _post(self, payload: dict[str, Any]) -> Any:
        headers = {
            "Content-Type": "application/json",
            "Accept": "application/json, text/event-stream",
            "Authorization": f"Bearer {self._token}",
        }
        if self._session_id:
            headers["Mcp-Session-Id"] = self._session_id
        request = urllib.request.Request(
            self._url,
            data=json.dumps(payload).encode("utf-8"),
            headers=headers,
            method="POST",
        )
        try:
            with urllib.request.urlopen(request, timeout=_REQUEST_TIMEOUT) as response:
                session = response.headers.get("Mcp-Session-Id")
                if session:
                    self._session_id = session
                body = response.read()
        except urllib.error.HTTPError as error:
            if error.code == 404 and self._session_id:
                self._session_id = None
                raise _SessionExpired("plaesy MCP session expired") from error
            if error.code == 401:
                raise RuntimeError("plaesy service rejected the token; check PLAESY_MCP_TOKEN") from error
            raise RuntimeError(f"plaesy service returned HTTP {error.code}") from error
        except (urllib.error.URLError, OSError) as error:
            raise RuntimeError(f"plaesy service is not reachable at {self._url}; is it running?") from error
        if not body:
            return None
        return _decode_reply(body.decode("utf-8", errors="replace"))

    def _ensure_session(self) -> None:
        if self._session_id:
            return
        self._post(self._request("initialize", {
            "protocolVersion": _MCP_PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": {"name": "hermes-plaesy", "version": "0.2.0"},
        }))
        if not self._session_id:
            raise RuntimeError("plaesy service did not start an MCP session")
        self._post({"jsonrpc": "2.0", "method": "notifications/initialized"})

    def call_tool(self, name: str, arguments: dict[str, Any]) -> dict[str, Any]:
        with self._lock:
            for attempt in range(2):
                try:
                    self._ensure_session()
                    reply = self._post(self._request("tools/call", {"name": name, "arguments": arguments}))
                except _SessionExpired:
                    if attempt == 0:
                        continue
                    raise RuntimeError("plaesy session expired again; try the request later")
                return _tool_payload(reply)
        raise RuntimeError("plaesy tool call failed")  # unreachable: the loop always returns or raises


class PlaesyMemoryProvider(MemoryProvider):
    """Read-only Plaesy vault search for Hermes, through the shared service."""

    def __init__(self, config: Optional[dict[str, Any]] = None, client: Optional[Any] = None) -> None:
        self._config = dict(config) if config is not None else _load_plugin_config()
        self._client = client

    @property
    def name(self) -> str:
        return "plaesy"

    def _url(self) -> str:
        return str(self._config.get("url") or os.environ.get("PLAESY_MCP_URL") or _DEFAULT_URL)

    def _token(self) -> str:
        return (os.environ.get(_TOKEN_ENV) or "").strip()

    def is_available(self) -> bool:
        """Configured when a token is present. No network call is made here."""
        return bool(self._token()) or self._client is not None

    def unavailable_reason(self) -> str:
        return (
            f"set {_TOKEN_ENV} in ~/.hermes/.env and start the service: "
            "plaesy memory --vault <vault> serve --mcp --transport http --port 9123"
        )

    def get_config_schema(self) -> list[dict[str, Any]]:
        return [
            {"key": "url", "description": "MCP endpoint of the shared plaesy service", "default": _DEFAULT_URL},
        ]

    def initialize(self, session_id: str, **kwargs: Any) -> None:
        if self._client is not None:
            return
        token = self._token()
        if not token:
            logger.warning("plaesy memory: %s", self.unavailable_reason())
            return
        self._client = _McpHttpClient(self._url(), token)

    def system_prompt_block(self) -> str:
        if self._client is None:
            return ""
        return (
            "# Plaesy Memory\n"
            "Active. Use plaesy_memory_search to find notes in the vault; each hit cites its "
            "file path and line range."
        )

    def prefetch(self, query: str, *, session_id: str = "") -> str:
        """Small recall for the upcoming turn. Errors yield no context, never a failed turn."""
        text = (query or "").strip()[:500]
        if len(text) < _PREFETCH_MIN_CHARS or self._client is None:
            return ""
        try:
            hits = self._search(text, _PREFETCH_LIMIT, "hybrid")
        except RuntimeError as error:
            logger.debug("plaesy prefetch skipped: %s", error)
            return ""
        if not hits:
            return ""
        lines = ["## Plaesy Memory"]
        for hit in hits:
            lines.append(f"- {_format_citation(hit)}: {str(hit.get('text', '')).strip()[:500]}")
        return "\n".join(lines)

    def get_tool_schemas(self) -> list[dict[str, Any]]:
        return [
            {
                "name": "plaesy_memory_search",
                "description": (
                    "Search the Plaesy vault for notes relevant to a question. Each hit includes its "
                    "file path, line range and confidence so it can be cited."
                ),
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": {"type": "string", "description": "What to search for."},
                        "limit": {
                            "type": "integer",
                            "minimum": 1,
                            "maximum": _MAX_LIMIT,
                            "default": _DEFAULT_LIMIT,
                            "description": "Maximum number of hits.",
                        },
                        "mode": {
                            "type": "string",
                            "enum": list(_SEARCH_MODES),
                            "default": "hybrid",
                            "description": "hybrid combines semantic and keyword search.",
                        },
                    },
                    "required": ["query"],
                },
            },
            {
                "name": "plaesy_memory_stats",
                "description": "Report index statistics for the Plaesy vault.",
                "parameters": {"type": "object", "properties": {}},
            },
        ]

    def handle_tool_call(self, tool_name: str, args: dict[str, Any], **kwargs: Any) -> str:
        if self._client is None:
            return tool_error(f"plaesy memory is not available: {self.unavailable_reason()}")
        try:
            if tool_name == "plaesy_memory_search":
                query = str(args.get("query") or "").strip()
                if not query:
                    return tool_error("query is required")
                mode = str(args.get("mode") or "hybrid")
                if mode not in _SEARCH_MODES:
                    return tool_error(f"mode must be one of: {', '.join(_SEARCH_MODES)}")
                limit = _clamp_int(args.get("limit"), _DEFAULT_LIMIT, 1, _MAX_LIMIT)
                return json.dumps({"hits": self._search(query, limit, mode)}, ensure_ascii=False)
            if tool_name == "plaesy_memory_stats":
                return json.dumps(self._client.call_tool("memory_stats", {}), ensure_ascii=False)
            return tool_error(f"Unknown tool: {tool_name}")
        except RuntimeError as error:
            return tool_error(str(error))

    def shutdown(self) -> None:
        # Nothing to stop: the service is owned outside Hermes.
        self._client = None

    def _search(self, query: str, limit: int, mode: str) -> list[dict[str, Any]]:
        if self._client is None:
            raise RuntimeError("plaesy memory is not initialized")
        result = self._client.call_tool("memory_search", {"query": query, "limit": limit, "mode": mode})
        hits = result.get("hits", [])
        return hits if isinstance(hits, list) else []


def _format_citation(hit: dict[str, Any]) -> str:
    citation = hit.get("citation") or {}
    path = citation.get("file_path", "?")
    line_range = citation.get("line_range")
    if isinstance(line_range, list) and len(line_range) == 2:
        return f"{path}:{line_range[0]}-{line_range[1]}"
    return str(path)


def register(ctx: Any) -> None:
    """Register the memory provider and, when configured, the Plaesy Relay tool and hooks.

    The memory provider is registered first so a relay setup problem (for example a missing
    binary) never costs the agent its memory.
    """
    ctx.register_memory_provider(PlaesyMemoryProvider())
    try:
        from . import relay

        relay.register(ctx)
    except FileNotFoundError as error:
        logger.warning("plaesy relay disabled: %s", error)
