"""Reference/candidate differential evidence for HTTP and HTTPS proxies."""

from __future__ import annotations

import os
import ssl

import httpx
import pytest

from eggfetch.compat.httpx import Client

from .native_fixtures import (
    _generate_ca_signed_server_cert,
    _generate_self_signed_cert,
    local_http_server,
    local_proxy_server,
    local_tls_proxy_server,
    local_tls_server,
)


@pytest.mark.parametrize("runtime", ["reference", "candidate"])
@pytest.mark.parametrize(
    ("origin_tls", "proxy_tls"),
    [(False, False), (True, False), (False, True), (True, True)],
)
def test_proxy_endpoint_matrix_matches_reference(runtime, origin_tls, proxy_tls):
    """All four origin/proxy scheme combinations use the same route shape."""
    origin_context = local_tls_server() if origin_tls else local_http_server()
    with origin_context as origin:
        origin_host, origin_port = origin[:2]
        origin_cert = origin[3] if origin_tls else None

        if proxy_tls:
            # The proxy uses its own CA-signed cert pair so the
            # client trust anchor (``ca_cert_path``) is enumerable
            # through ``ssl.SSLContext.get_ca_certs()``.  In the
            # True-True case the proxy is the origin of the
            # CONNECT path; we still need a CA-signed cert here
            # because the proxy's cert is a ``CA:FALSE`` end
            # entity cert that the Python ``ssl`` module would
            # otherwise hide from ``get_ca_certs``.
            import tempfile

            proxy_dir = tempfile.mkdtemp()
            (
                proxy_ca_path,
                _proxy_ca_key,
                proxy_server_cert,
                proxy_server_key,
            ) = _generate_ca_signed_server_cert(proxy_dir)
            proxy_context = local_tls_proxy_server(
                backend=None if origin_tls else (origin_host, origin_port),
                certificate=(proxy_server_cert, proxy_server_key),
            )
        else:
            proxy_context = local_proxy_server(
                backend=None if origin_tls else (origin_host, origin_port)
            )

        with proxy_context as proxy:
            proxy_host, proxy_port, handler = proxy[:3]
            target = (
                f"https://{origin_host}:{origin_port}/health"
                if origin_tls
                else f"http://{origin_host}:{origin_port}/health"
            )
            proxy_url = (
                f"https://{proxy_host}:{proxy_port}"
                if proxy_tls
                else f"http://{proxy_host}:{proxy_port}"
            )
            # ``local_tls_proxy_server`` yields
            # ``(server_cert_path, ca_cert_path_or_None)`` at index 3
            # when a CA-signed cert is in use.  ``local_proxy_server``
            # only yields three elements.  The CA cert is a
            # ``CA:TRUE`` anchor that the Python ``ssl`` module
            # exposes through ``get_ca_certs()``; using the server
            # cert as a trust anchor would be accepted by
            # ``load_verify_locations`` but hidden from
            # ``get_ca_certs``, which prevents eggfetch's
            # translation layer from extracting the actual DER
            # anchors.
            if proxy_tls:
                proxy_server_cert, proxy_ca_cert = proxy[3]
                # In True-True, the test supplies its own CA-signed
                # proxy cert pair; prefer that CA over the fixture
                # default when available.
                effective_proxy_ca = proxy_ca_path or proxy_ca_cert
            else:
                proxy_server_cert = None
                proxy_ca_cert = None
                effective_proxy_ca = None
            if origin_tls:
                verify = origin_cert
            elif proxy_tls:
                verify = effective_proxy_ca or proxy_server_cert
            else:
                verify = False

            if runtime == "reference":
                if proxy_tls:
                    proxy_ssl_context = ssl.create_default_context(
                        cafile=effective_proxy_ca or proxy_server_cert
                    )
                    proxy_arg = httpx.Proxy(proxy_url, ssl_context=proxy_ssl_context)
                    reference_verify = origin_cert if origin_tls else True
                else:
                    proxy_arg = proxy_url
                    reference_verify = origin_cert if origin_tls else False
                with httpx.Client(
                    proxy=proxy_arg,
                    trust_env=False,
                    timeout=5,
                    verify=reference_verify,
                ) as client:
                    response = client.get(target)
            else:
                # eggfetch: proxy endpoint TLS is governed by the
                # Proxy itself.  The proxy CA is supplied through
                # ``Proxy(ssl_context=...)`` so the origin ``verify=``
                # is not reused as a fallback for the proxy handshake.
                from eggfetch.compat.httpx import Proxy as _CompatProxy

                if proxy_tls:
                    proxy_ssl_context = ssl.create_default_context(
                        cafile=effective_proxy_ca or proxy_server_cert
                    )
                    candidate_proxy: object = _CompatProxy(
                        proxy_url, ssl_context=proxy_ssl_context
                    )
                else:
                    candidate_proxy = proxy_url
                with Client(
                    proxy=candidate_proxy,
                    trust_env=False,
                    timeout=5,
                    verify=verify,
                ) as client:
                    response = client.get(target)

            assert response.status_code == 200
            assert response.text == "ok"
            assert handler.recorded_requests

            observed = handler.recorded_requests[0]
            expected_method = "CONNECT" if origin_tls else "GET"
            assert observed["method"] == expected_method
            if origin_tls:
                assert observed["target"] == f"{origin_host}:{origin_port}"
            else:
                assert observed["target"].startswith("http://")
