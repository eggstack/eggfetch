#!/usr/bin/env python3
"""Qualification-only local measurements for the performance campaign.

This harness intentionally uses a deterministic loopback server and is not
part of routine CI. It exercises the public Python API so a benchmark result
cannot silently depend on private extension details.
"""

from __future__ import annotations

import argparse
import asyncio
import json
import statistics
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from threading import Thread
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
    # Linux reports KiB; macOS reports bytes.
    return usage / (1024 if usage < 10_000_000 else 1024 * 1024)


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


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repeats", type=int, default=3)
    args = parser.parse_args()
    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = Thread(target=server.serve_forever, daemon=True)
    thread.start()
    url = f"http://127.0.0.1:{server.server_port}"
    try:
        print(json.dumps({"sync": sync_cases(url, args.repeats), "async": asyncio.run(async_cases(url, args.repeats))}, indent=2))
    finally:
        server.shutdown()
        thread.join()


if __name__ == "__main__":
    main()
