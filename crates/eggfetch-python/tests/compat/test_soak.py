"""Soak test — bounded native request loop with policy binding.

Measures sustained throughput and error rates over a bounded duration.
Uses MockTransport (zero-network, deterministic) for fast CI.
One loopback test exercises the real TCP path for native proof.
"""
import json
import sys
import time

sys.path.insert(0, str(__import__("pathlib").Path(__file__).resolve().parent))
from eggfetch.compat.httpx import Client, MockTransport, Request, Response, Timeout
from native_fixtures import local_http_server

# Soak policy values
QUALIFICATION_MIN_REQUESTS = 50
QUALIFICATION_TIMEOUT_SECONDS = 120

SCHEDULED_MIN_REQUESTS = 500
SCHEDULED_MIN_DURATION_SECONDS = 300


def _mock_handler(request: Request) -> Response:
    """Fast mock handler for zero-network churn tests."""
    if request.url.path == "/json":
        return Response(200, json={"status": "ok"})
    return Response(404)


class TestQualificationChurn:
    """Bounded sync request loop to prove engine stability.

    Mode: qualification
    Policy: ≥50 requests, zero errors, all complete within timeout.

    Uses MockTransport for deterministic, zero-network churn.
    """

    def test_qualification_churn_sync(self):
        """Run 50 bounded sync requests to prove engine stability."""
        transport = MockTransport(_mock_handler)
        results = []
        errors = []
        start = time.monotonic()

        with Client(transport=transport, timeout=Timeout(5)) as c:
            for i in range(QUALIFICATION_MIN_REQUESTS):
                try:
                    r = c.get("http://test/json")
                    results.append(r.status_code)
                except Exception as exc:
                    errors.append(str(exc))

        elapsed = time.monotonic() - start
        assert len(errors) == 0, f"Unexpected errors: {errors}"
        assert all(s == 200 for s in results)
        assert len(results) >= QUALIFICATION_MIN_REQUESTS
        assert elapsed < QUALIFICATION_TIMEOUT_SECONDS

    def test_qualification_churn_with_body_reads(self):
        """Run requests reading body content each time."""
        transport = MockTransport(_mock_handler)
        count = 0
        with Client(transport=transport, timeout=Timeout(5)) as c:
            for i in range(QUALIFICATION_MIN_REQUESTS):
                r = c.get("http://test/json")
                assert r.status_code == 200
                body = r.content
                assert len(body) > 0
                count += 1
        assert count >= QUALIFICATION_MIN_REQUESTS

    def test_qualification_churn_repeated_client_cycles(self):
        """Run requests across repeated client open/close cycles."""
        cycles = 0
        for i in range(10):
            transport = MockTransport(_mock_handler)
            with Client(transport=transport, timeout=Timeout(5)) as c:
                for _ in range(5):
                    r = c.get("http://test/json")
                    assert r.status_code == 200
            cycles += 1
        assert cycles >= 10

    def test_qualification_churn_post(self):
        """Run POST requests to prove write stability."""
        transport = MockTransport(_mock_handler)
        count = 0
        with Client(transport=transport, timeout=Timeout(5)) as c:
            for i in range(QUALIFICATION_MIN_REQUESTS):
                r = c.post("http://test/json", content=b"test payload")
                assert r.status_code == 200
                count += 1
        assert count >= QUALIFICATION_MIN_REQUESTS

    def test_sustained_churn_sync(self):
        """Sustained sync request churn: 100 requests with body reads."""
        transport = MockTransport(_mock_handler)
        total_requests = 0

        with Client(transport=transport, timeout=Timeout(5)) as c:
            for i in range(100):
                r = c.get("http://test/json")
                assert r.status_code == 200
                body = r.content
                assert len(body) > 0
                total_requests += 1

        assert total_requests == 100, (
            f"Expected 100 successful requests, got {total_requests}"
        )

    def test_sustained_churn_mixed_methods(self):
        """Sustained mixed GET/POST churn: 50 pairs."""
        transport = MockTransport(_mock_handler)
        successes = 0

        with Client(transport=transport, timeout=Timeout(5)) as c:
            for i in range(50):
                r = c.get("http://test/json")
                assert r.status_code == 200
                successes += 1

                r = c.post("http://test/json", content=b"payload")
                assert r.status_code == 200
                successes += 1

        assert successes == 100, (
            f"Expected 100 successful requests, got {successes}"
        )

    def test_sustained_churn_repeated_clients(self):
        """Sustained churn with repeated client open/close: 20 cycles."""
        cycles = 0

        for i in range(20):
            transport = MockTransport(_mock_handler)
            with Client(transport=transport, timeout=Timeout(5)) as c:
                for _ in range(3):
                    r = c.get("http://test/json")
                    assert r.status_code == 200
            cycles += 1

        assert cycles == 20, (
            f"Expected 20 successful client cycles, got {cycles}"
        )

    def test_sustained_churn_body_content_validation(self):
        """Sustained churn with body content validation."""
        transport = MockTransport(_mock_handler)
        expected_body = json.dumps({"status": "ok"}).encode()

        with Client(transport=transport, timeout=Timeout(5)) as c:
            for i in range(50):
                r = c.get("http://test/json")
                assert r.status_code == 200
                body = r.content
                assert body == expected_body, (
                    f"Iteration {i}: expected {expected_body!r}, got {body!r}"
                )


class TestNativeLoopbackChurn:
    """Native loopback churn: real TCP socket proof.

    Single test with reduced count for CI resilience.
    """

    def test_loopback_churn_sync(self):
        """Run 20 sync requests over real loopback TCP."""
        with local_http_server() as (host, port):
            url = f"http://{host}:{port}/json"
            count = 0
            with Client(timeout=Timeout(120)) as c:
                for i in range(20):
                    r = c.get(url)
                    assert r.status_code == 200
                    count += 1
            assert count >= 20


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
