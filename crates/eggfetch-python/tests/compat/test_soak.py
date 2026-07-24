"""Short qualification churn test - bounded native request loop."""
import sys
import time

sys.path.insert(0, str(__import__("pathlib").Path(__file__).resolve().parent))
from eggfetch.compat.httpx import Client, Timeout
from native_fixtures import local_http_server


class TestQualificationChurn:
    """Bounded sync request loop to prove native stability."""

    def test_qualification_churn_sync(self):
        """Run 50 bounded sync requests to prove native stability."""
        with local_http_server() as (host, port):
            url = f"http://{host}:{port}/json"
            results = []
            start = time.monotonic()

            with Client(timeout=Timeout(10)) as c:
                for i in range(50):
                    r = c.get(url)
                    results.append(r.status_code)
                    if i % 20 == 0:
                        time.sleep(0.01)

            elapsed = time.monotonic() - start
            assert all(s == 200 for s in results)
            assert len(results) == 50
            assert elapsed < 60

    def test_qualification_churn_with_body_reads(self):
        """Run 50 requests reading body content each time."""
        with local_http_server() as (host, port):
            url = f"http://{host}:{port}/json"
            with Client(timeout=Timeout(10)) as c:
                for i in range(50):
                    r = c.get(url)
                    assert r.status_code == 200
                    body = r.content
                    assert len(body) > 0
                    if i % 20 == 0:
                        time.sleep(0.01)

    def test_qualification_churn_repeated_client_cycles(self):
        """Run requests across repeated client open/close cycles."""
        with local_http_server() as (host, port):
            url = f"http://{host}:{port}/json"
            for i in range(10):
                with Client(timeout=Timeout(10)) as c:
                    for _ in range(5):
                        r = c.get(url)
                        assert r.status_code == 200
                if i % 3 == 0:
                    time.sleep(0.01)

    def test_qualification_churn_post(self):
        """Run 50 POST requests to prove write stability."""
        with local_http_server() as (host, port):
            url = f"http://{host}:{port}/json"
            with Client(timeout=Timeout(10)) as c:
                for i in range(50):
                    r = c.post(url, content=b"test payload")
                    assert r.status_code == 200
                    if i % 20 == 0:
                        time.sleep(0.01)

    def test_sustained_churn_sync(self):
        """Sustained sync request churn: 200 requests with body reads.

        Exercises connection reuse, body reads, and response cleanup under
        sustained load. Uses a fixed iteration count for determinism.
        """
        with local_http_server() as (host, port):
            url = f"http://{host}:{port}/json"
            total_requests = 0
            errors = []

            with Client(timeout=Timeout(10)) as c:
                for i in range(200):
                    try:
                        r = c.get(url)
                        assert r.status_code == 200
                        _ = r.content
                        total_requests += 1
                        if i % 50 == 0:
                            time.sleep(0.01)
                    except Exception as exc:
                        errors.append(str(exc))

            assert total_requests > 150, f"Only {total_requests} requests completed"
            assert len(errors) <= 5, f"Too many errors during churn: {errors}"

    def test_sustained_churn_mixed_methods(self):
        """Sustained mixed GET/POST churn: 100 pairs."""
        with local_http_server() as (host, port):
            url = f"http://{host}:{port}/json"
            successes = 0
            errors = []

            with Client(timeout=Timeout(10)) as c:
                for i in range(100):
                    try:
                        r = c.get(url)
                        assert r.status_code == 200
                        successes += 1

                        r = c.post(url, content=b"payload")
                        assert r.status_code == 200
                        successes += 1
                        if i % 25 == 0:
                            time.sleep(0.01)
                    except Exception as exc:
                        errors.append(str(exc))

            assert successes > 100, f"Only {successes} requests completed"
            assert len(errors) <= 5, f"Too many errors: {errors}"

    def test_sustained_churn_repeated_clients(self):
        """Sustained churn with repeated client open/close: 50 cycles.

        This exercises resource cleanup under sustained create/destroy pressure.
        """
        with local_http_server() as (host, port):
            url = f"http://{host}:{port}/json"
            cycles = 0
            errors = []

            for i in range(50):
                try:
                    with Client(timeout=Timeout(10)) as c:
                        for _ in range(3):
                            r = c.get(url)
                            assert r.status_code == 200
                    cycles += 1
                    if i % 10 == 0:
                        time.sleep(0.01)
                except Exception as exc:
                    errors.append(str(exc))

            assert cycles > 30, f"Only {cycles} client cycles completed"
            assert len(errors) <= 5, f"Too many errors: {errors}"
