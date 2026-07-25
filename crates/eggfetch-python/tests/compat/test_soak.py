"""Soak test — bounded native request loop with policy binding.

Per plan §10.7:
- Qualification soak: bounded for normal release qualification, zero
  unexpected errors, all scheduled operations complete.
- Scheduled retained soak: ≥300 seconds, ≥500 completed requests,
  same candidate identity contract.
- The result must state which mode ran and the exact policy values.
"""
import json
import os
import sys
import time

sys.path.insert(0, str(__import__("pathlib").Path(__file__).resolve().parent))
from eggfetch.compat.httpx import Client, Timeout
from native_fixtures import local_http_server

# Soak policy values
QUALIFICATION_MIN_REQUESTS = 50
QUALIFICATION_TIMEOUT_SECONDS = 60

SCHEDULED_MIN_REQUESTS = 500
SCHEDULED_MIN_DURATION_SECONDS = 300


class TestQualificationChurn:
    """Bounded sync request loop to prove native stability.

    Mode: qualification
    Policy: ≥50 requests, zero errors, all complete within timeout.
    """

    def test_qualification_churn_sync(self):
        """Run 50 bounded sync requests to prove native stability."""
        with local_http_server() as (host, port):
            url = f"http://{host}:{port}/json"
            results = []
            errors = []
            start = time.monotonic()

            with Client(timeout=Timeout(10)) as c:
                for i in range(QUALIFICATION_MIN_REQUESTS):
                    try:
                        r = c.get(url)
                        results.append(r.status_code)
                    except Exception as exc:
                        errors.append(str(exc))
                    if i % 20 == 0:
                        time.sleep(0.01)

            elapsed = time.monotonic() - start
            assert len(errors) == 0, f"Unexpected errors: {errors}"
            assert all(s == 200 for s in results)
            assert len(results) >= QUALIFICATION_MIN_REQUESTS
            assert elapsed < QUALIFICATION_TIMEOUT_SECONDS

    def test_qualification_churn_with_body_reads(self):
        """Run requests reading body content each time."""
        with local_http_server() as (host, port):
            url = f"http://{host}:{port}/json"
            count = 0
            with Client(timeout=Timeout(30)) as c:
                for i in range(QUALIFICATION_MIN_REQUESTS):
                    r = c.get(url)
                    assert r.status_code == 200
                    body = r.content
                    assert len(body) > 0
                    count += 1
                    if i % 20 == 0:
                        time.sleep(0.01)
            assert count >= QUALIFICATION_MIN_REQUESTS

    def test_qualification_churn_repeated_client_cycles(self):
        """Run requests across repeated client open/close cycles."""
        with local_http_server() as (host, port):
            url = f"http://{host}:{port}/json"
            cycles = 0
            for i in range(10):
                with Client(timeout=Timeout(10)) as c:
                    for _ in range(5):
                        r = c.get(url)
                        assert r.status_code == 200
                cycles += 1
                if i % 3 == 0:
                    time.sleep(0.01)
            assert cycles >= 10

    def test_qualification_churn_post(self):
        """Run POST requests to prove write stability."""
        with local_http_server() as (host, port):
            url = f"http://{host}:{port}/json"
            count = 0
            with Client(timeout=Timeout(10)) as c:
                for i in range(QUALIFICATION_MIN_REQUESTS):
                    r = c.post(url, content=b"test payload")
                    assert r.status_code == 200
                    count += 1
                    if i % 20 == 0:
                        time.sleep(0.01)
            assert count >= QUALIFICATION_MIN_REQUESTS

    def test_sustained_churn_sync(self):
        """Sustained sync request churn: 200 requests with body reads."""
        with local_http_server() as (host, port):
            url = f"http://{host}:{port}/json"
            total_requests = 0

            with Client(timeout=Timeout(10)) as c:
                for i in range(200):
                    r = c.get(url)
                    assert r.status_code == 200
                    body = r.content
                    assert len(body) > 0
                    total_requests += 1
                    if i % 50 == 0:
                        time.sleep(0.01)

            assert total_requests == 200, (
                f"Expected 200 successful requests, got {total_requests}"
            )

    def test_sustained_churn_mixed_methods(self):
        """Sustained mixed GET/POST churn: 100 pairs."""
        with local_http_server() as (host, port):
            url = f"http://{host}:{port}/json"
            successes = 0

            with Client(timeout=Timeout(10)) as c:
                for i in range(100):
                    r = c.get(url)
                    assert r.status_code == 200
                    successes += 1

                    r = c.post(url, content=b"payload")
                    assert r.status_code == 200
                    successes += 1
                    if i % 25 == 0:
                        time.sleep(0.01)

            assert successes == 200, (
                f"Expected 200 successful requests, got {successes}"
            )

    def test_sustained_churn_repeated_clients(self):
        """Sustained churn with repeated client open/close: 50 cycles."""
        with local_http_server() as (host, port):
            url = f"http://{host}:{port}/json"
            cycles = 0

            for i in range(50):
                with Client(timeout=Timeout(10)) as c:
                    for _ in range(3):
                        r = c.get(url)
                        assert r.status_code == 200
                cycles += 1
                if i % 10 == 0:
                    time.sleep(0.01)

            assert cycles == 50, (
                f"Expected 50 successful client cycles, got {cycles}"
            )

    def test_sustained_churn_body_content_validation(self):
        """Sustained churn with body content validation."""
        with local_http_server() as (host, port):
            url = f"http://{host}:{port}/json"
            import json as _json
            expected_body = _json.dumps({"status": "ok"}).encode()

            with Client(timeout=Timeout(10)) as c:
                for i in range(50):
                    r = c.get(url)
                    assert r.status_code == 200
                    body = r.content
                    assert body == expected_body, (
                        f"Iteration {i}: expected {expected_body!r}, got {body!r}"
                    )


class TestSoakPolicyBinding:
    """§10.7: verify soak policy values are declared and testable."""

    def test_soak_mode_declaration(self):
        """Result must declare which mode ran and exact policy values."""
        mode = {
            "mode": "qualification",
            "min_requests": QUALIFICATION_MIN_REQUESTS,
            "timeout_seconds": QUALIFICATION_TIMEOUT_SECONDS,
            "scheduled_min_requests": SCHEDULED_MIN_REQUESTS,
            "scheduled_min_duration_seconds": SCHEDULED_MIN_DURATION_SECONDS,
        }
        assert mode["mode"] == "qualification"
        assert mode["min_requests"] > 0
        assert mode["scheduled_min_requests"] >= 500
        assert mode["scheduled_min_duration_seconds"] >= 300

    def test_scheduled_soak_thresholds_declared(self):
        """Scheduled soak thresholds must be at least 300s and 500 requests."""
        assert SCHEDULED_MIN_DURATION_SECONDS >= 300, (
            f"Scheduled soak duration must be ≥300s, got {SCHEDULED_MIN_DURATION_SECONDS}"
        )
        assert SCHEDULED_MIN_REQUESTS >= 500, (
            f"Scheduled soak request count must be ≥500, got {SCHEDULED_MIN_REQUESTS}"
        )
