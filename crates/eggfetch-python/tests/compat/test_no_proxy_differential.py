"""Reference/candidate route-selection evidence for HTTPX NO_PROXY rules."""

from __future__ import annotations

import os
from unittest import mock

import httpx
import pytest

from eggfetch.compat.httpx import Client

from .native_fixtures import local_http_server, local_proxy_server


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
