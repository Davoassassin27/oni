from __future__ import annotations

import json
import threading
import urllib.error
import urllib.request

import pytest

from juniperus.runtime import MockBackend, RouteBackendError
from juniperus.server import build_server


@pytest.fixture()
def server():
    instance = build_server(MockBackend(), port=0)
    thread = threading.Thread(target=instance.serve_forever, daemon=True)
    thread.start()
    try:
        yield instance
    finally:
        instance.shutdown()
        instance.server_close()
        thread.join(timeout=5)


def _url(server, path: str) -> str:
    host, port = server.server_address
    return f"http://{host}:{port}{path}"


def _get(server, path: str):
    return urllib.request.urlopen(_url(server, path), timeout=5)


def _post(server, payload, token: str | None = None):
    request = urllib.request.Request(
        _url(server, "/route"),
        data=json.dumps(payload).encode("utf-8"),
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    if token:
        request.add_header("Authorization", f"Bearer {token}")
    return request


def test_health(server) -> None:
    with _get(server, "/health") as response:
        body = json.loads(response.read())
    assert body["ok"] is True
    assert body["service"] == "juniperus"
    assert body["backend"] == "mock"


def test_schema(server) -> None:
    with _get(server, "/schema") as response:
        body = json.loads(response.read())
    assert body["title"].startswith("Juniperus")


def test_route(server) -> None:
    payload = {
        "text": "mergeá 3f2a1b9c",
        "context": {
            "tasks": [
                {
                    "id": "3f2a1b9c",
                    "title": "Add pricing section",
                    "state": "review",
                    "project": "landing",
                }
            ]
        },
    }
    with urllib.request.urlopen(_post(server, payload), timeout=5) as response:
        body = json.loads(response.read())
    assert body["decision"]["intent"] == "merge_worktree"
    assert body["meta"]["backend"] == "mock"


def test_route_rejects_bad_body(server) -> None:
    with pytest.raises(urllib.error.HTTPError) as error:
        urllib.request.urlopen(_post(server, {"context": {}}), timeout=5)
    assert error.value.code == 400


def test_token_is_enforced() -> None:
    instance = build_server(MockBackend(), port=0, token="secret")
    thread = threading.Thread(target=instance.serve_forever, daemon=True)
    thread.start()
    try:
        with pytest.raises(urllib.error.HTTPError) as error:
            urllib.request.urlopen(_post(instance, {"text": "ayuda"}), timeout=5)
        assert error.value.code == 401
        with urllib.request.urlopen(
            _post(instance, {"text": "ayuda"}, token="secret"), timeout=5
        ) as response:
            assert response.status == 200
    finally:
        instance.shutdown()
        instance.server_close()
        thread.join(timeout=5)


class BrokenBackend:
    name = "broken"
    model = "none"

    def route(self, text, context=None, gold=None):
        raise RouteBackendError("model exploded")


def test_backend_failure_is_503() -> None:
    instance = build_server(BrokenBackend(), port=0)
    thread = threading.Thread(target=instance.serve_forever, daemon=True)
    thread.start()
    try:
        with pytest.raises(urllib.error.HTTPError) as error:
            urllib.request.urlopen(_post(instance, {"text": "ayuda"}), timeout=5)
        assert error.value.code == 503
    finally:
        instance.shutdown()
        instance.server_close()
        thread.join(timeout=5)
