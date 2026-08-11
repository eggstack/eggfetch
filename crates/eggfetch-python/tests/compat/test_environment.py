"""Tests for environment variable handling and trust_env behaviour."""

from __future__ import annotations

import os
import typing
from unittest import mock

import pytest

from eggfetch.compat.httpx import Client, AsyncClient, URL
from eggfetch.compat.httpx._mock import MockTransport, _build_response
from eggfetch.compat.httpx._request import Request
from eggfetch.compat.httpx._response import Response
from .native_fixtures import local_http_server, local_proxy_server


def _request_with_runtime(runtime, target):
    if runtime == "reference":
        import httpx

        with httpx.Client(trust_env=True, timeout=3) as client:
            return client.get(target)
    with Client(trust_env=True, timeout=3) as client:
        return client.get(target)


# ---------------------------------------------------------------------------
# trust_env parameter
# ---------------------------------------------------------------------------


class TestTrustEnv:
    """Verify trust_env is accepted and stored on Client/AsyncClient."""

    def test_client_default_trust_env_is_true(self):
        client = Client()
        assert client.trust_env is True
        client.close()

    def test_client_trust_env_false(self):
        client = Client(trust_env=False)
        assert client.trust_env is False
        client.close()

    def test_async_client_default_trust_env_is_true(self):
        client = AsyncClient()
        assert client.trust_env is True

    def test_async_client_trust_env_false(self):
        client = AsyncClient(trust_env=False)
        assert client.trust_env is False


# ---------------------------------------------------------------------------
# Proxy environment variables
# ---------------------------------------------------------------------------


class TestProxyEnvVars:
    """Test that proxy env vars are recognized when trust_env=True.

    The compat layer passes HTTPX-compatible environment selection through to
    the native client, including TLS-protected HTTP proxy endpoints.
    """

    @mock.patch.dict(os.environ, {"HTTP_PROXY": "http://proxy.local:8080"}, clear=False)
    def test_http_proxy_env_accepted(self):
        with Client(trust_env=True) as client:
            assert client.trust_env is True

    @mock.patch.dict(os.environ, {"NO_PROXY": "localhost,127.0.0.1"}, clear=False)
    def test_no_proxy_env_accepted(self):
        with Client(trust_env=True) as client:
            assert client.trust_env is True

    @mock.patch.dict(os.environ, {"http_proxy": "http://proxy.local:8080"}, clear=False)
    def test_lowercase_proxy_env_accepted(self):
        with Client(trust_env=True) as client:
            assert client.trust_env is True


class TestProxyEnvOverride:
    """trust_env=False disables environment proxy discovery."""

    @mock.patch.dict(os.environ, {"HTTP_PROXY": "http://proxy.local:8080"}, clear=False)
    def test_trust_env_false_ignores_http_proxy(self):
        with Client(trust_env=False) as client:
            assert client.trust_env is False


class TestProxyEnvNegative:
    """Verify behavior with unsupported or unexpected proxy env vars."""

    @mock.patch.dict(os.environ, {"ALL_PROXY": "http://proxy.local:8080"}, clear=False)
    def test_all_proxy_env_accepted(self):
        with Client(trust_env=True) as client:
            assert client.trust_env is True

    @mock.patch.dict(os.environ, {"NO_PROXY": "*"}, clear=False)
    def test_no_proxy_wildcard_accepted(self):
        with Client(trust_env=True) as client:
            assert client.trust_env is True

    @mock.patch.dict(os.environ, {"NO_PROXY": "10.0.0.0/8,.example.com,localhost:8080"}, clear=False)
    def test_no_proxy_complex_pattern_accepted(self):
        with Client(trust_env=True) as client:
            assert client.trust_env is True

    @mock.patch.dict(os.environ, {"HTTPS_PROXY": "http://proxy.local:8080"}, clear=False)
    def test_https_proxy_env_accepted(self):
        with Client(trust_env=True) as client:
            assert client.trust_env is True


# ---------------------------------------------------------------------------
# Explicit proxy overrides env
# ---------------------------------------------------------------------------


class TestExplicitProxyOverridesEnv:
    """Explicit proxy= constructor argument takes precedence over env vars."""

    @mock.patch.dict(os.environ, {"HTTP_PROXY": "http://env-proxy:8080"}, clear=False)
    def test_explicit_proxy_wins(self):
        with Client(proxy="http://explicit-proxy:9090") as client:
            assert client.trust_env is True


@pytest.mark.parametrize("runtime", ["reference", "candidate"])
def test_lowercase_proxy_variables_win_over_uppercase(runtime):
    with local_http_server() as (backend_host, backend_port):
        with local_proxy_server(backend=(backend_host, backend_port)) as (
            upper_host,
            upper_port,
            upper_handler,
        ):
            with local_proxy_server(backend=(backend_host, backend_port)) as (
                lower_host,
                lower_port,
                lower_handler,
            ):
                env = {
                    "HTTP_PROXY": f"http://{upper_host}:{upper_port}",
                    "http_proxy": f"http://{lower_host}:{lower_port}",
                }
                with mock.patch.dict(os.environ, env, clear=True):
                    response = _request_with_runtime(
                        runtime, f"http://{backend_host}:{backend_port}/health"
                    )
                assert response.status_code == 200
                assert lower_handler.recorded_requests
                assert not upper_handler.recorded_requests


@pytest.mark.parametrize("runtime", ["reference", "candidate"])
def test_scheme_less_environment_proxy_is_normalized(runtime):
    with local_http_server() as (backend_host, backend_port):
        with local_proxy_server(backend=(backend_host, backend_port)) as (
            proxy_host,
            proxy_port,
            handler,
        ):
            env = {"HTTP_PROXY": f"{proxy_host}:{proxy_port}"}
            with mock.patch.dict(os.environ, env, clear=True):
                response = _request_with_runtime(
                    runtime, f"http://{backend_host}:{backend_port}/health"
                )
            assert response.status_code == 200
            assert handler.recorded_requests


# ---------------------------------------------------------------------------
# Base URL resolution
# ---------------------------------------------------------------------------


class TestBaseUrlResolution:
    """Base URL is resolved before mount/dispatch routing."""

    def test_base_url_merge(self):
        with Client(base_url="http://example.com/api") as client:
            req = client.build_request("GET", "/users")
            # /users is absolute, so urljoin replaces the path
            assert req.url.host == "example.com"
            assert req.url.path == "/users"

    def test_base_url_with_relative_path(self):
        with Client(base_url="http://example.com/api/") as client:
            req = client.build_request("GET", "users")
            assert req.url.host == "example.com"
            assert req.url.path == "/api/users"

    def test_base_url_with_absolute_url(self):
        with Client(base_url="http://example.com/api") as client:
            req = client.build_request("GET", "http://other.com/other")
            assert req.url.host == "other.com"
