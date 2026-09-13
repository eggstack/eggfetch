"""Async eggfetch example: AsyncClient with concurrent requests.

Run against the default demo endpoints::

    python examples/python_async.py

Or point at any compatible base URL (handy for a local stub)::

    python examples/python_async.py http://127.0.0.1:8000
"""

import asyncio
import sys

import eggfetch


async def main(base: str) -> None:
    async with eggfetch.AsyncClient() as client:
        print("=== GET request ===")
        r = await client.get(f"{base}/get")
        print("Status:", r.status_code)

        print("\n=== Concurrent requests ===")
        responses = await asyncio.gather(
            client.get(f"{base}/get"),
            client.get(f"{base}/ip"),
        )
        for resp in responses:
            print(resp.json())

        print("\n=== Streaming response ===")
        total = 0
        resp = await client.stream("GET", f"{base}/stream-bytes/10000")
        async for chunk in resp.aiter_bytes():
            total += len(chunk)
        print(f"Streamed {total} bytes")

    print("\nAll examples completed successfully!")


if __name__ == "__main__":
    base = sys.argv[1] if len(sys.argv) > 1 else "https://httpbin.org"
    asyncio.run(main(base))
