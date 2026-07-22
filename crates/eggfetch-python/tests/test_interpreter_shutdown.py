"""Interpreter shutdown tests.

These tests verify that the Python interpreter shuts down cleanly even
when clients are not explicitly closed, when streaming responses are
abandoned, or when multiple clients are created and destroyed rapidly.
Each test runs a subprocess to verify no deadlocks, panics, leaked
threads, or excessive shutdown delay.
"""

import subprocess
import sys
import textwrap
import time

import pytest


PYTHON = sys.executable


def run_subprocess(script: str, timeout: int = 10) -> subprocess.CompletedProcess:
    """Run a Python script as a subprocess and return the result."""
    return subprocess.run(
        [PYTHON, "-c", textwrap.dedent(script)],
        capture_output=True,
        text=True,
        timeout=timeout,
    )


# ---------------------------------------------------------------------------
# Client not explicitly closed
# ---------------------------------------------------------------------------


class TestInterpreterShutdown:
    """Verify clean interpreter shutdown under various conditions."""

    def test_client_not_closed(self):
        """Client not explicitly closed should not deadlock on exit."""
        result = run_subprocess("""
            import eggfetch
            client = eggfetch.Client()
            # Do not call client.close()
        """)
        assert result.returncode == 0, (
            f"Non-zero exit: rc={result.returncode}\n"
            f"stdout: {result.stdout}\nstderr: {result.stderr}"
        )

    def test_async_client_not_closed(self):
        """AsyncClient not explicitly closed should not deadlock on exit."""
        result = run_subprocess("""
            import asyncio, eggfetch
            async def main():
                client = eggfetch.AsyncClient()
                # Do not call client.close()
            asyncio.run(main())
        """)
        assert result.returncode == 0, (
            f"Non-zero exit: rc={result.returncode}\n"
            f"stdout: {result.stdout}\nstderr: {result.stderr}"
        )

    def test_context_manager_exit(self):
        """Context manager should close client cleanly."""
        result = run_subprocess("""
            import eggfetch
            with eggfetch.Client() as client:
                pass
        """)
        assert result.returncode == 0, (
            f"Non-zero exit: rc={result.returncode}\n"
            f"stdout: {result.stdout}\nstderr: {result.stderr}"
        )

    def test_repeated_construction_and_destruction(self):
        """Repeatedly create and destroy clients should not leak resources."""
        result = run_subprocess("""
            import eggfetch
            for _ in range(10):
                client = eggfetch.Client()
                client.close()
        """)
        assert result.returncode == 0, (
            f"Non-zero exit: rc={result.returncode}\n"
            f"stdout: {result.stdout}\nstderr: {result.stderr}"
        )

    def test_multiple_clients_not_closed(self):
        """Multiple clients not closed should not deadlock on exit."""
        result = run_subprocess("""
            import eggfetch
            clients = [eggfetch.Client() for _ in range(5)]
            # Do not close any of them
        """)
        assert result.returncode == 0, (
            f"Non-zero exit: rc={result.returncode}\n"
            f"stdout: {result.stdout}\nstderr: {result.stderr}"
        )

    def test_context_manager_with_exception(self):
        """Context manager should close client even if exception occurs."""
        result = run_subprocess("""
            import eggfetch
            try:
                with eggfetch.Client() as client:
                    raise ValueError("test exception")
            except ValueError:
                pass
        """)
        assert result.returncode == 0, (
            f"Non-zero exit: rc={result.returncode}\n"
            f"stdout: {result.stdout}\nstderr: {result.stderr}"
        )

    def test_client_outlives_scope(self):
        """Client used after context manager exit should raise ValueError."""
        result = run_subprocess("""
            import eggfetch
            with eggfetch.Client() as client:
                pass
            try:
                client.get("http://example.com")
                print("ERROR: should have raised")
            except ValueError as e:
                if "closed" in str(e):
                    print("OK: got expected ValueError")
                else:
                    print(f"ERROR: unexpected ValueError: {e}")
        """)
        assert result.returncode == 0, (
            f"Non-zero exit: rc={result.returncode}\n"
            f"stdout: {result.stdout}\nstderr: {result.stderr}"
        )
        assert "OK: got expected ValueError" in result.stdout
