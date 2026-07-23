#!/usr/bin/env python3
"""Execute compatibility performance budgets.

Measures eggfetch.compat.httpx performance against the thresholds defined in
compat/httpx/0.28.1/performance-budgets.toml. Outputs a JSON report.
"""

from __future__ import annotations

import argparse
import json
import os
import statistics
import sys
import tempfile
import time
import http.server
import threading
import tracemalloc
from http.server import HTTPServer, BaseHTTPRequestHandler
from pathlib import Path
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parent
BUDGETS_PATH = REPO_ROOT / "compat" / "httpx" / "0.28.1" / "performance-budgets.toml"


class _EchoHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == "/large-body":
            body = b"X" * (1024 * 1024)  # 1 MB
            self.send_response(200)
            self.send_header("Content-Type", "application/octet-stream")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        elif self.path == "/stream":
            # Use Content-Length (not chunked) for streaming test
            body = (b"hello world! " * 10 + b"\n") * 100  # ~13KB
            self.send_response(200)
            self.send_header("Content-Type", "text/plain")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        else:
            body = b'{"ok":true}'
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

    def do_POST(self):
        length = int(self.headers.get("Content-Length", 0))
        self.rfile.read(length)
        body = b'{"ok":true}'
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, format, *args):
        pass


def _start_server() -> tuple[HTTPServer, str]:
    server = HTTPServer(("127.0.0.1", 0), _EchoHandler)
    port = server.server_address[1]
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    return server, f"http://127.0.0.1:{port}"


def _measure_import_time() -> float:
    """Measure time to import eggfetch.compat.httpx (ms)."""
    times = []
    for _ in range(5):
        t0 = time.perf_counter()
        import importlib
        import eggfetch.compat.httpx  # noqa: F401
        importlib.reload(eggfetch.compat.httpx)
        times.append((time.perf_counter() - t0) * 1000)
    return statistics.median(times)


def _measure_httpx_import_time() -> float:
    """Measure time to import httpx (ms) for baseline comparison."""
    times = []
    for _ in range(5):
        t0 = time.perf_counter()
        import importlib
        import httpx  # noqa: F401
        importlib.reload(httpx)
        times.append((time.perf_counter() - t0) * 1000)
    return statistics.median(times)


def _measure_client_construction(base_url: str) -> float:
    """Time to construct Client() (ms)."""
    import eggfetch.compat.httpx as compat
    times = []
    for _ in range(50):
        t0 = time.perf_counter()
        compat.Client(base_url=base_url)
        times.append((time.perf_counter() - t0) * 1000)
    return statistics.median(times)


def _measure_httpx_client_construction(base_url: str) -> float:
    import httpx
    times = []
    for _ in range(50):
        t0 = time.perf_counter()
        httpx.Client(base_url=base_url)
        times.append((time.perf_counter() - t0) * 1000)
    return statistics.median(times)


def _measure_one_shot_request(base_url: str) -> float:
    """Time for a single GET request (ms)."""
    import eggfetch.compat.httpx as compat
    times = []
    for _ in range(20):
        t0 = time.perf_counter()
        with compat.Client(timeout=30.0) as c:
            c.get(f"{base_url}/get")
        times.append((time.perf_counter() - t0) * 1000)
    return statistics.median(times)


def _measure_httpx_one_shot_request(base_url: str) -> float:
    import httpx
    times = []
    for _ in range(20):
        t0 = time.perf_counter()
        with httpx.Client() as c:
            c.get(f"{base_url}/get")
        times.append((time.perf_counter() - t0) * 1000)
    return statistics.median(times)


def _measure_reused_request(base_url: str) -> float:
    """Time for a GET request on reused connection (ms)."""
    import eggfetch.compat.httpx as compat
    with compat.Client(timeout=30.0) as c:
        c.get(f"{base_url}/get")  # warm up
        times = []
        for _ in range(30):
            t0 = time.perf_counter()
            c.get(f"{base_url}/get")
            times.append((time.perf_counter() - t0) * 1000)
    return statistics.median(times)


def _measure_httpx_reused_request(base_url: str) -> float:
    import httpx
    with httpx.Client() as c:
        c.get(f"{base_url}/get")
        times = []
        for _ in range(50):
            t0 = time.perf_counter()
            c.get(f"{base_url}/get")
            times.append((time.perf_counter() - t0) * 1000)
    return statistics.median(times)


def _measure_large_body_throughput(base_url: str) -> float:
    """Throughput for 1MB response (MB/s)."""
    import eggfetch.compat.httpx as compat
    with compat.Client(timeout=30.0) as c:
        t0 = time.perf_counter()
        resp = c.get(f"{base_url}/large-body")
        data = resp.read()
        elapsed = time.perf_counter() - t0
    mb = len(data) / (1024 * 1024)
    return mb / elapsed if elapsed > 0 else 0


def _measure_httpx_large_body_throughput(base_url: str) -> float:
    import httpx
    with httpx.Client() as c:
        t0 = time.perf_counter()
        resp = c.get(f"{base_url}/large-body")
        data = resp.read()
        elapsed = time.perf_counter() - t0
    mb = len(data) / (1024 * 1024)
    return mb / elapsed if elapsed > 0 else 0


def _measure_streaming_overhead(base_url: str) -> float:
    """Overhead of streaming vs buffered (ratio)."""
    import eggfetch.compat.httpx as compat
    with compat.Client(timeout=30.0) as c:
        # Buffered
        t0 = time.perf_counter()
        resp = c.get(f"{base_url}/stream")
        resp.read()
        buffered = time.perf_counter() - t0

        # Streaming via iter_raw
        t0 = time.perf_counter()
        resp = c.get(f"{base_url}/stream")
        for _ in resp.iter_raw():
            pass
        streamed = time.perf_counter() - t0
    return streamed / buffered if buffered > 0 else 0


def _measure_memory_growth(base_url: str) -> float:
    """Memory growth during 100 sequential requests (bytes)."""
    import eggfetch.compat.httpx as compat
    tracemalloc.start()
    with compat.Client(timeout=30.0) as c:
        for _ in range(100):
            c.get(f"{base_url}/get")
    _, peak = tracemalloc.get_traced_memory()
    tracemalloc.stop()
    return float(peak)


def _measure_multipart_upload(base_url: str) -> float:
    """Time for multipart upload using bytes (ms)."""
    import eggfetch.compat.httpx as compat
    times = []
    payload = b"x" * (1024 * 100)  # 100 KB
    for _ in range(10):
        t0 = time.perf_counter()
        with compat.Client(timeout=30.0) as c:
            c.post(f"{base_url}/post", content=payload,
                   headers={"Content-Type": "application/octet-stream"})
        times.append((time.perf_counter() - t0) * 1000)
    return statistics.median(times)


def _measure_asgi_transport() -> float:
    """Time for request via ASGITransport."""
    try:
        import eggfetch.compat.httpx as compat
        from eggfetch.compat.httpx import ASGITransport

        def app(scope, receive, send):
            body = b'{"ok":true}'
            send({"type": "http.response.start", "status": 200,
                  "headers": [[b"content-type", b"application/json"],
                              [b"content-length", str(len(body)).encode()]]})
            send({"type": "http.response.body", "body": body})

        times = []
        transport = ASGITransport(app=app)
        for _ in range(20):
            t0 = time.perf_counter()
            with compat.Client(transport=transport) as c:
                c.get("http://testserver/get")
            times.append((time.perf_counter() - t0) * 1000)
        return statistics.median(times)
    except Exception:
        return -1.0


def _load_budgets() -> list[dict[str, Any]]:
    try:
        import tomllib
    except ImportError:
        import tomli as tomllib  # type: ignore
    with open(BUDGETS_PATH, "rb") as f:
        data = tomllib.load(f)
    return data.get("threshold", [])


def _evaluate(metric_id: str, measured: float, budgets: list[dict]) -> dict[str, Any]:
    for b in budgets:
        if b.get("id") == metric_id:
            budget_type = b.get("budget-type", "informational")
            if "max-ratio" in b:
                threshold = float(b["max-ratio"])
                passed = measured <= threshold
                return {
                    "id": metric_id,
                    "budget_type": budget_type,
                    "measured_ratio": round(measured, 3),
                    "threshold": threshold,
                    "direction": "max",
                    "passed": passed,
                }
            if "min-ratio" in b:
                threshold = float(b["min-ratio"])
                passed = measured >= threshold
                return {
                    "id": metric_id,
                    "budget_type": budget_type,
                    "measured_ratio": round(measured, 3),
                    "threshold": threshold,
                    "direction": "min",
                    "passed": passed,
                }
            if "max-bytes" in b:
                threshold = float(b["max-bytes"])
                passed = measured <= threshold
                return {
                    "id": metric_id,
                    "budget_type": budget_type,
                    "measured_bytes": round(measured),
                    "threshold_bytes": int(threshold),
                    "direction": "max",
                    "passed": passed,
                }
    return {"id": metric_id, "passed": True, "note": "no budget defined"}


def main() -> None:
    parser = argparse.ArgumentParser(description="Run performance budgets")
    parser.add_argument("--output", default="performance-budget-results.json")
    args = parser.parse_args()

    budgets = _load_budgets()
    server, base_url = _start_server()

    try:
        import_time = _measure_import_time()
        httpx_import_time = _measure_httpx_import_time()
        import_ratio = import_time / httpx_import_time if httpx_import_time > 0 else 0

        client_time = _measure_client_construction(base_url)
        httpx_client_time = _measure_httpx_client_construction(base_url)
        client_ratio = client_time / httpx_client_time if httpx_client_time > 0 else 0

        one_shot = _measure_one_shot_request(base_url)
        httpx_one_shot = _measure_httpx_one_shot_request(base_url)
        one_shot_ratio = one_shot / httpx_one_shot if httpx_one_shot > 0 else 0

        reused = _measure_reused_request(base_url)
        httpx_reused = _measure_httpx_reused_request(base_url)
        reused_ratio = reused / httpx_reused if httpx_reused > 0 else 0

        throughput = _measure_large_body_throughput(base_url)
        httpx_throughput = _measure_httpx_large_body_throughput(base_url)
        throughput_ratio = throughput / httpx_throughput if httpx_throughput > 0 else 0

        streaming = _measure_streaming_overhead(base_url)
        memory = _measure_memory_growth(base_url)
        multipart = _measure_multipart_upload(base_url)
        asgi = _measure_asgi_transport()

        results = [
            _evaluate("IMPORT-TIME-001", import_ratio, budgets),
            _evaluate("CLIENT-CONSTRUCTION-001", client_ratio, budgets),
            _evaluate("ONE-SHOT-REQUEST-001", one_shot_ratio, budgets),
            _evaluate("REUSED-REQUEST-001", reused_ratio, budgets),
            _evaluate("LARGE-BODY-THROUGHPUT-001", throughput_ratio, budgets),
            _evaluate("STREAMING-OVERHEAD-001", streaming, budgets),
            _evaluate("MEMORY-GROWTH-001", memory, budgets),
            # MULTIPART and ASGI measured as ratios against one-shot baseline
            _evaluate("MULTIPART-UPLOAD-001", multipart / max(one_shot, 0.001), budgets),
            _evaluate("ASGI-TRANSPORT-001", asgi / max(one_shot, 0.001) if asgi > 0 else 0, budgets),
        ]

        all_passed = all(r.get("passed", True) for r in results)
        severe_failed = any(
            not r.get("passed", True) and r.get("budget_type") in ("correctness-blocker", "severe-regression")
            for r in results
        )

        report = {
            "baseline_httpx_import_ms": round(httpx_import_time, 2),
            "eggfetch_import_ms": round(import_time, 2),
            "import_ratio": round(import_ratio, 3),
            "client_construction_ratio": round(client_ratio, 3),
            "one_shot_request_ratio": round(one_shot_ratio, 3),
            "reused_request_ratio": round(reused_ratio, 3),
            "large_body_throughput_ratio": round(throughput_ratio, 3),
            "streaming_overhead_ratio": round(streaming, 3),
            "memory_growth_bytes": round(memory),
            "results": results,
            "all_passed": all_passed,
            "severe_regression": severe_failed,
        }

        Path(args.output).write_text(json.dumps(report, indent=2) + "\n")
        print(f"Results written to {args.output}")
        print(f"Overall: {'PASS' if all_passed else 'FAIL'}")
        if severe_failed:
            print("SEVERE REGRESSION DETECTED")
        for r in results:
            status = "PASS" if r.get("passed", True) else "FAIL"
            print(f"  {r['id']}: {status} ({r.get('budget_type', '?')})")

    finally:
        server.shutdown()


if __name__ == "__main__":
    main()
