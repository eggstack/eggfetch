"""Tests for URL-pattern mount routing."""

from __future__ import annotations

import pytest

from eggfetch.compat.httpx import (
    Client,
    AsyncClient,
    MockTransport,
    Request,
    Response,
)
from eggfetch.compat.httpx._client import _match_mount, _parse_mount_pattern


def _make_handler(response_text: str):
    def handler(request):
        return Response(200, content=response_text.encode())

    return handler


class TestParseMountPattern:
    def test_all_catchall(self):
        assert _parse_mount_pattern("all://") == ("", None, None, "")

    def test_http_scheme_only(self):
        assert _parse_mount_pattern("http://") == ("http", None, None, "")

    def test_https_scheme_only(self):
        assert _parse_mount_pattern("https://") == ("https", None, None, "")

    def test_scheme_and_host(self):
        assert _parse_mount_pattern("http://example.com") == (
            "http", "example.com", None, "",
        )

    def test_scheme_host_port(self):
        assert _parse_mount_pattern("http://example.com:8080") == (
            "http", "example.com", 8080, "",
        )

    def test_scheme_host_path(self):
        assert _parse_mount_pattern("http://example.com/api") == (
            "http", "example.com", None, "/api",
        )

    def test_full_pattern(self):
        assert _parse_mount_pattern("https://example.com:443/api/v1") == (
            "https", "example.com", 443, "/api/v1",
        )


class TestMountRouting:
    def test_exact_scheme_match(self):
        http_handler = _make_handler("http")
        https_handler = _make_handler("https")

        with Client(
            mounts={
                "http://": MockTransport(http_handler),
                "https://": MockTransport(https_handler),
            }
        ) as client:
            resp = client.get("http://example.com/")
            assert resp.content == b"http"

    def test_longer_prefix_wins(self):
        general = _make_handler("general")
        specific = _make_handler("specific")

        with Client(
            mounts={
                "http://": MockTransport(general),
                "http://specific.example.com": MockTransport(specific),
            }
        ) as client:
            resp = client.get("http://specific.example.com/path")
            assert resp.content == b"specific"

    def test_no_match_falls_through(self):
        mock_resp = Response(200, content=b"default")

        def handler(request):
            return mock_resp

        with Client(transport=MockTransport(handler)) as client:
            resp = client.get("http://example.com/")
            assert resp.status_code == 200

    def test_mount_close_on_client_close(self):
        closed = []

        class TrackingTransport:
            def handle_request(self, request):
                return Response(200)

            def close(self):
                closed.append(True)

        client = Client(mounts={"http://": TrackingTransport()})
        client.close()
        assert len(closed) == 1


class TestComponentBasedMountRouting:
    """Tests for the component-based mount matching."""

    def test_all_catchall_matches_everything(self):
        with Client(
            mounts={"all://": MockTransport(_make_handler("catchall"))}
        ) as client:
            resp = client.get("http://example.com/")
            assert resp.content == b"catchall"

    def test_host_specific_does_not_match_different_host(self):
        specific = _make_handler("specific")
        default = _make_handler("default")

        with Client(
            mounts={
                "http://specific.com": MockTransport(specific),
                "all://": MockTransport(default),
            }
        ) as client:
            resp = client.get("http://other.com/")
            assert resp.content == b"default"

    def test_port_specific_match(self):
        port8080 = _make_handler("port8080")
        port9090 = _make_handler("port9090")

        with Client(
            mounts={
                "http://example.com:8080": MockTransport(port8080),
                "http://example.com:9090": MockTransport(port9090),
            }
        ) as client:
            resp = client.get("http://example.com:8080/")
            assert resp.content == b"port8080"

    def test_port_specific_no_match(self):
        port8080 = _make_handler("port8080")
        default = _make_handler("default")

        with Client(
            mounts={
                "http://example.com:8080": MockTransport(port8080),
                "all://": MockTransport(default),
            }
        ) as client:
            resp = client.get("http://example.com:9090/")
            assert resp.content == b"default"

    def test_path_prefix_match(self):
        api = _make_handler("api")
        default = _make_handler("default")

        with Client(
            mounts={
                "http://example.com/api": MockTransport(api),
                "all://": MockTransport(default),
            }
        ) as client:
            resp = client.get("http://example.com/api/users")
            assert resp.content == b"api"

    def test_path_prefix_no_match_without_prefix(self):
        api = _make_handler("api")
        default = _make_handler("default")

        with Client(
            mounts={
                "http://example.com/api": MockTransport(api),
                "all://": MockTransport(default),
            }
        ) as client:
            resp = client.get("http://example.com/other")
            assert resp.content == b"default"

    def test_host_beats_scheme_only(self):
        host_handler = _make_handler("host")
        scheme_handler = _make_handler("scheme")

        with Client(
            mounts={
                "http://": MockTransport(scheme_handler),
                "http://example.com": MockTransport(host_handler),
            }
        ) as client:
            resp = client.get("http://example.com/")
            assert resp.content == b"host"

    def test_host_with_path_beats_host_only(self):
        host_handler = _make_handler("host")
        path_handler = _make_handler("path")

        with Client(
            mounts={
                "http://example.com": MockTransport(host_handler),
                "http://example.com/api": MockTransport(path_handler),
            }
        ) as client:
            resp = client.get("http://example.com/api/endpoint")
            assert resp.content == b"path"

    def test_no_mount_returns_none(self):
        result = _match_mount("http://example.com/", {})
        assert result is None

    def test_scheme_mismatch_skips(self):
        https_handler = _make_handler("https")
        default = _make_handler("default")

        with Client(
            mounts={
                "https://": MockTransport(https_handler),
                "all://": MockTransport(default),
            }
        ) as client:
            resp = client.get("http://example.com/")
            assert resp.content == b"default"

    def test_host_port_beats_host_path(self):
        """host+port (score 205) beats host+path (score 200+len)."""
        port_handler = _make_handler("port")
        path_handler = _make_handler("path")

        with Client(
            mounts={
                "http://example.com:8080": MockTransport(port_handler),
                "http://example.com/api": MockTransport(path_handler),
            }
        ) as client:
            resp = client.get("http://example.com:8080/api/endpoint")
            assert resp.content == b"port"

    def test_full_url_beats_all(self):
        """Full URL pattern (score 10000) beats catch-all."""
        full_handler = _make_handler("full")
        catchall_handler = _make_handler("catchall")

        with Client(
            mounts={
                "http://example.com:8080/api": MockTransport(full_handler),
                "all://": MockTransport(catchall_handler),
            }
        ) as client:
            resp = client.get("http://example.com:8080/api/v1")
            assert resp.content == b"full"

    def test_no_explicit_port_matches_default(self):
        """URL without explicit port matches mount without port."""
        port_handler = _make_handler("port")
        default_handler = _make_handler("default")

        with Client(
            mounts={
                "http://example.com:8080": MockTransport(port_handler),
                "all://": MockTransport(default_handler),
            }
        ) as client:
            resp = client.get("http://example.com/")
            assert resp.content == b"default"

    def test_base_url_plus_mount(self):
        """Mount matching uses the resolved URL (after base_url merge)."""
        api_handler = _make_handler("api")
        default_handler = _make_handler("default")

        with Client(
            base_url="http://example.com",
            mounts={
                "http://example.com/api": MockTransport(api_handler),
                "all://": MockTransport(default_handler),
            },
        ) as client:
            resp = client.get("/api/users")
            assert resp.content == b"api"


class TestMountPriorityEdgeCases:
    """Edge cases for mount routing priority."""

    def test_custom_scheme_mount(self):
        """Custom (non-http/https) scheme mounts work."""
        ftp_handler = _make_handler("ftp")

        with Client(
            mounts={"ftp://": MockTransport(ftp_handler)}
        ) as client:
            resp = client.get("ftp://files.example.com/data")
            assert resp.content == b"ftp"

    def test_scheme_only_http_does_not_match_https(self):
        """http:// mount must not match https:// URLs."""
        http_handler = _make_handler("http")
        https_handler = _make_handler("https")
        default_handler = _make_handler("default")

        with Client(
            mounts={
                "http://": MockTransport(http_handler),
                "https://": MockTransport(https_handler),
                "all://": MockTransport(default_handler),
            }
        ) as client:
            resp = client.get("https://example.com/")
            assert resp.content == b"https"

    def test_longer_path_wins_over_shorter(self):
        """More specific path prefix beats shorter one."""
        short_handler = _make_handler("short")
        long_handler = _make_handler("long")

        with Client(
            mounts={
                "http://example.com/api": MockTransport(short_handler),
                "http://example.com/api/v2": MockTransport(long_handler),
            }
        ) as client:
            resp = client.get("http://example.com/api/v2/resource")
            assert resp.content == b"long"

    def test_catchall_always_lowest_priority(self):
        """Catch-all always loses to any more-specific mount."""
        catchall = _make_handler("catchall")
        scheme = _make_handler("scheme")
        host = _make_handler("host")

        with Client(
            mounts={
                "all://": MockTransport(catchall),
                "http://": MockTransport(scheme),
                "http://example.com": MockTransport(host),
            }
        ) as client:
            resp = client.get("http://example.com/")
            assert resp.content == b"host"

    def test_mount_none_transport_falls_through(self):
        """Passing None as transport value falls through to default transport."""
        def handler(request):
            return Response(200, content=b"default")

        with Client(
            transport=MockTransport(handler),
            mounts={"http://none.example.com": None},
        ) as client:
            resp = client.get("http://none.example.com/")
            assert resp.content == b"default"

    def test_empty_mounts_dict(self):
        """Empty mounts dict falls through to default transport."""
        def handler(request):
            return Response(200, content=b"default")

        with Client(transport=MockTransport(handler), mounts={}) as client:
            resp = client.get("http://example.com/")
            assert resp.content == b"default"

    def test_host_case_insensitive(self):
        """Mount matching is case-insensitive for hosts."""
        upper_handler = _make_handler("upper")

        with Client(
            mounts={
                "http://Example.Com": MockTransport(upper_handler),
            }
        ) as client:
            resp = client.get("http://example.com/")
            assert resp.content == b"upper"

    def test_path_exact_match(self):
        """Exact path match (no trailing content) works."""
        handler = _make_handler("exact")

        with Client(
            mounts={"http://example.com/api": MockTransport(handler)}
        ) as client:
            resp = client.get("http://example.com/api")
            assert resp.content == b"exact"

    def test_mount_with_query_string_ignored(self):
        """Query strings don't affect mount matching."""
        handler = _make_handler("matched")

        with Client(
            mounts={"http://example.com/api": MockTransport(handler)}
        ) as client:
            resp = client.get("http://example.com/api?key=value")
            assert resp.content == b"matched"


class TestMountPriorityAsync:
    """Async mount priority edge cases."""

    @pytest.mark.asyncio
    async def test_async_host_port_beats_host_path(self):
        async def port_handler(request):
            return Response(200, content=b"port")

        async def path_handler(request):
            return Response(200, content=b"path")

        async with AsyncClient(
            mounts={
                "http://example.com:8080": MockTransport(port_handler),
                "http://example.com/api": MockTransport(path_handler),
            }
        ) as client:
            resp = await client.get("http://example.com:8080/api/endpoint")
            assert resp.content == b"port"

    @pytest.mark.asyncio
    async def test_async_custom_scheme(self):
        async def ftp_handler(request):
            return Response(200, content=b"ftp-async")

        async with AsyncClient(
            mounts={"ftp://": MockTransport(ftp_handler)}
        ) as client:
            resp = await client.get("ftp://files.example.com/")
            assert resp.content == b"ftp-async"


class TestAsyncMountRouting:
    @pytest.mark.asyncio
    async def test_async_mount_dispatch(self):
        async def handler(request):
            return Response(200, content=b"async-mount")

        async with AsyncClient(
            mounts={"http://": MockTransport(handler)}
        ) as client:
            resp = await client.get("http://example.com/")
            assert resp.content == b"async-mount"

    @pytest.mark.asyncio
    async def test_async_transport_constructor(self):
        async def handler(request):
            return Response(200, content=b"async-transport")

        async with AsyncClient(
            async_transport=MockTransport(handler)
        ) as client:
            resp = await client.get("http://example.com/")
            assert resp.content == b"async-transport"
