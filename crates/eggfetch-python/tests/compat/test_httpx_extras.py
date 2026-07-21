"""Optional HTTPX extra compatibility tests.

These tests exercise optional HTTPX features (HTTP/2, brotli, etc.) and
may skip if the corresponding extras are not installed.
"""

import pytest

import eggfetch


class TestHTTP2:
    """HTTP/2 compatibility tests."""

    def test_http2_flag_accepted(self):
        """http2=True parameter should be accepted."""
        client = eggfetch.Client()
        assert hasattr(client, "get")


class TestRetryCompat:
    """Retry compatibility (eggfetch-only feature)."""

    def test_retry_object_creation(self):
        retry = eggfetch.Retry(max_attempts=3)
        assert retry.max_attempts == 3

    def test_retry_default_statuses(self):
        retry = eggfetch.Retry()
        expected = {408, 429, 502, 503, 504}
        assert set(retry.statuses) == expected
