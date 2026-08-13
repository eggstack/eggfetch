"""Executable HTTPX 0.28.1 timeout reference evidence for corrective closure."""

from __future__ import annotations

import time
import http.server

import httpx
import pytest

from eggfetch.compat.httpx import AsyncClient, Timeout

from .native_fixtures import local_http_server


class _IndependentPhaseHandler(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def do_GET(self):
        if self.path != "/independent-phases":
            self.send_error(404)
            return
        self.send_response(200)
        self.send_header("Content-Type", "text/plain")
        self.send_header("Transfer-Encoding", "chunked")
        self.end_headers()
        time.sleep(0.25)
        self.wfile.write(b"1\r\na\r\n")
        self.wfile.flush()
        time.sleep(0.25)
        self.wfile.write(b"1\r\nb\r\n0\r\n\r\n")
        self.wfile.flush()

    def log_message(self, format, *args):
        pass


def test_reference_timeout_shape_has_four_operational_dimensions():
    reference = httpx.Timeout(5.0, connect=1.0)
    assert reference.connect == 1.0
    assert reference.read == 5.0
    assert reference.write == 5.0
    assert reference.pool == 5.0
    assert not hasattr(reference, "total")

    candidate = Timeout(5.0, connect=1.0)
    assert candidate.connect == 1.0
    assert candidate.read == 5.0
    assert candidate.write == 5.0
    assert candidate.pool == 5.0


@pytest.mark.parametrize(
    ("kwargs", "expected"),
    [
        ({}, (5.0, 5.0, 5.0, 5.0)),
        ({"connect": None}, (None, 5.0, 5.0, 5.0)),
        ({"read": None}, (5.0, None, 5.0, 5.0)),
        ({"write": None}, (5.0, 5.0, None, 5.0)),
        ({"pool": None}, (5.0, 5.0, 5.0, None)),
    ],
)
def test_timeout_omitted_vs_explicit_none_matches_reference(kwargs, expected):
    reference = httpx.Timeout(5.0, **kwargs)
    candidate = Timeout(5.0, **kwargs)
    assert (reference.connect, reference.read, reference.write, reference.pool) == expected
    assert (candidate.connect, candidate.read, candidate.write, candidate.pool) == expected


def test_timeout_without_default_requires_all_phases_like_reference():
    with pytest.raises(ValueError):
        httpx.Timeout()
    with pytest.raises(ValueError):
        Timeout()

    reference = httpx.Timeout(None, connect=1.0)
    candidate = Timeout(None, connect=1.0)
    assert candidate.as_dict == {
        "connect": reference.connect,
        "read": reference.read,
        "write": reference.write,
        "pool": reference.pool,
    }


@pytest.mark.asyncio
@pytest.mark.parametrize("runtime", ["reference", "candidate"])
async def test_independent_read_phases_do_not_form_synthetic_total(runtime):
    """Two sub-scalar read waits succeed because HTTPX has no total timeout."""
    with local_http_server(_IndependentPhaseHandler) as (host, port):
        target = f"http://{host}:{port}/independent-phases"
        if runtime == "reference":
            async with httpx.AsyncClient(timeout=0.4, trust_env=False) as client:
                response = await client.get(target)
        else:
            async with AsyncClient(timeout=0.4, trust_env=False) as client:
                response = await client.get(target)
        assert response.status_code == 200
        assert response.text == "ab"
