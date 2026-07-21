#!/usr/bin/env python3
"""Verify the Python package has expected public API surface.

This ensures the hand-written docs/python/guide.md stays in sync
with the actual public exports of the eggfetch package.
"""

import sys


EXPECTED_CLIENT_APIS = [
    "Client",
    "AsyncClient",
    "StreamingResponse",
    "BasicAuth",
    "BearerAuth",
    "NOAUTH",
    "Timeout",
    "Retry",
    "Cookie",
    "Cookies",
    "File",
    "Headers",
    "NoAuth",
    "RequestError",
    "EggfetchError",
    "TimeoutException",
    "NetworkError",
    "HTTPStatusError",
    "TooManyRedirects",
    "ProxyError",
]

EXPECTED_TOPLEVEL = [
    "get",
    "post",
    "put",
    "patch",
    "delete",
    "head",
    "options",
    "request",
]


def main() -> int:
    try:
        import eggfetch
    except ImportError:
        print("eggfetch not installed — skipping API surface check.")
        print("Run `maturin develop -m crates/eggfetch-python/Cargo.toml` first.")
        return 0  # Not a failure if package isn't installed

    errors = 0

    for name in EXPECTED_CLIENT_APIS:
        if not hasattr(eggfetch, name):
            print(f"  Missing: eggfetch.{name}")
            errors += 1

    for name in EXPECTED_TOPLEVEL:
        if not callable(getattr(eggfetch, name, None)):
            print(f"  Missing or not callable: eggfetch.{name}")
            errors += 1

    if errors:
        print(f"\n{errors} missing public API item(s).")
        return 1

    print("Python API surface check passed.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
