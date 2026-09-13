"""Sync eggfetch example: client, JSON POST, and streaming download.

Run against the default demo endpoints::

    python examples/python_sync.py

Or point at any compatible base URL (handy for a local stub)::

    python examples/python_sync.py http://127.0.0.1:8000
"""

import sys

import eggfetch


def main(base: str) -> None:
    with eggfetch.Client(headers={"User-Agent": "eggfetch-example/0.1"}) as client:
        print("=== GET request ===")
        r = client.get(f"{base}/get")
        print("Status:", r.status_code)
        print("Body:", r.json())

        print("\n=== POST with JSON body ===")
        r = client.post(f"{base}/post", json={"key": "value"})
        print("Status:", r.status_code)

        print("\n=== Streaming response ===")
        total = 0
        with client.stream("GET", f"{base}/stream-bytes/10000") as r:
            for chunk in r.iter_bytes():
                total += len(chunk)
        print(f"Streamed {total} bytes")

    print("\nAll examples completed successfully!")


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "https://httpbin.org")
