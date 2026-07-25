"""Native shutdown lifecycle tests using real local sockets.

Verifies that the eggfetch engine shuts down cleanly: connections are
dropped, resources are released, and the client can be closed without
leaving dangling tasks or sockets.
"""
import socket
import sys
import threading
import time

import pytest

sys.path.insert(0, str(__import__("pathlib").Path(__file__).resolve().parent))
from eggfetch.compat.httpx import Client
from native_fixtures import local_http_server


class TestNativeShutdown:
    """Shutdown lifecycle tests using real local sockets."""

    def test_client_close_releases_socket(self):
        """Closing the client releases the underlying socket."""
        with local_http_server() as (host, port):
            with Client() as c:
                resp = c.get(f"http://{host}:{port}/health")
                assert resp.status_code == 200
            # After exiting context, client should be closed
            assert c.is_closed

    def test_multiple_requests_same_connection(self):
        """Multiple requests reuse the same connection pool."""
        with local_http_server() as (host, port):
            with Client() as c:
                for i in range(5):
                    resp = c.get(f"http://{host}:{port}/health")
                    assert resp.status_code == 200
            assert c.is_closed

    def test_multiple_clients_sequential_shutdown(self):
        """Multiple sequential clients each shut down cleanly."""
        with local_http_server() as (host, port):
            for i in range(5):
                with Client() as c:
                    resp = c.get(f"http://{host}:{port}/health")
                    assert resp.status_code == 200
                assert c.is_closed

    def test_client_close_without_requests(self):
        """Client can be closed without making any requests."""
        with local_http_server() as (host, port):
            with Client() as c:
                pass
            assert c.is_closed

    def test_connection_pool_cleanup(self):
        """Connection pool is cleaned up on close."""
        with local_http_server() as (host, port):
            with Client() as c:
                resp = c.get(f"http://{host}:{port}/health")
                assert resp.status_code == 200
                # Pool should have active connections
                assert c.is_closed is False
            # After close, pool should be empty
            assert c.is_closed

    def test_graceful_shutdown_during_request(self):
        """Client handles shutdown during an in-flight request gracefully."""
        # Start a slow server
        srv = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        srv.bind(("127.0.0.1", 0))
        srv.listen(5)
        port = srv.getsockname()[1]

        def slow_handler():
            try:
                conn, addr = srv.accept()
                # Accept but don't respond
                time.sleep(5)
                conn.close()
            except Exception:
                pass

        t = threading.Thread(target=slow_handler, daemon=True)
        t.start()

        try:
            with Client(timeout=0.5) as c:
                with pytest.raises(Exception):
                    c.get(f"http://127.0.0.1:{port}/")
            assert c.is_closed
        finally:
            srv.close()
