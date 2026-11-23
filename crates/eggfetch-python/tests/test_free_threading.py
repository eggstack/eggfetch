"""Free-threaded (PEP 703) runtime behaviour tests.

This module runs on every interpreter. Assertions that describe
free-threaded-only behaviour are guarded by _free_threaded_build(), so the
file stays meaningful on a GIL-enabled build.

Coverage:

* Importing eggfetch must not re-enable the GIL on a free-threaded
  interpreter. The module declares Py_MOD_GIL_NOT_USED, so a clean process
  that imports it keeps the interpreter free-threaded state.
* Concurrent requests through one Client must not corrupt shared Rust-side
  state: every request must get back its own response.
* A user trace callback may be entered concurrently on a free-threaded
  build. This is the documented contract (see
  plans/python-free-threaded-support-investigation.md, finding F3): the
  binding does not serialize user callbacks, so shared user state must
  carry its own synchronization. This test pins that contract; if the
  binding later adds explicit serialization, this is the place to update.
* A request-body iterator shared by concurrent requests must neither lose
  nor duplicate chunks.
"""

from __future__ import annotations

import http.server
import re
import subprocess
import sys
import sysconfig
import threading

import pytest

import eggfetch

from conftest import _ThreadingHTTPServer


# ---------------------------------------------------------------------------
# Interpreter probes
# ---------------------------------------------------------------------------


def _free_threaded_build() -> bool:
    """True when this interpreter was built with Py_GIL_DISABLED."""
    return sysconfig.get_config_var("Py_GIL_DISABLED") == 1


def _gil_enabled():
    """sys._is_gil_enabled() when the interpreter provides it."""
    probe = getattr(sys, "_is_gil_enabled", None)
    if probe is None:
        return None
    return bool(probe())


# ---------------------------------------------------------------------------
# Local echo server
# ---------------------------------------------------------------------------


def _read_chunked_body(rfile) -> bytes:
    """Read a chunked request body off the wire."""
    body = bytearray()
    while True:
        line = rfile.readline()
        if not line:
            break
        size_text = line.split(b";", 1)[0].strip()
        if not size_text:
            break
        size = int(size_text, 16)
        if size == 0:
            rfile.readline()  # CRLF after the terminal chunk
            break
        body += rfile.read(size)
        rfile.readline()  # CRLF that terminates the chunk
    return bytes(body)


class _EchoHandler(http.server.BaseHTTPRequestHandler):
    """Echoes the request path (GET) or body (POST) back to the client."""

    def do_GET(self):
        self._respond(self.path.encode())

    def do_POST(self):
        self._respond(self._read_body())

    def _read_body(self) -> bytes:
        encoding = self.headers.get("Transfer-Encoding", "").lower()
        if encoding == "chunked":
            return _read_chunked_body(self.rfile)
        length = int(self.headers.get("Content-Length", 0) or 0)
        return self.rfile.read(length) if length else b""

    def _respond(self, body: bytes) -> None:
        self.send_response(200)
        self.send_header("Content-Type", "application/octet-stream")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, format, *args):
        pass  # suppress noisy output


@pytest.fixture(scope="module")
def echo_server():
    srv = _ThreadingHTTPServer(("127.0.0.1", 0), _EchoHandler)
    port = srv.server_address[1]
    thread = threading.Thread(target=srv.serve_forever, daemon=True)
    thread.start()
    yield f"http://127.0.0.1:{port}"
    srv.shutdown()
    srv.server_close()


# ---------------------------------------------------------------------------
# GIL state
# ---------------------------------------------------------------------------


def test_import_does_not_reenable_the_gil():
    """Importing eggfetch must leave the interpreter GIL state unchanged.

    The probe runs in a clean subprocess: the test process itself may have
    loaded other extensions that request the GIL, which would make an
    in-process reading meaningless.
    """
    probe = (
        "import sys, sysconfig;"
        "probe = getattr(sys, '_is_gil_enabled', None);"
        "before = probe() if probe else None;"
        "import eggfetch, eggfetch._native;"
        "after = probe() if probe else None;"
        "print(sysconfig.get_config_var('Py_GIL_DISABLED'), before, after)"
    )
    result = subprocess.run(
        [sys.executable, "-c", probe],
        capture_output=True,
        text=True,
        check=True,
    )
    parts = result.stdout.split()
    assert len(parts) == 3, result.stdout
    disabled, before, after = parts
    # Importing the binding must not flip the interpreter GIL state.
    assert after == before
    if disabled == "1":
        # Free-threaded build: the GIL must stay disabled across the import.
        assert before == "False"
        assert after == "False"


# ---------------------------------------------------------------------------
# Concurrency through one client
# ---------------------------------------------------------------------------


def test_concurrent_requests_through_one_client(echo_server):
    """Concurrent requests on one client must each receive their own response."""
    client = eggfetch.Client()
    results = []
    errors = []

    def worker(index: int) -> None:
        try:
            response = client.get(f"{echo_server}/item/{index}")
            results.append((index, response.status_code, response.text))
        except Exception as exc:  # pragma: no cover - re-raised by the assert
            errors.append(exc)

    threads = [threading.Thread(target=worker, args=(i,)) for i in range(8)]
    for thread in threads:
        thread.start()
    for thread in threads:
        thread.join(timeout=60)

    assert not any(thread.is_alive() for thread in threads), "request threads hung"
    assert not errors, errors
    assert len(results) == 8
    for index, status, text in results:
        assert status == 200
        assert text == f"/item/{index}"


def test_trace_callback_is_entered_concurrently_on_free_threaded_builds(echo_server):
    """A trace callback may be entered by two requests at the same time.

    The callback blocks until a second invocation is in flight. On a
    GIL-enabled build the second invocation cannot start, so the test is
    skipped there; on a free-threaded build the wait proves the binding
    does not serialize user callbacks.
    """
    if not _free_threaded_build():
        pytest.skip("callback concurrency is only observable on a free-threaded build")

    client = eggfetch.Client()
    lock = threading.Lock()
    both_in_flight = threading.Event()
    state = {"active": 0, "entries": 0}
    threads_seen = set()

    def trace(name, info):
        with lock:
            state["active"] += 1
            state["entries"] += 1
            threads_seen.add(threading.get_ident())
            if state["active"] >= 2:
                both_in_flight.set()
        # Wait for a second concurrent invocation. A timeout here means the
        # callbacks were serialized, which would deadlock a user callback
        # that waits on its own shared state.
        both_in_flight.wait(timeout=15)
        with lock:
            state["active"] -= 1

    def worker() -> None:
        client.get(f"{echo_server}/traced", extensions={"trace": trace})

    threads = [threading.Thread(target=worker) for _ in range(2)]
    for thread in threads:
        thread.start()
    for thread in threads:
        thread.join(timeout=60)

    assert not any(thread.is_alive() for thread in threads), "request threads hung"
    assert both_in_flight.is_set(), "trace callbacks were serialized"
    assert len(threads_seen) >= 2, "callbacks did not run on distinct threads"


def test_shared_request_body_iterator_neither_loses_nor_duplicates_chunks(echo_server):
    """A body iterator shared by concurrent requests must not lose chunks.

    Each chunk leaves the shared iterator exactly once, so the union of the
    request bodies must contain every chunk exactly once, whatever the split
    between the two requests.
    """
    chunks = [f"chunk-{index};".encode() for index in range(8)]

    class _SharedIterator:
        """Thread-safe iterator shared by two concurrent requests."""

        def __init__(self, items):
            self._items = list(items)
            self._lock = threading.Lock()
            self._index = 0

        def __iter__(self):
            return self

        def __next__(self):
            with self._lock:
                if self._index >= len(self._items):
                    raise StopIteration
                item = self._items[self._index]
                self._index += 1
                return item

    client = eggfetch.Client()
    shared = _SharedIterator(chunks)
    bodies = []
    errors = []

    def worker() -> None:
        try:
            response = client.request("POST", f"{echo_server}/upload", content=shared)
            bodies.append(response.text)
        except Exception as exc:  # pragma: no cover - re-raised by the assert
            errors.append(exc)

    threads = [threading.Thread(target=worker) for _ in range(2)]
    for thread in threads:
        thread.start()
    for thread in threads:
        thread.join(timeout=60)

    assert not any(thread.is_alive() for thread in threads), "request threads hung"
    assert not errors, errors
    assert len(bodies) == 2

    received = re.findall(rb"chunk-\d+;", "".join(bodies).encode())
    assert sorted(received) == sorted(chunks)
