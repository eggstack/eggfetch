"""Differential qualification for HTTPX's Unix-domain transport options."""

from __future__ import annotations

import asyncio
import http.server
import os
import socket
import ssl
import subprocess
import tempfile
import threading
from contextlib import contextmanager

import httpx
import pytest

from eggfetch.compat.httpx import AsyncClient, AsyncHTTPTransport, Client, HTTPTransport


class _AddressHandler(http.server.BaseHTTPRequestHandler):
    observed: list[str] = []

    def do_GET(self):
        self.__class__.observed.append(self.client_address[0])
        self.send_response(200)
        self.send_header("Content-Length", "2")
        self.end_headers()
        self.wfile.write(b"ok")

    def log_message(self, *_args):
        pass


class _UnixHTTPServer:
    def __init__(self, path: str, *, tls: bool, cert_path: str | None = None,
                 key_path: str | None = None) -> None:
        self.path = path
        self.tls = tls
        self.cert_path = cert_path
        self.key_path = key_path
        self.connections = 0
        self.requests = 0
        self._lock = threading.Lock()
        self._stop = threading.Event()
        self._clients: list[socket.socket] = []
        self._listener = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self._listener.bind(path)
        self._listener.listen(8)
        self._listener.settimeout(0.2)
        self._thread = threading.Thread(target=self._serve, daemon=True)
        self._thread.start()

    def _serve(self) -> None:
        context = None
        if self.tls:
            context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
            assert self.cert_path is not None and self.key_path is not None
            context.load_cert_chain(self.cert_path, self.key_path)
        while not self._stop.is_set():
            try:
                client, _ = self._listener.accept()
            except socket.timeout:
                continue
            except OSError:
                break
            with self._lock:
                self.connections += 1
                self._clients.append(client)
            thread = threading.Thread(
                target=self._handle_client, args=(client, context), daemon=True
            )
            thread.start()

    def _handle_client(self, client: socket.socket, context: ssl.SSLContext | None) -> None:
        try:
            if context is not None:
                client = context.wrap_socket(client, server_side=True)
            client.settimeout(2)
            while not self._stop.is_set():
                request = bytearray()
                while b"\r\n\r\n" not in request:
                    chunk = client.recv(4096)
                    if not chunk:
                        return
                    request.extend(chunk)
                header_end = request.index(b"\r\n\r\n") + 4
                headers = bytes(request[:header_end]).decode("latin-1")
                path = headers.split(" ", 2)[1]
                with self._lock:
                    self.requests += 1
                if path == "/chunked":
                    response = (
                        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n"
                        b"Connection: keep-alive\r\n\r\n"
                        b"3\r\nuds\r\n4\r\n-ok!\r\n0\r\n\r\n"
                    )
                else:
                    body = b"uds-ok"
                    response = (
                        b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\n"
                        b"Connection: keep-alive\r\n\r\n" + body
                    )
                client.sendall(response)
        except (OSError, ssl.SSLError, socket.timeout):
            return
        finally:
            try:
                client.close()
            except OSError:
                pass

    def close(self) -> None:
        self._stop.set()
        self._listener.close()
        with self._lock:
            clients = list(self._clients)
        for client in clients:
            try:
                client.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass
            client.close()
        self._thread.join(timeout=2)
        try:
            os.unlink(self.path)
        except FileNotFoundError:
            pass


@contextmanager
def _unix_server(*, tls: bool):
    with tempfile.TemporaryDirectory() as directory:
        path = os.path.join(directory, "eggfetch.sock")
        cert_path = key_path = None
        if tls:
            cert_path = os.path.join(directory, "cert.pem")
            key_path = os.path.join(directory, "key.pem")
            subprocess.run(
                [
                    "openssl", "req", "-x509", "-newkey", "rsa:2048",
                    "-keyout", key_path, "-out", cert_path, "-days", "1",
                    "-nodes", "-subj", "/CN=example.test",
                    "-addext", "basicConstraints=critical,CA:FALSE",
                    "-addext", "subjectAltName=DNS:example.test",
                ],
                check=True,
                capture_output=True,
            )
        server = _UnixHTTPServer(
            path, tls=tls, cert_path=cert_path, key_path=key_path
        )
        try:
            yield server, path, cert_path
        finally:
            server.close()


def _sync_request(runtime: str, path: str, *, tls: bool, cert_path: str | None):
    scheme = "https" if tls else "http"
    url = f"{scheme}://example.test/{'chunked' if tls else 'fixed'}"
    if runtime == "reference":
        transport = httpx.HTTPTransport(
            uds=path, verify=cert_path if tls else False
        )
        client_type = httpx.Client
    else:
        transport = HTTPTransport(uds=path, verify=cert_path if tls else False)
        client_type = Client
    with client_type(transport=transport, trust_env=False, timeout=3) as client:
        first = client.get(url)
        second = client.get(url)
        return first.status_code, first.text, second.text


async def _async_request(runtime: str, path: str, *, tls: bool, cert_path: str | None):
    scheme = "https" if tls else "http"
    url = f"{scheme}://example.test/chunked"
    if runtime == "reference":
        transport = httpx.AsyncHTTPTransport(
            uds=path, verify=cert_path if tls else False
        )
        client_type = httpx.AsyncClient
    else:
        transport = AsyncHTTPTransport(uds=path, verify=cert_path if tls else False)
        client_type = AsyncClient
    async with client_type(transport=transport, trust_env=False, timeout=3) as client:
        first = await client.get(url)
        second = await client.get(url)
        return first.status_code, first.text, second.text


@pytest.mark.parametrize("runtime", ["reference", "candidate"])
def test_uds_http_fixed_response_reuses_connection(runtime):
    with _unix_server(tls=False) as (server, path, cert_path):
        assert _sync_request(runtime, path, tls=False, cert_path=cert_path) == (
            200, "uds-ok", "uds-ok"
        )
        assert server.connections == 1
        assert server.requests == 2


@pytest.mark.parametrize("runtime", ["reference", "candidate"])
def test_uds_https_chunked_response(runtime):
    with _unix_server(tls=True) as (server, path, cert_path):
        result = asyncio.run(
            _async_request(runtime, path, tls=True, cert_path=cert_path)
        )
        assert result == (200, "uds-ok!", "uds-ok!")
        assert server.connections == 1
        assert server.requests == 2


@pytest.mark.parametrize("runtime", ["reference", "candidate"])
def test_local_address_binds_loopback_source(runtime):
    _AddressHandler.observed = []
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), _AddressHandler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        url = f"http://127.0.0.1:{server.server_port}/health"
        if runtime == "reference":
            transport = httpx.HTTPTransport(local_address="127.0.0.1")
            client_type = httpx.Client
        else:
            transport = HTTPTransport(local_address="127.0.0.1")
            client_type = Client
        with client_type(transport=transport, trust_env=False, timeout=3) as client:
            assert client.get(url).text == "ok"
        assert _AddressHandler.observed == ["127.0.0.1"]
    finally:
        server.shutdown()
        thread.join(timeout=2)
