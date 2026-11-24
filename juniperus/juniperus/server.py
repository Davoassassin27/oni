"""Sidecar HTTP local para Juniperus.

Endpoints (sin CORS, bind localhost por defecto):

* ``GET  /health``  estado del backend y del contrato.
* ``GET  /schema``  JSON Schema del contrato v1.
* ``POST /route``   ``{"text": "...", "context": {...}}`` -> decisión validada.

Oni llama a ``/route``. Si el sidecar no está o falla, el daemon usa su router
determinístico: el SLM nunca es un punto único de falla.
"""

from __future__ import annotations

import json
import os
import sys
import time
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any

from schema.intent_schema import CONTRACT_VERSION, RouterContext, decision_schema

from .runtime import (
    Backend,
    RouteBackendError,
    build_backend,
    result_payload,
    route_text,
)

MAX_BODY = 64 * 1024


class RouterServer(ThreadingHTTPServer):
    daemon_threads = True
    allow_reuse_address = True

    def __init__(self, address: tuple[str, int], backend: Backend, token: str | None = None):
        super().__init__(address, RouterHandler)
        self.backend = backend
        self.token = token
        self.started = time.time()


class RouterHandler(BaseHTTPRequestHandler):
    server: RouterServer

    def log_message(self, format: str, *args: Any) -> None:  # noqa: A002
        sys.stderr.write(f"[juniperus] {self.address_string()} {format % args}\n")

    def _authorized(self) -> bool:
        if not self.server.token:
            return True
        header = self.headers.get("Authorization", "")
        if header.startswith("Bearer ") and header[7:] == self.server.token:
            return True
        return self.headers.get("X-Juniperus-Token") == self.server.token

    def _json(self, status: int, payload: dict[str, Any]) -> None:
        body = json.dumps(payload, ensure_ascii=False).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self) -> None:  # noqa: N802
        path = self.path.split("?", 1)[0]
        if path == "/health":
            self._json(
                HTTPStatus.OK,
                {
                    "ok": True,
                    "service": "juniperus",
                    "contract": CONTRACT_VERSION,
                    "backend": self.server.backend.name,
                    "model": self.server.backend.model,
                    "auth_required": bool(self.server.token),
                },
            )
            return
        if path == "/schema":
            self._json(HTTPStatus.OK, decision_schema())
            return
        self._json(HTTPStatus.NOT_FOUND, {"error": "not found"})

    def do_POST(self) -> None:  # noqa: N802
        path = self.path.split("?", 1)[0]
        if path != "/route":
            self._json(HTTPStatus.NOT_FOUND, {"error": "not found"})
            return
        if not self._authorized():
            self._json(HTTPStatus.UNAUTHORIZED, {"error": "missing or invalid token"})
            return

        try:
            length = int(self.headers.get("Content-Length", "0"))
        except ValueError:
            self._json(HTTPStatus.BAD_REQUEST, {"error": "invalid Content-Length"})
            return
        if length <= 0 or length > MAX_BODY:
            self._json(HTTPStatus.BAD_REQUEST, {"error": "invalid body size"})
            return

        try:
            body = json.loads(self.rfile.read(length).decode("utf-8"))
        except (UnicodeDecodeError, json.JSONDecodeError):
            self._json(HTTPStatus.BAD_REQUEST, {"error": "body must be JSON"})
            return
        if not isinstance(body, dict):
            self._json(HTTPStatus.BAD_REQUEST, {"error": "body must be a JSON object"})
            return

        text = body.get("text")
        if not isinstance(text, str) or not text.strip():
            self._json(HTTPStatus.BAD_REQUEST, {"error": "'text' is required"})
            return
        if len(text) > 8000:
            self._json(HTTPStatus.BAD_REQUEST, {"error": "'text' is too long (max 8000)"})
            return

        try:
            context = RouterContext.model_validate(body.get("context") or {})
        except Exception as error:
            self._json(HTTPStatus.BAD_REQUEST, {"error": f"invalid context: {error}"})
            return

        try:
            result = route_text(self.server.backend, text, context)
        except RouteBackendError as error:
            self._json(
                HTTPStatus.SERVICE_UNAVAILABLE,
                {"error": str(error), "backend": self.server.backend.name},
            )
            return
        except Exception as error:
            self._json(HTTPStatus.INTERNAL_SERVER_ERROR, {"error": str(error)})
            return

        self._json(HTTPStatus.OK, result_payload(result))


def build_server(
    backend: Backend,
    host: str = "127.0.0.1",
    port: int = 7767,
    token: str | None = None,
) -> RouterServer:
    return RouterServer((host, port), backend, token)


def serve(
    *,
    backend: str = "auto",
    model_path: Path = Path("models/juniperus-v1.gguf"),
    base_model: str = "Qwen/Qwen2.5-0.5B-Instruct",
    adapter: Path = Path("models/juniperus-lora"),
    host: str = "127.0.0.1",
    port: int = 7767,
    token: str | None = None,
    threads: int | None = None,
    gpu_layers: int = 0,
) -> None:
    backend_instance = build_backend(
        backend,
        model_path=model_path,
        base_model=base_model,
        adapter=adapter,
        n_threads=threads,
        n_gpu_layers=gpu_layers,
    )
    server = build_server(backend_instance, host, port, token)
    print(
        f"juniperus {CONTRACT_VERSION} · backend={backend_instance.name} "
        f"model={backend_instance.model or '-'} · http://{host}:{server.server_port}"
    )
    print("routes: GET /health · GET /schema · POST /route")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\nshutting down")
    finally:
        server.server_close()


def main(argv: list[str] | None = None) -> int:
    import argparse

    from dotenv import load_dotenv

    load_dotenv()
    parser = argparse.ArgumentParser(description="Run the Juniperus local router sidecar")
    parser.add_argument(
        "--backend", choices=["auto", "mock", "llama", "transformers"], default="auto"
    )
    parser.add_argument(
        "--model-path",
        type=Path,
        default=Path(os.environ.get("JUNIPERUS_MODEL_PATH", "models/juniperus-v1.gguf")),
    )
    parser.add_argument(
        "--base-model",
        default=os.environ.get("JUNIPERUS_BASE_MODEL", "Qwen/Qwen2.5-0.5B-Instruct"),
    )
    parser.add_argument(
        "--adapter",
        type=Path,
        default=Path(os.environ.get("JUNIPERUS_ADAPTER", "models/juniperus-lora")),
    )
    parser.add_argument("--host", default=os.environ.get("JUNIPERUS_HOST", "127.0.0.1"))
    parser.add_argument("--port", type=int, default=int(os.environ.get("JUNIPERUS_PORT", "7767")))
    parser.add_argument("--token", default=os.environ.get("JUNIPERUS_TOKEN") or None)
    parser.add_argument("--threads", type=int, default=None)
    parser.add_argument("--gpu-layers", type=int, default=0)
    args = parser.parse_args(argv)

    serve(
        backend=args.backend,
        model_path=args.model_path,
        base_model=args.base_model,
        adapter=args.adapter,
        host=args.host,
        port=args.port,
        token=args.token,
        threads=args.threads,
        gpu_layers=args.gpu_layers,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
