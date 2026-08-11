"""Reference/candidate differential evidence for HTTP and HTTPS proxies."""

from __future__ import annotations

import os
import ssl

import httpx
import pytest

from eggfetch.compat.httpx import Client

from .native_fixtures import (
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
            certificate = None
            if origin_tls:
                certificate = (
                    origin_cert,
                    os.path.join(os.path.dirname(origin_cert), "key.pem"),
                )
            proxy_context = local_tls_proxy_server(
                backend=None if origin_tls else (origin_host, origin_port),
                certificate=certificate,
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
            verify = origin_cert if origin_tls else (proxy[3] if proxy_tls else False)

            if runtime == "reference":
                if proxy_tls:
                    proxy_ssl_context = ssl.create_default_context(cafile=proxy[3])
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
                with Client(
                    proxy=proxy_url,
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
