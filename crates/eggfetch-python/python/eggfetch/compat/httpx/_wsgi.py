"""WSGI transport for testing WSGI applications with the HTTPX-compatible client."""

from __future__ import annotations

import io
import sys
import typing
from typing import Callable

from eggfetch.compat.httpx._request import Request
from eggfetch.compat.httpx._response import Response
from eggfetch.compat.httpx._transports import BaseTransport

if typing.TYPE_CHECKING:
    pass


class WSGITransport(BaseTransport):
    """Transport for making requests to a WSGI application.

    Usage::

        def app(environ, start_response):
            start_response("200 OK", [("Content-Type", "text/plain")])
            return [b"Hello, World!"]

        with Client(transport=WSGITransport(app)) as client:
            response = client.get("http://testserver/")
    """

    def __init__(
        self,
        app: Callable,
        raise_app_exceptions: bool = True,
        script_name: str = "",
        remote_addr: str = "127.0.0.1",
        wsgi_errors=None,
    ) -> None:
        self._app = app
        self._raise_app_exceptions = raise_app_exceptions
        self._script_name = script_name
        self._remote_addr = remote_addr
        self._wsgi_errors = wsgi_errors

    def handle_request(self, request: Request) -> Response:
        """Convert HTTPX request to WSGI environ and call the app."""
        environ = self._build_environ(request)

        status: str | None = None
        response_headers: list[tuple[str, str]] = []
        exc_info_stored: tuple | None = None

        def start_response(
            wsgi_status: str,
            wsgi_headers: list[tuple[str, str]],
            exc_info: tuple | None = None,
        ) -> None:
            nonlocal status, exc_info_stored
            status = wsgi_status
            response_headers.clear()
            response_headers.extend(wsgi_headers)
            if exc_info is not None:
                exc_info_stored = exc_info

        app_iter = None
        body = b""

        try:
            app_iter = self._app(environ, start_response)
            parts: list[bytes] = []
            for part in app_iter:
                if isinstance(part, str):
                    part = part.encode("utf-8")
                parts.append(part)
            body = b"".join(parts)
        except Exception:
            if self._raise_app_exceptions:
                raise
            status = "500 Internal Server Error"
            response_headers = [("Content-Type", "text/plain")]
            body = b"Internal Server Error"
        finally:
            if app_iter is not None and hasattr(app_iter, "close"):
                try:
                    app_iter.close()
                except Exception:
                    pass

        # If start_response was called with exc_info, re-raise the stored
        # exception (matching HTTPX behaviour where exc_info causes the
        # original exception to propagate after the response is built).
        if exc_info_stored is not None and self._raise_app_exceptions:
            exc_type, exc_value, _tb = exc_info_stored
            if exc_value is not None:
                raise exc_value

        if status is None:
            status = "500 Internal Server Error"

        status_code = int(status.split(" ", 1)[0])

        return Response(
            status_code,
            headers=response_headers,
            content=body,
        )

    def _build_environ(self, request: Request) -> dict:
        """Build a WSGI environ dict from an HTTPX Request."""
        url = request.url

        scheme = url.scheme or "http"
        host = url.host or "localhost"
        port = url.port
        path = url.path or "/"

        query = ""
        if url.query:
            q = url.query
            query = q.decode() if isinstance(q, bytes) else str(q)

        server_port = port or (443 if scheme == "https" else 80)

        environ: dict = {
            "REQUEST_METHOD": request.method,
            "SCRIPT_NAME": self._script_name,
            "PATH_INFO": path,
            "QUERY_STRING": query,
            "SERVER_NAME": host,
            "SERVER_PORT": str(server_port),
            "SERVER_PROTOCOL": "HTTP/1.1",
            "HTTPS": "on" if scheme == "https" else "",
            "wsgi.version": (1, 0),
            "wsgi.url_scheme": scheme,
            "wsgi.input": io.BytesIO(request.content or b""),
            "wsgi.errors": sys.stderr,
            "wsgi.multithread": False,
            "wsgi.multiprocess": False,
            "wsgi.run_once": False,
            "REMOTE_ADDR": self._remote_addr,
        }

        for name, value in request.headers.items():
            wsgi_name = "HTTP_" + name.upper().replace("-", "_")
            environ[wsgi_name] = value

        if "content-type" in request.headers:
            environ["CONTENT_TYPE"] = request.headers["content-type"]
        if "content-length" in request.headers:
            environ["CONTENT_LENGTH"] = request.headers["content-length"]

        return environ

    def close(self) -> None:
        pass

    def __enter__(self) -> WSGITransport:
        return self

    def __exit__(self, *args: typing.Any) -> None:
        self.close()
