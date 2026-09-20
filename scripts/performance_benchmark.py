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

import eggfetch


BODY = b"0123456789abcdef" * (64 * 1024)  # 1 MiB, split into large frames.


class Handler(BaseHTTPRequestHandler):
    def do_GET(self) -> None:  # noqa: N802 - stdlib protocol hook
        self.send_response(200)
        self.send_header("Content-Length", str(len(BODY)))
        self.send_header("Content-Type", "application/octet-stream")
        self.end_headers()
        for offset in range(0, len(BODY), 64 * 1024):
            self.wfile.write(BODY[offset : offset + 64 * 1024])

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

        return {
            "sync_iter_bytes_1k": measure(stream_small_chunks, repeats),
            "sync_iter_lines": measure(stream_lines, repeats),
            "buffered_without_text": measure(buffered_without_text, repeats),
            "buffered_first_text": measure(buffered_with_text, repeats),
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

        return {"async_iter_bytes_1k": await measure_async(stream_small_chunks, repeats)}


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
    url = f"http://127.0.0.1:{server.server_port}/body"
    try:
        print(json.dumps({"sync": sync_cases(url, args.repeats), "async": asyncio.run(async_cases(url, args.repeats))}, indent=2))
    finally:
        server.shutdown()
        thread.join()


if __name__ == "__main__":
    main()
