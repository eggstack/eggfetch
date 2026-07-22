"""Close/request race condition tests.

These tests verify that closing a client while requests are in-flight
does not cause panics, deadlocks, or resource leaks. They exercise the
thread-safety of the Mutex<Option<Client>> pattern in PyClient and the
&self pattern in PyAsyncClient.
"""

import asyncio
import http.server
import threading
import time

import pytest

import eggfetch


# ---------------------------------------------------------------------------
# Local test server with delayed response
# ---------------------------------------------------------------------------


class _SlowHandler(http.server.BaseHTTPRequestHandler):
    """Handler that sleeps before responding, allowing race windows."""

    def do_GET(self):
        if self.path.startswith("/slow"):
            time.sleep(0.5)  # 500ms delay
        body = b"OK"
        self.send_response(200)
        self.send_header("Content-Type", "text/plain")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, format, *args):
        pass  # suppress noisy output


@pytest.fixture(scope="module")
def slow_server():
    srv = http.server.HTTPServer(("127.0.0.1", 0), _SlowHandler)
    port = srv.server_address[1]
    t = threading.Thread(target=srv.serve_forever, daemon=True)
    t.start()
    yield f"http://127.0.0.1:{port}"
    srv.shutdown()


# ---------------------------------------------------------------------------
# Sync client: close during request
# ---------------------------------------------------------------------------


class TestSyncCloseDuringRequest:
    """Test that closing a sync client while requests are in-flight
    does not cause panics or deadlocks."""

    def test_close_during_concurrent_requests(self, slow_server):
        """Close the client while multiple threads are making requests."""
        client = eggfetch.Client()
        errors = []

        def make_request(idx):
            try:
                r = client.get(f"{slow_server}/slow")
                # Request may succeed or fail — both are OK
            except ValueError as e:
                if "closed" not in str(e):
                    errors.append(f"thread {idx}: unexpected error: {e}")
            except Exception:
                pass  # Network errors from shutdown are expected

        threads = [threading.Thread(target=make_request, args=(i,)) for i in range(5)]
        for t in threads:
            t.start()

        # Close while requests are in-flight
        time.sleep(0.05)  # let requests start
        client.close()

        for t in threads:
            t.join(timeout=5)

        assert not errors, f"errors occurred: {errors}"

    def test_close_is_idempotent_under_contention(self, slow_server):
        """Multiple threads calling close() simultaneously should not panic."""
        client = eggfetch.Client()
        errors = []

        def close_client(idx):
            try:
                client.close()
            except Exception as e:
                errors.append(f"thread {idx}: unexpected error: {e}")

        threads = [threading.Thread(target=close_client, args=(i,)) for i in range(10)]
        for t in threads:
            t.start()
        for t in threads:
            t.join(timeout=5)

        assert client.is_closed
        assert not errors, f"errors occurred: {errors}"

    def test_request_after_close_from_other_thread(self, slow_server):
        """Request from one thread, close from another — no deadlock."""
        client = eggfetch.Client()
        results = []
        close_event = threading.Event()

        def make_request():
            try:
                r = client.get(f"{slow_server}/slow")
                results.append("success")
            except ValueError as e:
                if "closed" in str(e):
                    results.append("closed")
                else:
                    results.append(f"error: {e}")
            except Exception:
                pass  # Network errors from shutdown are expected

        def close_later():
            close_event.wait()
            client.close()

        req_thread = threading.Thread(target=make_request)
        close_thread = threading.Thread(target=close_later)

        req_thread.start()
        close_thread.start()

        close_event.set()  # signal close
        req_thread.join(timeout=5)
        close_thread.join(timeout=5)

        assert len(results) <= 1
        if results:
            assert results[0] in ("success", "closed")


# ---------------------------------------------------------------------------
# Async client: close during request
# ---------------------------------------------------------------------------


class TestAsyncCloseDuringRequest:
    """Test that closing an async client while requests are in-flight
    does not cause panics or deadlocks."""

    def test_close_during_concurrent_requests(self, slow_server):
        async def _test():
            client = eggfetch.AsyncClient()
            tasks = [client.get(f"{slow_server}/slow") for _ in range(5)]

            # Start tasks
            running = [asyncio.ensure_future(t) for t in tasks]

            # Let them start
            await asyncio.sleep(0.05)

            # Close while requests are in-flight
            client.close()

            # Gather results — some may fail with "closed"
            results = await asyncio.gather(*running, return_exceptions=True)
            for r in results:
                if isinstance(r, Exception):
                    assert "closed" in str(r) or "connect" in str(r).lower()

        asyncio.run(_test())

    def test_close_is_idempotent_under_contention(self, slow_server):
        async def _test():
            client = eggfetch.AsyncClient()

            async def close_client():
                client.close()

            # Multiple concurrent close calls
            await asyncio.gather(*[close_client() for _ in range(10)])
            assert client.is_closed

        asyncio.run(_test())

    def test_request_after_close_from_other_task(self, slow_server):
        """Request from one task, close from another — no deadlock."""
        async def _test():
            client = eggfetch.AsyncClient()
            results = []

            async def make_request():
                try:
                    r = await client.get(f"{slow_server}/slow")
                    results.append("success")
                except ValueError as e:
                    if "closed" in str(e):
                        results.append("closed")
                    else:
                        results.append(f"error: {e}")
                except Exception as e:
                    results.append(f"error: {e}")

            async def close_later():
                await asyncio.sleep(0.05)
                client.close()

            # Start request and close tasks concurrently
            req_task = asyncio.ensure_future(make_request())
            close_task = asyncio.ensure_future(close_later())

            await asyncio.gather(req_task, close_task, return_exceptions=True)

            assert len(results) == 1
            assert results[0] in ("success", "closed")

        asyncio.run(_test())


# ---------------------------------------------------------------------------
# Context manager: close during request
# ---------------------------------------------------------------------------


class TestContextManagerCloseDuringRequest:
    """Test that exiting a context manager while requests are in-flight
    handles gracefully."""

    def test_sync_context_exit_during_request(self, slow_server):
        errors = []

        def make_request(client):
            try:
                r = client.get(f"{slow_server}/slow")
            except ValueError as e:
                if "closed" not in str(e):
                    errors.append(f"unexpected error: {e}")
            except Exception:
                pass  # Network errors from shutdown are expected

        with eggfetch.Client() as client:
            threads = [threading.Thread(target=make_request, args=(client,)) for _ in range(3)]
            for t in threads:
                t.start()
            time.sleep(0.05)  # let requests start
            # __exit__ will call close()

        for t in threads:
            t.join(timeout=5)

        assert not errors, f"errors occurred: {errors}"

    def test_async_context_exit_during_request(self, slow_server):
        async def _test():
            errors = []

            async with eggfetch.AsyncClient() as client:
                tasks = [client.get(f"{slow_server}/slow") for _ in range(3)]
                running = [asyncio.ensure_future(t) for t in tasks]
                await asyncio.sleep(0.05)  # let requests start
                # __aexit__ will call close()

            # Gather results
            results = await asyncio.gather(*running, return_exceptions=True)
            for r in results:
                if isinstance(r, Exception):
                    if "closed" not in str(r) and "connect" not in str(r).lower():
                        errors.append(f"unexpected error: {r}")

            assert not errors, f"errors occurred: {errors}"

        asyncio.run(_test())
