"""Wire-level SOCKS5 qualification against HTTPX 0.28.1.

The fixture records the negotiation and CONNECT bytes, then relays a local
HTTP/1.1 origin. Each scenario is executed once against the pinned reference
and once against the compatibility facade.
"""

from __future__ import annotations

import asyncio
import http.server
import select
import socket
import socketserver
import threading
from contextlib import contextmanager

import httpx
import pytest

from eggfetch.compat.httpx import Client, Proxy

from .native_fixtures import local_tls_server


class _OriginHandler(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def do_GET(self):
        self.connection.sendall(
            b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\n"
            b"Connection: keep-alive\r\n\r\nsocks-ok"
        )

    def log_message(self, *_args):
        pass


class _StallingOriginHandler(_OriginHandler):
    started = threading.Event()
    release = threading.Event()

    def do_GET(self):
        if self.path == "/stall":
            self.__class__.started.set()
            self.__class__.release.wait(timeout=5)
        super().do_GET()


@contextmanager
def _origin():
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), _OriginHandler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield server.server_port
    finally:
        server.shutdown()
        thread.join(timeout=2)


def _read_exact(sock: socket.socket, length: int) -> bytes:
    data = bytearray()
    while len(data) < length:
        chunk = sock.recv(length - len(data))
        if not chunk:
            raise ConnectionError("SOCKS fixture connection closed")
        data.extend(chunk)
    return bytes(data)


class _SocksHandler(socketserver.BaseRequestHandler):
    def handle(self):
        state = self.server.state
        record = {"methods": None, "auth": None, "connect": None, "origin_target": None}
        state["records"].append(record)
        client = self.request
        client.settimeout(4)
        try:
            version, method_count = _read_exact(client, 2)
            methods = _read_exact(client, method_count)
            assert version == 5
            record["methods"] = methods
            selected = self.server.selected_method
            client.sendall(bytes((5, selected)))

            if selected == 2:
                subversion, username_length = _read_exact(client, 2)
                username = _read_exact(client, username_length)
                password_length = _read_exact(client, 1)[0]
                password = _read_exact(client, password_length)
                record["auth"] = (subversion, username, password)
                if self.server.reject_auth:
                    client.sendall(b"\x01\x01")
                    return
                client.sendall(b"\x01\x00")
            elif selected != 0:
                return

            header = _read_exact(client, 4)
            if header[:3] != b"\x05\x01\x00":
                return
            address_type = header[3]
            if address_type == 1:
                host = socket.inet_ntoa(_read_exact(client, 4))
            elif address_type == 3:
                host_length = _read_exact(client, 1)[0]
                host = _read_exact(client, host_length).decode("ascii")
            elif address_type == 4:
                host = socket.inet_ntop(socket.AF_INET6, _read_exact(client, 16))
            else:
                return
            port = int.from_bytes(_read_exact(client, 2), "big")
            record["connect"] = (address_type, host, port)

            try:
                upstream = socket.create_connection((host, port), timeout=2)
            except OSError:
                client.sendall(b"\x05\x05\x00\x01\x00\x00\x00\x00\x00\x00")
                return

            with upstream:
                client.sendall(b"\x05\x00\x00\x01\x7f\x00\x00\x01\x00\x00")
                self._relay(client, upstream, record)
        except (AssertionError, ConnectionError, OSError, socket.timeout):
            return

    @staticmethod
    def _relay(client: socket.socket, upstream: socket.socket, record: dict):
        client.settimeout(None)
        upstream.settimeout(None)
        request_prefix = bytearray()
        while True:
            readable, _, _ = select.select([client, upstream], [], [], 4)
            if not readable:
                return
            for source in readable:
                data = source.recv(65536)
                if not data:
                    return
                destination = upstream if source is client else client
                destination.sendall(data)
                if source is client and record["origin_target"] is None:
                    request_prefix.extend(data)
                    if b"\r\n" in request_prefix:
                        first_line = bytes(request_prefix).split(b"\r\n", 1)[0]
                        fields = first_line.split(b" ", 2)
                        if len(fields) >= 2:
                            record["origin_target"] = fields[1].decode("ascii")


class _SocksServer(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True

    def __init__(self, selected_method=0, reject_auth=False):
        super().__init__(("127.0.0.1", 0), _SocksHandler)
        self.selected_method = selected_method
        self.reject_auth = reject_auth
        self.state = {"records": []}


@contextmanager
def _socks_server(*, selected_method=0, reject_auth=False):
    server = _SocksServer(selected_method, reject_auth)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield server
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=2)


def _get(runtime, proxy_url, target_url, *, verify=True):
    if runtime == "reference":
        with httpx.Client(
            proxy=proxy_url, verify=verify, trust_env=False, timeout=3
        ) as client:
            return client.get(target_url)
    with Client(
        proxy=proxy_url, verify=verify, trust_env=False, timeout=3
    ) as client:
        return client.get(target_url)


@contextmanager
def _client(runtime, proxy_url):
    if runtime == "reference":
        with httpx.Client(proxy=proxy_url, trust_env=False, timeout=3) as client:
            yield client
    else:
        with Client(proxy=proxy_url, trust_env=False, timeout=3) as client:
            yield client


@pytest.mark.parametrize("runtime", ["reference", "candidate"])
def test_socks_https_uses_origin_tls_and_reuses_connection(runtime):
    with local_tls_server() as (host, origin_port, _client_ssl, cert_path):
        with _socks_server() as proxy:
            proxy_url = f"socks5://127.0.0.1:{proxy.server_address[1]}"
            target = f"https://{host}:{origin_port}/health"
            response = _get(runtime, proxy_url, target, verify=cert_path)
            assert response.status_code == 200
            assert response.text == "ok"
            records = proxy.state["records"]
            assert len(records) == 1
            assert records[0]["connect"] == (1, "127.0.0.1", origin_port)


@pytest.mark.parametrize("runtime", ["reference", "candidate"])
def test_socks_cancellation_allows_same_route_follow_up(runtime):
    _StallingOriginHandler.started = threading.Event()
    _StallingOriginHandler.release = threading.Event()
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), _StallingOriginHandler)
    origin_thread = threading.Thread(target=server.serve_forever, daemon=True)
    origin_thread.start()
    try:
        async def scenario():
            with _socks_server() as proxy:
                proxy_url = f"socks5://127.0.0.1:{proxy.server_address[1]}"
                target = f"http://127.0.0.1:{server.server_port}"
                if runtime == "reference":
                    client_type = httpx.AsyncClient
                else:
                    from eggfetch.compat.httpx import AsyncClient
                    client_type = AsyncClient
                async with client_type(
                    proxy=proxy_url, trust_env=False, timeout=5
                ) as client:
                    pending = asyncio.create_task(client.get(f"{target}/stall"))
                    await asyncio.to_thread(_StallingOriginHandler.started.wait, 2)
                    pending.cancel()
                    with pytest.raises(asyncio.CancelledError):
                        await pending
                    _StallingOriginHandler.release.set()
                    response = await client.get(f"{target}/follow-up")
                    assert response.text == "socks-ok"
                    assert len(proxy.state["records"]) >= 2

        asyncio.run(scenario())
    finally:
        _StallingOriginHandler.release.set()
        server.shutdown()
        origin_thread.join(timeout=2)
@pytest.mark.parametrize("runtime", ["reference", "candidate"])
def test_socks_no_auth_reuses_same_route_connection(runtime):
    with _origin() as origin_port, _socks_server() as proxy:
        proxy_url = f"socks5://127.0.0.1:{proxy.server_address[1]}"
        target = f"http://127.0.0.1:{origin_port}/path?query=1"
        with _client(runtime, proxy_url) as client:
            assert client.get(target).text == "socks-ok"
            assert client.get(target).text == "socks-ok"
        records = proxy.state["records"]
        assert len(records) == 1
        assert records[0]["methods"] == b"\x00"
        assert records[0]["connect"] == (1, "127.0.0.1", origin_port)
        assert records[0]["origin_target"] == "/path?query=1"


def test_socks_auth_wire_matches_reference():
    results = []
    for runtime in ("reference", "candidate"):
        with _origin() as origin_port, _socks_server(selected_method=2) as proxy:
            proxy_url = (
                f"socks5://user%40name:p%40ss@127.0.0.1:{proxy.server_address[1]}"
            )
            target = f"http://127.0.0.1:{origin_port}/auth"
            assert _get(runtime, proxy_url, target).text == "socks-ok"
            results.append(proxy.state["records"][0])

    assert [record["methods"] for record in results] == [b"\x02", b"\x02"]
    assert [record["auth"][1:] for record in results] == [
        (b"user@name", b"p@ss"),
        (b"user@name", b"p@ss"),
    ]


@pytest.mark.parametrize("scheme", ["socks5", "socks5h"])
def test_socks_hostname_address_type_matches_reference(scheme):
    results = []
    for runtime in ("reference", "candidate"):
        with _origin() as origin_port, _socks_server() as proxy:
            proxy_url = f"{scheme}://127.0.0.1:{proxy.server_address[1]}"
            target = f"http://localhost:{origin_port}/hostname"
            assert _get(runtime, proxy_url, target).status_code == 200
            results.append(proxy.state["records"][0]["connect"])

    assert results[0][0] == results[1][0] == 3
    assert results[0][1] == results[1][1] == "localhost"


@pytest.mark.parametrize("selected_method", [2, 255])
def test_socks_rejects_unoffered_or_unacceptable_method(selected_method):
    with _origin() as origin_port, _socks_server(selected_method=selected_method) as proxy:
        proxy_url = f"socks5://127.0.0.1:{proxy.server_address[1]}"
        target = f"http://127.0.0.1:{origin_port}/rejected"
        with pytest.raises(Exception):
            _get("candidate", proxy_url, target)
        assert proxy.state["records"][0]["methods"] == b"\x00"
