#!/usr/bin/env python3
"""Qualification-only local measurements for the performance campaign.

This harness intentionally uses a deterministic loopback server and is not
part of routine CI. It exercises the public Python API so a benchmark result
cannot silently depend on private extension details.

The optional ``--concurrency`` group measures how throughput scales when
several threads share one Client. It exists so a GIL-enabled interpreter can
be compared against a free-threaded one; see the comment above
``CONCURRENCY_SERVER_SOURCE`` for what that comparison does and does not
establish.
"""

from __future__ import annotations

import argparse
import asyncio
import hashlib
import json
import os
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from threading import Barrier, Thread
from urllib.parse import parse_qs, urlsplit

import eggfetch

try:
    import resource
except ImportError:  # pragma: no cover - platform-dependent fallback
    resource = None


BODY = b"0123456789abcdef" * (64 * 1024)  # 1 MiB, split into large frames.
TEXT_BODY = ("row-€-" * 120_000).encode()
LINES_BODY = b"short line\n" * 120_000 + b"final partial line"


class Handler(BaseHTTPRequestHandler):
    def do_GET(self) -> None:  # noqa: N802 - stdlib protocol hook
        query = parse_qs(urlsplit(self.path).query)
        path = urlsplit(self.path).path
        if path == "/text":
            body = TEXT_BODY
            content_type = "text/plain; charset=utf-8"
        elif path == "/lines":
            body = LINES_BODY
            content_type = "text/plain; charset=utf-8"
        else:
            body = BODY
            content_type = "application/octet-stream"
        requested_size = query.get("size")
        if requested_size:
            size = int(requested_size[0])
            body = (body * ((size + len(body) - 1) // len(body)))[:size]
        header_count = int(query.get("headers", ["0"])[0])
        self.send_response(200)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Content-Type", content_type)
        self.send_header("Connection", "close")
        for index in range(header_count):
            self.send_header(f"X-Bench-{index}", "value")
        self.send_header("X-Multi", "first")
        self.send_header("X-Multi", "second")
        self.end_headers()
        try:
            for offset in range(0, len(body), 64 * 1024):
                self.wfile.write(body[offset : offset + 64 * 1024])
        except BrokenPipeError:
            # A header-construction control may close a streaming response
            # before consuming its body; that is an expected fixture event.
            pass

    def log_message(self, *_args: object) -> None:
        return


def measure(fn, repeats: int) -> dict[str, float]:
    samples = []
    for _ in range(repeats):
        started = time.perf_counter()
        fn()
        samples.append(time.perf_counter() - started)
    return {
        "median_seconds": statistics.median(samples),
        "min_seconds": min(samples),
        "max_seconds": max(samples),
    }


def peak_rss_mib() -> float | None:
    if resource is None:
        return None
    usage = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
    # Linux reports KiB, macOS reports bytes: branch on platform, not on
    # magnitude (the `usage < 10M` heuristic fails for large-Linux/small-macOS).
    if sys.platform == "darwin":
        return usage / (1024 * 1024)
    return usage / 1024


def measure_phases(fn, repeats: int) -> dict[str, object]:
    samples: dict[str, list[float]] = {}
    peak = None
    for _ in range(repeats):
        result = fn()
        peak = result.pop("peak_rss_mib", peak)
        for name, value in result.items():
            samples.setdefault(name, []).append(float(value))
    return {
        name: {
            "median_seconds": statistics.median(values),
            "min_seconds": min(values),
            "max_seconds": max(values),
        }
        for name, values in samples.items()
    } | ({"peak_rss_mib": peak} if peak is not None else {})


def buffered_iterator_case(client, url: str, kind: str) -> dict[str, float | None]:
    response = client.get(url)
    started = time.perf_counter()
    if kind == "bytes":
        iterator = response.iter_bytes(chunk_size=1024)
        expected = BODY if "/body" in url else response.content
        first = next(iterator, None)
        first_yield = time.perf_counter()
        rest = list(iterator)
        assert b"".join(([first] if first is not None else []) + rest) == expected
    elif kind == "text":
        iterator = response.iter_text(chunk_size=1024)
        expected = response.text
        first = next(iterator, None)
        first_yield = time.perf_counter()
        rest = list(iterator)
        assert "".join(([first] if first is not None else []) + rest) == expected
    else:
        iterator = response.iter_lines()
        expected = response.text.splitlines()
        first = next(iterator, None)
        first_yield = time.perf_counter()
        rest = list(iterator)
        assert ([first] if first is not None else []) + rest == expected
    finished = time.perf_counter()
    return {
        "construction_seconds": first_yield - started,
        "time_to_first_item_seconds": first_yield - started,
        "full_consumption_seconds": finished - started,
        "peak_rss_mib": peak_rss_mib(),
    }


def header_cases(client, base_url: str, count: int, streaming: bool) -> None:
    url = f"{base_url}/body?headers={count}&size=1"
    if streaming:
        with client.stream("GET", url) as response:
            assert response.headers.get_list("x-multi") == ["first", "second"]
            assert response.headers.get(f"x-bench-{count - 1}") == "value"
            assert b"".join(response.iter_bytes()) == BODY[:1]
    else:
        response = client.get(url)
        assert response.headers.get_list("x-multi") == ["first", "second"]
        assert response.headers.get(f"x-bench-{count - 1}") == "value"


def sync_cases(url: str, repeats: int) -> dict[str, object]:
    with eggfetch.Client() as client:
        def stream_small_chunks() -> None:
            with client.stream("GET", url) as response:
                assert b"".join(response.iter_bytes(chunk_size=1024)) == BODY

        def stream_lines() -> None:
            with client.stream("GET", url) as response:
                assert sum(1 for _ in response.iter_lines()) >= 1

        def buffered_without_text() -> None:
            response = client.get(url)
            assert response.content == BODY

        def buffered_with_text() -> None:
            response = client.get(url)
            assert len(response.text) == len(BODY)

        header_measurements = {}
        # Hyper's HTTP/1 parser intentionally caps response header count at
        # 100; use the largest admissible fixture for adapter conversion.
        for count in (8, 50, 80):
            header_measurements[f"buffered_headers_{count}"] = measure(
                lambda count=count: header_cases(client, url, count, False), repeats
            )
            header_measurements[f"streaming_headers_{count}"] = measure(
                lambda count=count: header_cases(client, url, count, True), repeats
            )

        return {
            "sync_iter_bytes_1k": measure(stream_small_chunks, repeats),
            "sync_iter_lines": measure(stream_lines, repeats),
            "buffered_without_text": measure(buffered_without_text, repeats),
            "buffered_first_text": measure(buffered_with_text, repeats),
            "buffered_iter_bytes": measure_phases(
                lambda: buffered_iterator_case(client, f"{url}/body", "bytes"), repeats
            ),
            "buffered_iter_text": measure_phases(
                lambda: buffered_iterator_case(client, f"{url}/text", "text"), repeats
            ),
            "buffered_iter_lines": measure_phases(
                lambda: buffered_iterator_case(client, f"{url}/lines", "lines"), repeats
            ),
            "header_construction": header_measurements,
        }


async def async_cases(url: str, repeats: int) -> dict[str, object]:
    async with eggfetch.AsyncClient() as client:
        async def stream_small_chunks() -> None:
            response = await client.stream("GET", url)
            try:
                chunks = [chunk async for chunk in response.aiter_bytes(chunk_size=1024)]
                assert b"".join(chunks) == BODY
            finally:
                await response.aclose()

        async def aread_case(size: int) -> None:
            response = await client.stream("GET", f"{url}/body?size={size}")
            try:
                content = await response.aread()
                assert type(content) is bytes
                assert len(content) == size
            finally:
                await response.aclose()

        aread_measurements = {}
        for size in (1024, 1 * 1024 * 1024, 4 * 1024 * 1024):
            aread_measurements[f"aread_{size}"] = await measure_async(
                lambda size=size: aread_case(size), repeats
            )

        return {
            "async_iter_bytes_1k": await measure_async(stream_small_chunks, repeats),
            "async_aread": aread_measurements,
        }


async def measure_async(fn, repeats: int) -> dict[str, float]:
    async def run() -> list[float]:
        samples = []
        for _ in range(repeats):
            started = time.perf_counter()
            await fn()
            samples.append(time.perf_counter() - started)
        return samples

    samples = await run()
    return {
        "median_seconds": statistics.median(samples),
        "min_seconds": min(samples),
        "max_seconds": max(samples),
    }


# ---------------------------------------------------------------------------
# Concurrency scaling
# ---------------------------------------------------------------------------

# This group exists to compare a GIL-enabled interpreter against a
# free-threaded one. The Criterion suites in `eggfetch-bench` measure
# `eggfetch-core` in Rust, so the interpreter's GIL state cannot appear in
# them at all; the measurement that can show a difference is a Python-level
# one with several threads sharing one Client.
#
# Two workloads are measured:
#
#   io     - request plus body read, with no extra Python-level CPU work.
#   io_cpu - the same, plus repeated hashing of the body. This models the
#             "CPU-bound Python work around network calls" shape, which is
#             where a GIL-enabled build serializes and a free-threaded build
#             does not. The engine already releases the GIL for the network
#             wait, so the io workload is expected to scale similarly on
#             both.
#
# The server runs in a separate process with a deliberately minimal
# per-request Python cost, so the client's own scaling curve is what gets
# measured. It is the same interpreter as the client, so the meaningful
# comparison is the scaling factor between thread counts within a single run,
# not absolute throughput between runs. A server ceiling would show up as the
# same plateau in both runs; a free-threaded run that out-scales the
# GIL-enabled run is evidence that the ceiling was the client, not the
# server.

CONCURRENCY_BODY_SIZE = 8 * 1024
CONCURRENCY_REQUESTS_PER_THREAD = 2000
CONCURRENCY_CPU_PASSES = 4
CONCURRENCY_JSON_RECORDS = 200
CONCURRENCY_WORKLOADS = ("io", "io_cpu", "json")

CONCURRENCY_SERVER_SOURCE = r"""
import json
import socket
import sys
import threading

body_size = int(sys.argv[1])
record_count = int(sys.argv[2])
port_file = sys.argv[3]

body = b"x" * body_size
binary_head = (
    "HTTP/1.1 200 OK\r\n"
    "Content-Length: %d\r\n"
    "Content-Type: application/octet-stream\r\n"
    "\r\n" % body_size
).encode()

records = [
    {
        "id": index,
        "name": "widget-%d" % index,
        "price": round(index * 1.5, 2),
        "tags": ["alpha", "beta", "gamma"],
        "active": index % 3 != 0,
    }
    for index in range(record_count)
]
json_body = json.dumps(records).encode()
json_head = (
    "HTTP/1.1 200 OK\r\n"
    "Content-Length: %d\r\n"
    "Content-Type: application/json\r\n"
    "\r\n" % len(json_body)
).encode()


def handle(connection):
    connection.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
    buffer = b""
    try:
        while True:
            while b"\r\n\r\n" not in buffer:
                chunk = connection.recv(65536)
                if not chunk:
                    return
                buffer += chunk
            separator = buffer.index(b"\r\n\r\n")
            request_head = buffer[:separator]
            buffer = buffer[separator + 4 :]
            if b"/json" in request_head:
                connection.sendall(json_head + json_body)
            else:
                connection.sendall(binary_head + body)
    except OSError:
        pass
    finally:
        try:
            connection.close()
        except OSError:
            pass


server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
server.bind(("127.0.0.1", 0))
server.listen(256)
with open(port_file, "w") as port_handle:
    port_handle.write(str(server.getsockname()[1]))
while True:
    connection, _ = server.accept()
    threading.Thread(target=handle, args=(connection,), daemon=True).start()
"""


def gil_enabled() -> bool | None:
    """Whether the running interpreter has the GIL enabled, if it can tell."""
    probe = getattr(sys, "_is_gil_enabled", None)
    if probe is None:
        return None
    return bool(probe())


def start_concurrency_server() -> tuple[int, subprocess.Popen, str]:
    """Start the minimal loopback server in a separate process."""
    directory = tempfile.mkdtemp(prefix="eggfetch-perf-concurrency-")
    port_file = os.path.join(directory, "port")
    server = subprocess.Popen(
        [
            sys.executable,
            "-c",
            CONCURRENCY_SERVER_SOURCE,
            str(CONCURRENCY_BODY_SIZE),
            str(CONCURRENCY_JSON_RECORDS),
            port_file,
        ],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    try:
        for _ in range(400):
            if os.path.exists(port_file):
                with open(port_file) as handle:
                    text = handle.read().strip()
                if text:
                    return int(text), server, directory
            time.sleep(0.05)
    except BaseException:
        server.kill()
        shutil.rmtree(directory, ignore_errors=True)
        raise
    server.kill()
    shutil.rmtree(directory, ignore_errors=True)
    raise RuntimeError("concurrency server did not start")


def concurrency_application_work(payload: bytes, workload: str) -> None:
    """The per-response application work a workload models."""
    if workload == "io_cpu":
        for _ in range(CONCURRENCY_CPU_PASSES):
            hashlib.sha256(payload).hexdigest()
    elif workload == "json":
        records = json.loads(payload)
        if len(records) != CONCURRENCY_JSON_RECORDS:
            raise RuntimeError("decoded %d records" % len(records))
        summary = [(record["id"], record["price"], record["active"]) for record in records]
        if len(summary) != CONCURRENCY_JSON_RECORDS:
            raise RuntimeError("summarised %d records" % len(summary))


def concurrency_once(
    client, url: str, threads: int, workload: str, body_size: int | None
) -> tuple[float, list[float], list[float]]:
    """Run one concurrency sample.

    Returns the wall time in seconds plus the per-request latency samples:
    the first series covers the request and body read, the second covers the
    whole user-visible operation including the modelled application work.
    """
    errors: list[Exception] = []
    barrier = Barrier(threads)
    samples: list[tuple[list[float], list[float]]] = []

    def worker() -> None:
        request_latencies: list[float] = []
        total_latencies: list[float] = []
        try:
            barrier.wait(timeout=120)
            for _ in range(CONCURRENCY_REQUESTS_PER_THREAD):
                started = time.perf_counter()
                response = client.get(url)
                if response.status_code != 200:
                    raise RuntimeError("status %s" % response.status_code)
                payload = response.content
                if body_size is not None and len(payload) != body_size:
                    raise RuntimeError("body length %d" % len(payload))
                received = time.perf_counter()
                concurrency_application_work(payload, workload)
                finished = time.perf_counter()
                request_latencies.append(received - started)
                total_latencies.append(finished - started)
        except Exception as exc:  # pragma: no cover - re-raised by the caller
            errors.append(exc)
        samples.append((request_latencies, total_latencies))

    workers = [Thread(target=worker) for _ in range(threads)]
    started = time.perf_counter()
    for thread in workers:
        thread.start()
    for thread in workers:
        thread.join(timeout=300)
    elapsed = time.perf_counter() - started
    if any(thread.is_alive() for thread in workers):
        raise RuntimeError("concurrency workers did not finish")
    if errors:
        raise errors[0]
    request_all = [value for request, _ in samples for value in request]
    total_all = [value for _, total in samples for value in total]
    return elapsed, request_all, total_all


def percentile(sorted_values: list[float], fraction: float) -> float | None:
    """Nearest-rank percentile of an already sorted sample."""
    if not sorted_values:
        return None
    index = int(round(fraction * (len(sorted_values) - 1)))
    return sorted_values[min(max(index, 0), len(sorted_values) - 1)]


def latency_summary(values: list[float]) -> dict[str, float | None]:
    """Latency distribution in milliseconds."""
    if not values:
        return {"p50_ms": None, "p95_ms": None, "p99_ms": None, "max_ms": None}
    ordered = sorted(values)
    return {
        "p50_ms": percentile(ordered, 0.50) * 1000,
        "p95_ms": percentile(ordered, 0.95) * 1000,
        "p99_ms": percentile(ordered, 0.99) * 1000,
        "max_ms": ordered[-1] * 1000,
    }


def concurrency_cases(
    repeats: int, thread_counts: list[int], workloads: list[str]
) -> dict[str, object]:
    """Measure throughput, scaling and latency at each thread count."""
    port, server, directory = start_concurrency_server()
    try:
        results: dict[str, dict[str, object]] = {}
        with eggfetch.Client() as client:
            for workload in workloads:
                if workload == "json":
                    url = f"http://127.0.0.1:{port}/json"
                    body_size = None
                else:
                    url = f"http://127.0.0.1:{port}/binary"
                    body_size = CONCURRENCY_BODY_SIZE
                for threads in thread_counts:
                    wall: list[float] = []
                    request_latencies: list[float] = []
                    total_latencies: list[float] = []
                    for _ in range(repeats):
                        elapsed, requests, totals = concurrency_once(
                            client, url, threads, workload, body_size
                        )
                        wall.append(elapsed)
                        request_latencies.extend(requests)
                        total_latencies.extend(totals)
                    best = min(wall)
                    total = threads * CONCURRENCY_REQUESTS_PER_THREAD
                    results[f"{workload}/{threads}"] = {
                        "best_seconds": best,
                        "median_seconds": statistics.median(wall),
                        "throughput_rps": total / best,
                        "request_latency": latency_summary(request_latencies),
                        "total_latency": latency_summary(total_latencies),
                        "samples": len(total_latencies),
                    }
    finally:
        server.kill()
        shutil.rmtree(directory, ignore_errors=True)

    baseline: dict[str, float] = {}
    for workload in workloads:
        first = results.get(f"{workload}/{thread_counts[0]}")
        if first is not None:
            baseline[workload] = first["throughput_rps"]
    for key, entry in results.items():
        workload = key.split("/")[0]
        base = baseline.get(workload)
        if base:
            entry["scaling_vs_first_thread_count"] = entry["throughput_rps"] / base

    return {
        "interpreter": sys.version.split()[0],
        "gil_enabled": gil_enabled(),
        "body_size": CONCURRENCY_BODY_SIZE,
        "json_records": CONCURRENCY_JSON_RECORDS,
        "requests_per_thread": CONCURRENCY_REQUESTS_PER_THREAD,
        "cpu_passes": CONCURRENCY_CPU_PASSES,
        "thread_counts": thread_counts,
        "workloads": workloads,
        "repeats": repeats,
        "results": results,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repeats", type=int, default=3)
    parser.add_argument(
        "--concurrency",
        action="store_true",
        help=(
            "also measure throughput scaling across thread counts. Use this to "
            "compare a GIL-enabled interpreter against a free-threaded one."
        ),
    )
    parser.add_argument(
        "--threads",
        default="1,2,4,8",
        help="comma-separated thread counts for the --concurrency group",
    )
    parser.add_argument(
        "--workloads",
        default=",".join(CONCURRENCY_WORKLOADS),
        help=(
            "comma-separated workloads for the --concurrency group: "
            "io (request and body read only), io_cpu (adds repeated hashing), "
            "json (adds a JSON decode and summary, the application mix)"
        ),
    )
    args = parser.parse_args()
    thread_counts = [
        int(value) for value in args.threads.split(",") if value.strip()
    ]
    if not thread_counts:
        raise SystemExit("--threads needs at least one thread count")
    workloads = [value.strip() for value in args.workloads.split(",") if value.strip()]
    unknown = [value for value in workloads if value not in CONCURRENCY_WORKLOADS]
    if unknown:
        raise SystemExit(
            "unknown workload(s): %s (known: %s)"
            % (", ".join(unknown), ", ".join(CONCURRENCY_WORKLOADS))
        )
    if not workloads:
        raise SystemExit("--workloads needs at least one workload")
    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = Thread(target=server.serve_forever, daemon=True)
    thread.start()
    url = f"http://127.0.0.1:{server.server_port}"
    try:
        report = {
            "sync": sync_cases(url, args.repeats),
            "async": asyncio.run(async_cases(url, args.repeats)),
        }
        if args.concurrency:
            report["concurrency"] = concurrency_cases(
                args.repeats, thread_counts, workloads
            )
        print(json.dumps(report, indent=2))
    finally:
        server.shutdown()
        thread.join()


if __name__ == "__main__":
    main()
