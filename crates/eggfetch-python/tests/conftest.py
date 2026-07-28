"""Shared pytest fixtures and helpers for eggfetch test suites.

This conftest provides a `_ThreadingHTTPServer` helper so test servers can
handle concurrent requests. Single-threaded ``http.server.HTTPServer`` is
susceptible to head-of-line blocking when one handler sleeps (for example,
a stall/slow-then-hang endpoint). ThreadingMixIn avoids that by spawning a
new OS thread per request.
"""

import http.server
import socketserver


class _ThreadingHTTPServer(socketserver.ThreadingMixIn, http.server.HTTPServer):
    """HTTP test server that handles each request on its own thread.

    Daemon threads allow the interpreter to exit even if a handler is still
    running. ``allow_reuse_address`` makes port reuse predictable across
    sequential fixtures in the same test run.
    """

    daemon_threads = True
    allow_reuse_address = True
    request_queue_size = 32
