"""Reference/candidate route-selection evidence for HTTPX NO_PROXY rules."""

from __future__ import annotations

import os
from unittest import mock

import httpx
import pytest
from httpx._utils import URLPattern

from eggfetch.compat.httpx import Client, InvalidURL

from .native_fixtures import (
    local_http_server,
    local_ipv6_http_server,
    local_proxy_server,
    local_tls_server,
    DelayedResponseHandler,
)


class _RecordingOriginHandler(DelayedResponseHandler):
    """Record origin dispatches for pre-dispatch environment failures."""

    recorded_requests = 0

    def do_GET(self):
        type(self).recorded_requests += 1
        super().do_GET()


def _request(runtime: str, target: str):
    if runtime == "reference":
        with httpx.Client(trust_env=True, timeout=3) as client:
            return client.get(target)
    with Client(trust_env=True, timeout=3) as client:
        return client.get(target)


@pytest.mark.parametrize("runtime", ["reference", "candidate"])
@pytest.mark.parametrize(
    ("no_proxy", "target_host", "expected_proxy"),
    [
        ("*", "localhost", False),
        ("*,localhost", "localhost", False),
        (" localhost, ", "localhost", False),
        ("localhost", "localhost", False),
        (".localhost", "localhost", True),
        ("localhost:{port}", "localhost", False),
        ("localhost:{other_port}", "localhost", True),
        ("127.0.0.1", "127.0.0.1", False),
        ("localhost", "127.0.0.1", True),
        ("10.0.0.0/8", "127.0.0.1", True),
        ("http://localhost", "localhost", False),
        ("https://localhost", "localhost", True),
    ],
)
def test_no_proxy_route_selection_matches_reference(
    runtime, no_proxy, target_host, expected_proxy
):
    """Assert actual direct/proxy routing, not only parser output."""
    with local_http_server() as (backend_host, backend_port):
        with local_proxy_server(backend=(backend_host, backend_port)) as (
            proxy_host,
            proxy_port,
            handler,
        ):
            values = {
                "port": backend_port,
                "other_port": backend_port + 1,
            }
            env = {
                "HTTP_PROXY": f"http://{proxy_host}:{proxy_port}",
                "NO_PROXY": no_proxy.format(**values),
            }
            target = f"http://{target_host}:{backend_port}/health"
            with mock.patch.dict(os.environ, env, clear=True):
                response = _request(runtime, target)
            assert response.status_code == 200
            assert bool(handler.recorded_requests) is expected_proxy


@pytest.mark.parametrize("runtime", ["reference", "candidate"])
def test_no_proxy_trust_env_false_routes_directly(runtime):
    with local_http_server() as (backend_host, backend_port):
        with local_proxy_server(backend=(backend_host, backend_port)) as (
            proxy_host,
            proxy_port,
            handler,
        ):
            env = {
                "HTTP_PROXY": f"http://{proxy_host}:{proxy_port}",
                "NO_PROXY": "",
            }
            target = f"http://{backend_host}:{backend_port}/health"
            with mock.patch.dict(os.environ, env, clear=True):
                if runtime == "reference":
                    with httpx.Client(trust_env=False, timeout=3) as client:
                        response = client.get(target)
                else:
                    with Client(trust_env=False, timeout=3) as client:
                        response = client.get(target)
            assert response.status_code == 200
            assert not handler.recorded_requests


@pytest.mark.parametrize("runtime", ["reference", "candidate"])
@pytest.mark.parametrize(
    ("no_proxy", "target_host", "expected_proxy"),
    [
        ("localhost", "localhost", False),
        ("localhost", "foo.localhost", True),
        (".localhost", "localhost", True),
        (".localhost", "foo.localhost", False),
        (".foo.localhost", "localhost", True),
    ],
)
def test_no_proxy_domain_suffix_route_selection(runtime, no_proxy, target_host, expected_proxy):
    """Bare and leading-dot domains are proven through actual local routing."""
    try:
        with local_ipv6_http_server() as (backend_host, backend_port):
            with local_proxy_server(backend=(backend_host, backend_port)) as (
                proxy_host,
                proxy_port,
                handler,
            ):
                env = {
                    "HTTP_PROXY": f"http://{proxy_host}:{proxy_port}",
                    "NO_PROXY": no_proxy,
                }
                with mock.patch.dict(os.environ, env, clear=True):
                    response = _request(
                        runtime, f"http://{target_host}:{backend_port}/health"
                    )
                assert response.status_code == 200
                assert bool(handler.recorded_requests) is expected_proxy
    except OSError as exc:
        pytest.skip(f"IPv6 loopback unavailable: {exc}")


@pytest.mark.parametrize("runtime", ["reference", "candidate"])
@pytest.mark.parametrize(
    ("no_proxy", "target_host", "expected_proxy"),
    [
        ("foo.localhost", "foo.localhost", False),
        ("foo.localhost", "deep.foo.localhost", False),
        ("foo.localhost", "foox.localhost", True),
        (".foo.localhost", "foo.localhost", True),
        (".foo.localhost", "deep.foo.localhost", False),
    ],
)
def test_no_proxy_generic_domain_route_selection(
    runtime, no_proxy, target_host, expected_proxy
):
    """Prove generic bare/leading-dot domains through local route selection."""
    try:
        with local_ipv6_http_server() as (backend_host, backend_port):
            with local_proxy_server(backend=(backend_host, backend_port)) as (
                proxy_host,
                proxy_port,
                handler,
            ):
                env = {
                    "HTTP_PROXY": f"http://{proxy_host}:{proxy_port}",
                    "NO_PROXY": no_proxy,
                }
                with mock.patch.dict(os.environ, env, clear=True):
                    response = _request(
                        runtime, f"http://{target_host}:{backend_port}/health"
                    )
                assert response.status_code == 200
                assert bool(handler.recorded_requests) is expected_proxy
    except OSError as exc:
        pytest.skip(f"IPv6 loopback unavailable: {exc}")


@pytest.mark.parametrize(
    ("no_proxy", "target", "expected"),
    [
        ("example.test", "http://example.test/", True),
        ("example.test", "http://www.example.test/", True),
        ("example.test", "http://deep.www.example.test/", True),
        ("example.test", "http://wwwexample.test/", False),
        ("example.test", "http://notexample.test/", False),
        ("example.test", "http://example.test.evil/", False),
        (".example.test", "http://example.test/", False),
        (".example.test", "http://www.example.test/", True),
        (".example.test", "http://deep.www.example.test/", True),
    ],
)
def test_no_proxy_generic_domain_reference_patterns(no_proxy, target, expected):
    """Pin HTTPX's generic domain URL-pattern boundary independently of localhost."""
    pattern = f"all://*{no_proxy}"
    assert URLPattern(pattern).matches(httpx.URL(target)) is expected


@pytest.mark.parametrize("runtime", ["reference", "candidate"])
@pytest.mark.parametrize(
    ("no_proxy", "target_host", "expected_proxy"),
    [
        ("localhost:{port}", "foo.localhost", False),
        ("localhost:{other_port}", "foo.localhost", True),
        ("example.test:{port}", "localhost", True),
    ],
)
def test_no_proxy_domain_port_routing_is_observable(
    runtime, no_proxy, target_host, expected_proxy
):
    """Exercise candidate routing through local hostnames and a recording proxy."""
    with local_ipv6_http_server() as (backend_host, backend_port):
        with local_proxy_server(backend=(backend_host, backend_port)) as (
            proxy_host,
            proxy_port,
            handler,
        ):
            values = {"port": backend_port, "other_port": backend_port + 1}
            env = {
                "HTTP_PROXY": f"http://{proxy_host}:{proxy_port}",
                "NO_PROXY": no_proxy.format(**values),
            }
            with mock.patch.dict(os.environ, env, clear=True):
                response = _request(runtime, f"http://{target_host}:{backend_port}/health")
            assert response.status_code == 200
            assert bool(handler.recorded_requests) is expected_proxy


@pytest.mark.parametrize(
    ("pattern", "target", "expected"),
    [
        ("all://*example.test:80", "http://example.test/", False),
        ("all://*example.test:80", "https://example.test:80/", True),
        ("all://*example.test:80", "http://example.test:8080/", False),
        ("all://*example.test:80", "http://example.test:80/", False),
        ("all://*example.test:8080", "http://example.test:8080/", True),
        ("all://*example.test:8080", "http://www.example.test:8080/", True),
        ("all://*example.test:8080", "http://example.test:8081/", False),
        ("http://example.test", "http://example.test/", True),
        ("http://example.test", "https://example.test/", False),
        ("https://example.test", "https://example.test/", True),
        ("https://example.test", "http://example.test/", False),
        ("http://example.test:80", "http://example.test/", True),
        ("https://example.test:443", "https://example.test/", True),
        ("all://127.0.0.1", "http://127.0.0.1/", True),
        ("all://127.0.0.1", "http://127.0.0.2/", False),
        ("all://10.0.0.0/8", "http://10.0.0.0/", True),
        ("all://10.0.0.0/8", "http://10.42.1.9/", False),
        ("all://[::1]", "http://[::1]/", True),
        ("all://[::1]", "http://[::2]/", False),
    ],
)
def test_no_proxy_scheme_and_port_reference_patterns(pattern, target, expected):
    """Record HTTPX's normalized scheme/port URL-pattern behavior."""
    assert URLPattern(pattern).matches(httpx.URL(target)) is expected


@pytest.mark.parametrize("runtime", ["reference", "candidate"])
@pytest.mark.parametrize("no_proxy", ["::1"])
def test_no_proxy_ipv6_route_selection(runtime, no_proxy):
    """IPv6 loopback patterns are route-tested when the platform supports them."""
    try:
        with local_ipv6_http_server() as (backend_host, backend_port):
            with local_proxy_server(backend=(backend_host, backend_port)) as (
                proxy_host,
                proxy_port,
                handler,
            ):
                env = {
                    "HTTP_PROXY": f"http://{proxy_host}:{proxy_port}",
                    "NO_PROXY": no_proxy,
                }
                target = f"http://[{backend_host}]:{backend_port}/health"
                with mock.patch.dict(os.environ, env, clear=True):
                    response = _request(runtime, target)
                assert response.status_code == 200
                assert not handler.recorded_requests
    except OSError as exc:
        pytest.skip(f"IPv6 loopback unavailable: {exc}")


@pytest.mark.parametrize("runtime", ["reference", "candidate"])
@pytest.mark.parametrize(
    ("no_proxy", "expected_proxy"),
    [
        ("::1", False),
        ("::1:8080", True),
        ("2001:db8::1", True),
    ],
)
def test_no_proxy_ipv6_environment_route_selection(runtime, no_proxy, expected_proxy):
    """Compare accepted IPv6 environment forms through actual routing."""
    try:
        with local_ipv6_http_server() as (backend_host, backend_port):
            with local_proxy_server(backend=(backend_host, backend_port)) as (
                proxy_host,
                proxy_port,
                handler,
            ):
                env = {
                    "HTTP_PROXY": f"http://{proxy_host}:{proxy_port}",
                    "NO_PROXY": no_proxy,
                }
                factory = httpx.Client if runtime == "reference" else Client
                with mock.patch.dict(os.environ, env, clear=True):
                    with factory(trust_env=True, timeout=3) as client:
                        response = client.get(
                            f"http://[{backend_host}]:{backend_port}/health"
                        )
                assert response.status_code == 200
                assert bool(handler.recorded_requests) is expected_proxy
    except OSError as exc:
        pytest.skip(f"IPv6 loopback unavailable: {exc}")


@pytest.mark.parametrize("runtime", ["reference", "candidate"])
@pytest.mark.parametrize(
    "no_proxy",
    ["[::1]", "[::1]:8080", "::1/128", "[::1]/128"],
)
def test_no_proxy_ipv6_environment_rejection_is_pre_dispatch(runtime, no_proxy):
    """Malformed HTTPX IPv6 forms fail before either local endpoint is used."""
    origin_handler = _RecordingOriginHandler
    origin_handler.recorded_requests = 0
    with local_http_server(handler_class=origin_handler) as (
        backend_host,
        backend_port,
    ):
        with local_proxy_server(backend=(backend_host, backend_port)) as (
            proxy_host,
            proxy_port,
            handler,
        ):
            env = {
                "HTTP_PROXY": f"http://{proxy_host}:{proxy_port}",
                "NO_PROXY": no_proxy,
            }
            factory = httpx.Client if runtime == "reference" else Client
            expected_error = httpx.InvalidURL if runtime == "reference" else InvalidURL
            with mock.patch.dict(os.environ, env, clear=True):
                with pytest.raises(expected_error):
                    with factory(trust_env=True, timeout=3):
                        pass
            assert not handler.recorded_requests
            assert origin_handler.recorded_requests == 0


@pytest.mark.parametrize("runtime", ["reference", "candidate"])
@pytest.mark.parametrize(
    ("scheme", "no_proxy", "expected_proxy"),
    [
        ("http", "http://127.0.0.1", False),
        ("http", "https://127.0.0.1", True),
        ("https", "https://127.0.0.1", False),
        ("https", "http://127.0.0.1", True),
    ],
)
def test_no_proxy_scheme_qualified_patterns_route_selection(
    runtime, scheme, no_proxy, expected_proxy
):
    """Scheme-qualified patterns match only the target scheme."""
    if scheme == "http":
        origin_context = local_http_server()
    else:
        origin_context = local_tls_server()
    with origin_context as origin:
        origin_host, origin_port = origin[:2]
        with local_proxy_server(backend=(origin_host, origin_port)) as (
            proxy_host,
            proxy_port,
            handler,
        ):
            variable = "HTTP_PROXY" if scheme == "http" else "HTTPS_PROXY"
            target = f"{scheme}://{origin_host}:{origin_port}/health"
            env = {
                variable: f"http://{proxy_host}:{proxy_port}",
                "NO_PROXY": no_proxy,
            }
            with mock.patch.dict(os.environ, env, clear=True):
                if runtime == "reference":
                    kwargs = {"trust_env": True, "timeout": 3}
                    if scheme == "https":
                        kwargs["verify"] = False
                    with httpx.Client(**kwargs) as client:
                        response = client.get(target)
                else:
                    kwargs = {"trust_env": True, "timeout": 3}
                    if scheme == "https":
                        kwargs["verify"] = False
                    with Client(**kwargs) as client:
                        response = client.get(target)
            assert response.status_code == 200
            assert bool(handler.recorded_requests) is expected_proxy


@pytest.mark.parametrize("runtime", ["reference", "candidate"])
def test_no_proxy_environment_precedence_is_route_observable(runtime):
    """Lowercase proxy and NO_PROXY variables take precedence in both runtimes."""
    with local_http_server() as (backend_host, backend_port):
        with local_proxy_server(backend=(backend_host, backend_port)) as first:
            with local_proxy_server(backend=(backend_host, backend_port)) as second:
                env = {
                    "http_proxy": f"http://{first[0]}:{first[1]}",
                    "HTTP_PROXY": f"http://{second[0]}:{second[1]}",
                    "no_proxy": "localhost",
                    "NO_PROXY": "",
                }
                target = f"http://localhost:{backend_port}/health"
                with mock.patch.dict(os.environ, env, clear=True):
                    response = _request(runtime, target)
                assert response.status_code == 200
                assert not first[2].recorded_requests
                assert not second[2].recorded_requests


@pytest.mark.parametrize("runtime", ["reference", "candidate"])
def test_no_proxy_without_applicable_proxy_variable_routes_directly(runtime):
    with local_http_server() as (backend_host, backend_port):
        with local_proxy_server(backend=(backend_host, backend_port)) as (
            proxy_host,
            proxy_port,
            handler,
        ):
            env = {"HTTPS_PROXY": f"http://{proxy_host}:{proxy_port}"}
            with mock.patch.dict(os.environ, env, clear=True):
                response = _request(
                    runtime, f"http://{backend_host}:{backend_port}/health"
                )
            assert response.status_code == 200
            assert not handler.recorded_requests
