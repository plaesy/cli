"""Tests for the Hermes Plaesy memory provider, against a fake shared MCP service over HTTP."""

from __future__ import annotations

import importlib.util
import json
import sys
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from types import ModuleType
from typing import Any

import pytest

PLUGIN = Path(__file__).parents[1] / "plaesy" / "__init__.py"
TOKEN = "t" * 48
SESSION = "session-test-1"


class _FakeMcpHandler(BaseHTTPRequestHandler):
    def log_message(self, *args: Any) -> None:  # keep test output quiet
        pass

    def _send(self, status: int, body: dict[str, Any] | None = None, session: str | None = None) -> None:
        # The real service answers with an SSE stream of ``data:`` events, so the fake does the same.
        raw = b"" if body is None else b"data: " + json.dumps(body).encode("utf-8") + b"\n\n"
        self.send_response(status)
        if session:
            self.send_header("Mcp-Session-Id", session)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Content-Length", str(len(raw)))
        self.end_headers()
        self.wfile.write(raw)

    def do_POST(self) -> None:  # noqa: N802 - http.server naming
        if self.headers.get("Authorization") != f"Bearer {TOKEN}":
            return self._send(401, {"error": "unauthorized"})
        length = int(self.headers.get("Content-Length", "0"))
        message = json.loads(self.rfile.read(length) or b"{}")
        method = message.get("method")
        if method == "initialize":
            return self._send(200, {"jsonrpc": "2.0", "id": message["id"], "result": {"protocolVersion": "2025-03-26"}}, session=SESSION)
        if method == "notifications/initialized":
            return self._send(202)
        if self.headers.get("Mcp-Session-Id") != SESSION:
            return self._send(404, {"error": "unknown session"})
        if method != "tools/call":
            return self._send(200, {"jsonrpc": "2.0", "id": message["id"], "error": {"code": -32601, "message": method}})
        name = message["params"]["name"]
        args = message["params"].get("arguments", {})
        if name == "memory_search" and args.get("query") == "boom":
            result = {"content": [{"type": "text", "text": "fake failure"}], "isError": True}
        elif name == "memory_search":
            hit = {
                "chunk_id": 1,
                "text": "Plaesy keeps notes in a vault.",
                "score": 0.9,
                "confidence": "FACT",
                "citation": {"docid": "d1", "file_path": "knowledges/plaesy.md", "line_range": [4, 9], "timestamp": "2026-01-01T00:00:00Z"},
            }
            result = {"content": [{"type": "text", "text": "{}"}], "structuredContent": {"hits": [hit], "mode": args.get("mode")}, "isError": False}
        elif name == "memory_stats":
            result = {"content": [{"type": "text", "text": "{}"}], "structuredContent": {"notes": 3}, "isError": False}
        else:
            result = {"content": [{"type": "text", "text": "unknown tool"}], "isError": True}
        return self._send(200, {"jsonrpc": "2.0", "id": message["id"], "result": result})


@pytest.fixture
def fake_service(monkeypatch):
    server = ThreadingHTTPServer(("127.0.0.1", 0), _FakeMcpHandler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    monkeypatch.setenv("PLAESY_MCP_TOKEN", TOKEN)
    yield f"http://127.0.0.1:{server.server_address[1]}/mcp"
    server.shutdown()
    server.server_close()


def load_module(monkeypatch: Any) -> ModuleType:
    """Load the plugin as a package, the way Hermes does, so relative imports resolve."""
    monkeypatch.setenv("PLAESY_CLI_BIN", sys.executable)
    spec = importlib.util.spec_from_file_location("plaesy_pkg", PLUGIN, submodule_search_locations=[str(PLUGIN.parent)])
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules["plaesy_pkg"] = module
    spec.loader.exec_module(module)
    return module


def provider_for(module: ModuleType, url: str, tmp_path: Path) -> Any:
    provider = module.PlaesyMemoryProvider(config={"url": url})
    provider.initialize("session-1", hermes_home=str(tmp_path))
    return provider


class FakeContext:
    def __init__(self) -> None:
        self.providers: list[Any] = []
        self.tools: list[str] = []
        self.hooks: list[str] = []

    def register_memory_provider(self, provider: Any) -> None:
        self.providers.append(provider)

    def register_tool(self, **kwargs: Any) -> None:
        self.tools.append(kwargs["name"])

    def register_hook(self, hook_name: str, callback: Any, **kwargs: Any) -> None:
        self.hooks.append(hook_name)


def test_register_hands_hermes_a_fresh_provider(monkeypatch):
    module = load_module(monkeypatch)
    first, second = FakeContext(), FakeContext()

    module.register(first)
    module.register(second)

    assert first.providers[0].name == "plaesy"
    assert first.providers[0] is not second.providers[0]


def test_is_available_requires_token_and_makes_no_network_call(monkeypatch):
    module = load_module(monkeypatch)
    monkeypatch.delenv("PLAESY_MCP_TOKEN", raising=False)

    provider = module.PlaesyMemoryProvider(config={"url": "http://127.0.0.1:9/mcp"})

    assert provider.is_available() is False
    assert "PLAESY_MCP_TOKEN" in provider.unavailable_reason()


def test_search_tool_returns_cited_hits(monkeypatch, tmp_path, fake_service):
    module = load_module(monkeypatch)
    provider = provider_for(module, fake_service, tmp_path)

    payload = json.loads(provider.handle_tool_call("plaesy_memory_search", {"query": "vault", "limit": 99}))

    hit = payload["hits"][0]
    assert hit["citation"]["file_path"] == "knowledges/plaesy.md"
    assert hit["citation"]["line_range"] == [4, 9]
    assert hit["confidence"] == "FACT"
    provider.shutdown()


def test_search_rejects_unknown_mode(monkeypatch, tmp_path, fake_service):
    module = load_module(monkeypatch)
    provider = provider_for(module, fake_service, tmp_path)

    payload = json.loads(provider.handle_tool_call("plaesy_memory_search", {"query": "x", "mode": "fuzzy"}))

    assert "mode must be one of" in payload["error"]
    provider.shutdown()


def test_service_error_becomes_tool_error(monkeypatch, tmp_path, fake_service):
    module = load_module(monkeypatch)
    provider = provider_for(module, fake_service, tmp_path)

    payload = json.loads(provider.handle_tool_call("plaesy_memory_search", {"query": "boom"}))

    assert payload["error"] == "fake failure"
    provider.shutdown()


def test_prefetch_formats_citations_and_stays_quiet_on_short_query(monkeypatch, tmp_path, fake_service):
    module = load_module(monkeypatch)
    provider = provider_for(module, fake_service, tmp_path)

    context = provider.prefetch("what is the vault")

    assert context.startswith("## Plaesy Memory")
    assert "knowledges/plaesy.md:4-9" in context
    assert provider.prefetch("hi") == ""
    provider.shutdown()


def test_stats_tool_and_shutdown(monkeypatch, tmp_path, fake_service):
    module = load_module(monkeypatch)
    provider = provider_for(module, fake_service, tmp_path)

    assert json.loads(provider.handle_tool_call("plaesy_memory_stats", {})) == {"notes": 3}

    provider.shutdown()
    assert provider._client is None


def test_wrong_token_is_reported_clearly(monkeypatch, tmp_path, fake_service):
    module = load_module(monkeypatch)
    monkeypatch.setenv("PLAESY_MCP_TOKEN", "x" * 48)
    provider = provider_for(module, fake_service, tmp_path)

    payload = json.loads(provider.handle_tool_call("plaesy_memory_stats", {}))

    assert "rejected the token" in payload["error"]


def test_unreachable_service_is_reported_clearly(monkeypatch, tmp_path):
    module = load_module(monkeypatch)
    monkeypatch.setenv("PLAESY_MCP_TOKEN", TOKEN)
    provider = provider_for(module, "http://127.0.0.1:9/mcp", tmp_path)

    payload = json.loads(provider.handle_tool_call("plaesy_memory_stats", {}))

    assert "not reachable" in payload["error"]


def test_tool_schemas_declare_both_tools(monkeypatch):
    module = load_module(monkeypatch)
    names = [schema["name"] for schema in module.PlaesyMemoryProvider(config={}).get_tool_schemas()]

    assert names == ["plaesy_memory_search", "plaesy_memory_stats"]
