"""Native vs compat environment-semantics isolation (ADR-0009 rule 5).

The native ``eggfetch`` surface and the HTTPX compatibility facades must not
share proxy-environment semantics:

* the **native** client uses eggfetch's own ``NO_PROXY`` rules
  (``NoProxy::parse``) and passes a ``socks5://`` proxy URL through unchanged,
  so ``socks5://`` keeps its local-DNS meaning;
* the **compat** facades use HTTPX 0.28.1's ``NO_PROXY`` rules
  (``NoProxy::parse_httpx``) and normalize ``socks5://`` to ``socks5h://`` so
  the SOCKS proxy resolves the destination hostname.

The separation is explicit: the native constructor takes a ``proxy_env``
option that defaults to ``"native"``, and only the compatibility facade opts
into ``"httpx"``.  Native users never inherit compat semantics.

Every assertion below is made against the *observable* effect on the wire
(actual proxy selection, actual SOCKS CONNECT encoding), not against an
internal parser result.
"""

from __future__ import annotations

import os
import select
import socket
import socketserver
import threading
from contextlib import contextmanager
from unittest import mock

import httpx
import pytest

import eggfetch
from eggfetch.compat.httpx import Client as CompatClient

from .native_fixtures import local_http_server, local_proxy_server

# ---------------------------------------------------------------------------
# Part 1: NO_PROXY rule set separation
# ---------------------------------------------------------------------------

#: (no_proxy value, target host, native bypasses the proxy?, HTTPX proxies?)
#:
#: ``localhost`` is the discriminating entry: eggfetch's native rules treat a
#: ``localhost`` entry as covering the loopback IP literals, while HTTPX
#: 0.28.1's rules match the hostname ``localhost`` only.
NO_PROXY_CASES = [
    ("localhost", "127.0.0.1", True, True),
    ("localhost", "localhost", True, False),
    ("127.0.0.1", "127.0.0.1", True, False),
    ("*", "127.0.0.1", True, False),
    (".localhost", "127.0.0.1", False, True),
]


def _request(runtime: str, target: str, *, trust_env: bool = True):
    if runtime == "reference":
        with httpx.Client(trust_env=trust_env, timeout=3) as client:
            return client.get(target)
    if runtime == "facade":
        with CompatClient(trust_env=trust_env, timeout=3) as client:
            return client.get(target)
    if runtime == "native_httpx":
        with eggfetch.Client(trust_env=trust_env, timeout=3, proxy_env="httpx") as client:
            return client.get(target)
    with eggfetch.Client(trust_env=trust_env, timeout=3) as client:
        return client.get(target)


@pytest.mark.parametrize(
    ("no_proxy", "target_host", "native_bypasses", "httpx_proxies"),
    NO_PROXY_CASES,
)
def test_native_and_compat_use_different_no_proxy_rules(
    no_proxy, target_host, native_bypasses, httpx_proxies
):
    """The native client and the compat facade must not share NO_PROXY rules."""
    with local_http_server() as (backend_host, backend_port):
        with local_proxy_server(backend=(backend_host, backend_port)) as (
            proxy_host,
            proxy_port,
            handler,
        ):
            env = {
                "HTTP_PROXY": f"http://{proxy_host}:{proxy_port}",
                "NO_PROXY": no_proxy,
            }
            target = f"http://{target_host}:{backend_port}/health"
            observed: dict[str, bool] = {}
            with mock.patch.dict(os.environ, env, clear=True):
                for runtime in ("native", "facade", "reference", "native_httpx"):
                    assert _request(runtime, target).status_code == 200
                    observed[runtime] = bool(handler.recorded_requests)
                    handler.recorded_requests.clear()

    # The native client applies eggfetch's own NO_PROXY rules.
    assert observed["native"] is (not native_bypasses)
    # The compat facade applies HTTPX 0.28.1's NO_PROXY rules ...
    assert observed["facade"] is httpx_proxies
    # ... and so does an explicit ``proxy_env='httpx'`` opt-in, which must
    # agree with the facade and with upstream httpx in every case.
    assert observed["native_httpx"] is httpx_proxies
    assert observed["facade"] is observed["reference"]


def test_native_default_is_native_semantics_not_compat_semantics():
    """The native default must be eggfetch's own NO_PROXY rule set."""
    with local_http_server() as (backend_host, backend_port):
        with local_proxy_server(backend=(backend_host, backend_port)) as (
            proxy_host,
            proxy_port,
            handler,
        ):
            env = {
                "HTTP_PROXY": f"http://{proxy_host}:{proxy_port}",
                "NO_PROXY": "localhost",
            }
            target = f"http://127.0.0.1:{backend_port}/health"
            with mock.patch.dict(os.environ, env, clear=True):
                assert _request("native", target).status_code == 200
            assert handler.recorded_requests == [], (
                "the native client must bypass the proxy for a loopback target "
                "under eggfetch's own NO_PROXY rules"
            )


def test_native_proxy_env_option_opts_into_httpx_semantics_explicitly():
    """``proxy_env='httpx'`` is the explicit, native-visible opt-in."""
    with local_http_server() as (backend_host, backend_port):
        with local_proxy_server(backend=(backend_host, backend_port)) as (
            proxy_host,
            proxy_port,
            handler,
        ):
            env = {
                "HTTP_PROXY": f"http://{proxy_host}:{proxy_port}",
                "NO_PROXY": "localhost",
            }
            target = f"http://127.0.0.1:{backend_port}/health"
            with mock.patch.dict(os.environ, env, clear=True):
                with eggfetch.Client(trust_env=True, timeout=3, proxy_env="httpx") as client:
                    assert client.get(target).status_code == 200
                assert handler.recorded_requests, (
                    "proxy_env='httpx' must apply HTTPX's NO_PROXY rules"
                )
                handler.recorded_requests.clear()

                with eggfetch.Client(trust_env=True, timeout=3, proxy_env="native") as client:
                    assert client.get(target).status_code == 200
                assert handler.recorded_requests == [], (
                    "proxy_env='native' must apply eggfetch's own NO_PROXY rules"
                )


def test_unknown_proxy_env_value_is_rejected():
    """An unknown semantics value fails loudly instead of silently defaulting."""
    with pytest.raises(ValueError):
        eggfetch.Client(proxy_env="compat-ish")
    with pytest.raises(ValueError):
        eggfetch.AsyncClient(proxy_env="compat-ish")


# ---------------------------------------------------------------------------
# Part 2: SOCKS scheme separation
# ---------------------------------------------------------------------------


def _read_exact(sock: socket.socket, length: int) -> bytes:
    data = bytearray()
    while len(data) < length:
        chunk = sock.recv(length - len(data))
        if not chunk:
            raise ConnectionError("SOCKS fixture connection closed")
        data.extend(chunk)
    return bytes(data)


class _SocksHandler(socketserver.BaseRequestHandler):
    """Records the SOCKS5 CONNECT destination and relays to a fixed origin."""

    def handle(self) -> None:
        record: dict[str, object] = {}
        self.server.records.append(record)  # type: ignore[attr-defined]
        client = self.request
        client.settimeout(4)
        try:
            version, method_count = _read_exact(client, 2)
            methods = _read_exact(client, method_count)
            assert version == 5
            record["methods"] = methods
            client.sendall(b"\x05\x00")

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
            record["atyp"] = address_type
            record["host"] = host
            record["port"] = port

            # Report success and relay to the fixture's fixed origin; the
            # recorded destination is the observable under test.
            client.sendall(b"\x05\x00\x00\x01\x7f\x00\x00\x01\x00\x00")
            upstream = socket.create_connection(self.server.origin, timeout=2)  # type: ignore[attr-defined]
            with upstream:
                client.settimeout(None)
                upstream.settimeout(None)
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
        except (AssertionError, ConnectionError, OSError, socket.timeout):
            return


class _SocksServer(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True


@contextmanager
def _socks_server(origin: tuple[str, int]):
    server = _SocksServer(("127.0.0.1", 0), _SocksHandler)
    server.records = []  # type: ignore[attr-defined]
    server.origin = origin  # type: ignore[attr-defined]
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield server
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=2)


def _socks_request(runtime: str, proxy_url: str, target: str) -> None:
    if runtime == "reference":
        with httpx.Client(proxy=proxy_url, trust_env=False, timeout=3) as client:
            client.get(target)
        return
    if runtime == "facade":
        with CompatClient(proxy=proxy_url, trust_env=False, timeout=3) as client:
            client.get(target)
        return
    with eggfetch.Client(proxy=proxy_url, trust_env=False, timeout=3) as client:
        client.get(target)


def test_native_leaves_socks5_scheme_alone_and_compat_rewrites_it():
    """``socks5://`` means local DNS natively and remote DNS in the facade."""
    with local_http_server() as (backend_host, backend_port):
        with _socks_server((backend_host, backend_port)) as server:
            proxy_url = f"socks5://127.0.0.1:{server.server_address[1]}"
            # The destination is a hostname, so the two DNS policies differ
            # in the SOCKS CONNECT address type: a domain name (ATYP 3) for
            # remote DNS, an IP literal (ATYP 1/4) for local DNS.
            target = f"http://localhost:{backend_port}/health"

            server.records.clear()
            _socks_request("native", proxy_url, target)
            native_records = list(server.records)

            server.records.clear()
            _socks_request("facade", proxy_url, target)
            facade_records = list(server.records)

            server.records.clear()
            _socks_request("reference", proxy_url, target)
            reference_records = list(server.records)

    assert native_records, "the native client did not reach the SOCKS proxy"
    assert facade_records, "the compat facade did not reach the SOCKS proxy"

    # Native: local DNS -> the CONNECT carries a resolved IP literal.
    assert native_records[0]["atyp"] in (1, 4), native_records
    assert native_records[0]["host"] != "localhost", native_records

    # Compat: the socks5 -> socks5h rewrite makes the proxy resolve.
    assert facade_records[0]["atyp"] == 3, facade_records
    assert facade_records[0]["host"] == "localhost", facade_records

    # Upstream httpx (httpcore) always sends the hostname to the proxy.
    assert reference_records, "upstream httpx did not reach the SOCKS proxy"
    assert reference_records[0]["atyp"] == 3, reference_records
    assert reference_records[0]["host"] == "localhost", reference_records


def test_native_proxy_env_option_selects_the_socks_scheme_policy():
    """``proxy_env='httpx'`` also selects the socks5 -> socks5h rewrite."""
    with local_http_server() as (backend_host, backend_port):
        with _socks_server((backend_host, backend_port)) as server:
            proxy_url = f"socks5://127.0.0.1:{server.server_address[1]}"
            target = f"http://localhost:{backend_port}/health"

            server.records.clear()
            with eggfetch.Client(
                proxy=proxy_url, trust_env=False, timeout=3, proxy_env="httpx"
            ) as client:
                client.get(target)
            httpx_records = list(server.records)

    assert httpx_records, "the native client did not reach the SOCKS proxy"
    assert httpx_records[0]["atyp"] == 3, httpx_records
    assert httpx_records[0]["host"] == "localhost", httpx_records


# ---------------------------------------------------------------------------
# Part 3: httpx2 inherits the compat semantics without touching httpx2
# ---------------------------------------------------------------------------


def test_httpx2_client_inherits_the_compat_environment_semantics():
    """``eggfetch.compat.httpx2`` must behave like the httpx facade."""
    from eggfetch.compat import httpx2

    with local_http_server() as (backend_host, backend_port):
        with local_proxy_server(backend=(backend_host, backend_port)) as (
            proxy_host,
            proxy_port,
            handler,
        ):
            env = {
                "HTTP_PROXY": f"http://{proxy_host}:{proxy_port}",
                "NO_PROXY": "localhost",
            }
            target = f"http://127.0.0.1:{backend_port}/health"
            with mock.patch.dict(os.environ, env, clear=True):
                with httpx2.Client(trust_env=True, timeout=3) as client:
                    assert client.get(target).status_code == 200
                assert handler.recorded_requests, (
                    "httpx2 must apply HTTPX's NO_PROXY rules through the "
                    "shared base facade"
                )


def test_httpx2_does_not_redefine_client_construction():
    """The httpx2 facade inherits the base facade's constructor unchanged.

    This is what makes the base facade's ``proxy_env='httpx'`` opt-in
    propagate to ``eggfetch.compat.httpx2`` without editing any httpx2
    module.
    """
    from eggfetch.compat import httpx2
    from eggfetch.compat.httpx import _client as base_client

    assert httpx2.Client.__init__ is base_client.Client.__init__
    assert httpx2.AsyncClient.__init__ is base_client.AsyncClient.__init__
    assert "_ensure_client" not in httpx2.Client.__dict__
    assert "_ensure_client" not in httpx2.AsyncClient.__dict__
