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

            with Client(timeout=Timeout(5)) as c:
                for _ in range(50):
                    r = c.get(url)
                    results.append(r.status_code)

            elapsed = time.monotonic() - start
            assert all(s == 200 for s in results)
            assert len(results) == 50
            assert elapsed < 30

    def test_qualification_churn_with_body_reads(self):
        """Run 50 requests reading body content each time."""
        with local_http_server() as (host, port):
            url = f"http://{host}:{port}/json"
            with Client(timeout=Timeout(5)) as c:
                for _ in range(50):
                    r = c.get(url)
                    assert r.status_code == 200
                    body = r.content
                    assert len(body) > 0

    def test_qualification_churn_repeated_client_cycles(self):
        """Run requests across repeated client open/close cycles."""
        with local_http_server() as (host, port):
            url = f"http://{host}:{port}/json"
            for _ in range(10):
                with Client(timeout=Timeout(5)) as c:
                    for _ in range(5):
                        r = c.get(url)
                        assert r.status_code == 200

    def test_qualification_churn_post(self):
        """Run 50 POST requests to prove write stability."""
        with local_http_server() as (host, port):
            url = f"http://{host}:{port}/json"
            with Client(timeout=Timeout(5)) as c:
                for _ in range(50):
                    r = c.post(url, content=b"test payload")
                    assert r.status_code == 200
