"""Tests for the eggfetch Python proxy configuration API."""

import pytest

import eggfetch


# ---------------------------------------------------------------------------
# Proxy parameter accepted by Client constructor
# ---------------------------------------------------------------------------


class TestClientProxyParam:
    def test_client_constructor_accepts_proxy_none(self):
        with eggfetch.Client(proxy=None) as client:
            assert not client.is_closed

    def test_client_constructor_accepts_proxy_string(self):
        with eggfetch.Client(proxy="http://proxy.example:8080") as client:
            assert not client.is_closed

    def test_client_constructor_accepts_proxy_false(self):
        with eggfetch.Client(proxy=False) as client:
            assert not client.is_closed

    def test_client_constructor_rejects_proxy_true(self):
        with pytest.raises(TypeError, match="True is not valid"):
            eggfetch.Client(proxy=True)

    def test_client_constructor_rejects_proxy_int(self):
        with pytest.raises(TypeError, match="proxy must be a URL string, False, or None"):
            eggfetch.Client(proxy=123)

    def test_client_constructor_rejects_proxy_list(self):
        with pytest.raises(TypeError, match="proxy must be a URL string, False, or None"):
            eggfetch.Client(proxy=["http://proxy.example:8080"])


# ---------------------------------------------------------------------------
# Proxy parameter accepted by request methods
# ---------------------------------------------------------------------------


class TestRequestMethodProxyParam:
    def test_client_request_proxy_none(self):
        with eggfetch.Client() as client:
            with pytest.raises(eggfetch.RequestError):
                client.request("GET", "http://127.0.0.1:1", proxy=None)

    def test_client_request_proxy_string(self):
        with eggfetch.Client() as client:
            with pytest.raises(eggfetch.RequestError):
                client.request("GET", "http://127.0.0.1:1", proxy="http://proxy.example:8080")

    def test_client_request_proxy_false(self):
        with eggfetch.Client() as client:
            with pytest.raises(eggfetch.RequestError):
                client.request("GET", "http://127.0.0.1:1", proxy=False)

    def test_client_request_proxy_true_raises(self):
        with eggfetch.Client() as client:
            with pytest.raises(TypeError, match="True is not valid"):
                client.request("GET", "http://127.0.0.1:1", proxy=True)

    def test_client_get_proxy_none(self):
        with eggfetch.Client() as client:
            with pytest.raises(eggfetch.RequestError):
                client.get("http://127.0.0.1:1", proxy=None)

    def test_client_get_proxy_string(self):
        with eggfetch.Client() as client:
            with pytest.raises(eggfetch.RequestError):
                client.get("http://127.0.0.1:1", proxy="http://proxy.example:8080")

    def test_client_get_proxy_false(self):
        with eggfetch.Client() as client:
            with pytest.raises(eggfetch.RequestError):
                client.get("http://127.0.0.1:1", proxy=False)

    def test_client_post_proxy_string(self):
        with eggfetch.Client() as client:
            with pytest.raises(eggfetch.RequestError):
                client.post("http://127.0.0.1:1", proxy="http://proxy.example:8080")

    def test_client_put_proxy_string(self):
        with eggfetch.Client() as client:
            with pytest.raises(eggfetch.RequestError):
                client.put("http://127.0.0.1:1", proxy="http://proxy.example:8080")

    def test_client_patch_proxy_string(self):
        with eggfetch.Client() as client:
            with pytest.raises(eggfetch.RequestError):
                client.patch("http://127.0.0.1:1", proxy="http://proxy.example:8080")

    def test_client_delete_proxy_string(self):
        with eggfetch.Client() as client:
            with pytest.raises(eggfetch.RequestError):
                client.delete("http://127.0.0.1:1", proxy="http://proxy.example:8080")

    def test_client_head_proxy_string(self):
        with eggfetch.Client() as client:
            with pytest.raises(eggfetch.RequestError):
                client.head("http://127.0.0.1:1", proxy="http://proxy.example:8080")

    def test_client_options_proxy_string(self):
        with eggfetch.Client() as client:
            with pytest.raises(eggfetch.RequestError):
                client.options("http://127.0.0.1:1", proxy="http://proxy.example:8080")


# ---------------------------------------------------------------------------
# Proxy parameter accepted by module-level functions
# ---------------------------------------------------------------------------


class TestModuleLevelProxyParam:
    def test_get_proxy_none(self):
        with pytest.raises(eggfetch.RequestError):
            eggfetch.get("http://127.0.0.1:1", proxy=None)

    def test_get_proxy_string(self):
        with pytest.raises(eggfetch.RequestError):
            eggfetch.get("http://127.0.0.1:1", proxy="http://proxy.example:8080")

    def test_get_proxy_false(self):
        with pytest.raises(eggfetch.RequestError):
            eggfetch.get("http://127.0.0.1:1", proxy=False)

    def test_get_proxy_true_raises(self):
        with pytest.raises(TypeError, match="True is not valid"):
            eggfetch.get("http://127.0.0.1:1", proxy=True)

    def test_post_proxy_string(self):
        with pytest.raises(eggfetch.RequestError):
            eggfetch.post("http://127.0.0.1:1", proxy="http://proxy.example:8080")

    def test_put_proxy_string(self):
        with pytest.raises(eggfetch.RequestError):
            eggfetch.put("http://127.0.0.1:1", proxy="http://proxy.example:8080")

    def test_patch_proxy_string(self):
        with pytest.raises(eggfetch.RequestError):
            eggfetch.patch("http://127.0.0.1:1", proxy="http://proxy.example:8080")

    def test_delete_proxy_string(self):
        with pytest.raises(eggfetch.RequestError):
            eggfetch.delete("http://127.0.0.1:1", proxy="http://proxy.example:8080")

    def test_head_proxy_string(self):
        with pytest.raises(eggfetch.RequestError):
            eggfetch.head("http://127.0.0.1:1", proxy="http://proxy.example:8080")

    def test_options_proxy_string(self):
        with pytest.raises(eggfetch.RequestError):
            eggfetch.options("http://127.0.0.1:1", proxy="http://proxy.example:8080")

    def test_request_proxy_string(self):
        with pytest.raises(eggfetch.RequestError):
            eggfetch.request("GET", "http://127.0.0.1:1", proxy="http://proxy.example:8080")


# ---------------------------------------------------------------------------
# Invalid proxy URL raises appropriate error
# ---------------------------------------------------------------------------


class TestInvalidProxyUrl:
    def test_invalid_proxy_url_scheme(self):
        with pytest.raises(eggfetch.ProxyError, match="not supported|unsupported|invalid"):
            eggfetch.get("http://example.com", proxy="ftp://proxy.example:8080")

    def test_invalid_proxy_url_no_host(self):
        with pytest.raises(eggfetch.ProxyError, match="must have a host|invalid"):
            eggfetch.get("http://example.com", proxy="http://")

    def test_invalid_proxy_url_with_credentials(self):
        with pytest.raises(eggfetch.ProxyError, match="credentials|invalid"):
            eggfetch.get(
                "http://example.com",
                proxy="http://user:pass@proxy.example:8080",
            )

    def test_invalid_proxy_url_with_fragment(self):
        with pytest.raises(eggfetch.ProxyError, match="fragment|invalid"):
            eggfetch.get(
                "http://example.com",
                proxy="http://proxy.example:8080#frag",
            )

    def test_invalid_proxy_url_with_query(self):
        with pytest.raises(eggfetch.ProxyError, match="query|invalid"):
            eggfetch.get(
                "http://example.com",
                proxy="http://proxy.example:8080?key=value",
            )

    def test_invalid_proxy_url_on_client(self):
        with pytest.raises(eggfetch.ProxyError):
            eggfetch.Client(proxy="not-a-url")

    def test_invalid_proxy_url_on_client_request(self):
        with eggfetch.Client() as client:
            with pytest.raises(eggfetch.ProxyError):
                client.get("http://example.com", proxy="not-a-url")


# ---------------------------------------------------------------------------
# proxy=False disables proxy
# ---------------------------------------------------------------------------


class TestProxyDisable:
    def test_false_disables_proxy_on_client(self):
        with eggfetch.Client(proxy=False) as client:
            assert not client.is_closed

    def test_false_disables_proxy_on_request(self):
        with eggfetch.Client() as client:
            with pytest.raises(eggfetch.RequestError):
                client.get("http://127.0.0.1:1", proxy=False)

    def test_false_disables_proxy_on_module_level(self):
        with pytest.raises(eggfetch.RequestError):
            eggfetch.get("http://127.0.0.1:1", proxy=False)

    def test_request_false_overrides_client_proxy(self):
        with eggfetch.Client(proxy="http://proxy.example:8080") as client:
            with pytest.raises(eggfetch.RequestError):
                client.get("http://127.0.0.1:1", proxy=False)


# ---------------------------------------------------------------------------
# Proxy exceptions hierarchy
# ---------------------------------------------------------------------------


class TestProxyExceptionHierarchy:
    def test_proxy_error_is_request_error(self):
        assert issubclass(eggfetch.ProxyError, eggfetch.RequestError)

    def test_proxy_connect_error_is_proxy_error(self):
        assert issubclass(eggfetch.ProxyConnectError, eggfetch.ProxyError)

    def test_proxy_auth_error_is_proxy_error(self):
        assert issubclass(eggfetch.ProxyAuthError, eggfetch.ProxyError)

    def test_proxy_connect_error_is_request_error(self):
        assert issubclass(eggfetch.ProxyConnectError, eggfetch.RequestError)

    def test_proxy_auth_error_is_request_error(self):
        assert issubclass(eggfetch.ProxyAuthError, eggfetch.RequestError)


# ---------------------------------------------------------------------------
# Proxy type validation
# ---------------------------------------------------------------------------


class TestProxyTypeValidation:
    def test_proxy_int_type_error(self):
        with pytest.raises(TypeError, match="proxy must be a URL string, False, or None"):
            eggfetch.get("http://example.com", proxy=42)

    def test_proxy_float_type_error(self):
        with pytest.raises(TypeError, match="proxy must be a URL string, False, or None"):
            eggfetch.get("http://example.com", proxy=3.14)

    def test_proxy_dict_type_error(self):
        with pytest.raises(TypeError, match="proxy must be a URL string, False, or None"):
            eggfetch.get("http://example.com", proxy={"http": "proxy.example:8080"})

    def test_proxy_bytes_type_error(self):
        with pytest.raises(TypeError, match="proxy must be a URL string, False, or None"):
            eggfetch.get("http://example.com", proxy=b"http://proxy.example:8080")

    def test_proxy_true_type_error(self):
        with pytest.raises(TypeError, match="True is not valid"):
            eggfetch.get("http://example.com", proxy=True)

    def test_proxy_invalid_string_raises_proxy_error(self):
        with pytest.raises(eggfetch.ProxyError):
            eggfetch.get("http://example.com", proxy="not a url")

    def test_client_proxy_int_type_error(self):
        with pytest.raises(TypeError, match="proxy must be a URL string, False, or None"):
            eggfetch.Client(proxy=42)

    def test_client_request_proxy_int_type_error(self):
        with eggfetch.Client() as client:
            with pytest.raises(TypeError, match="proxy must be a URL string, False, or None"):
                client.get("http://example.com", proxy=42)
