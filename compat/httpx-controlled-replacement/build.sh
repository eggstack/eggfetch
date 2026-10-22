#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
# The runtime code is the shared replacement source in
# ../httpx-replacement-source, mapped in by pyproject.toml's package-dir.
# Set PYTHON to an interpreter that has `build` and `setuptools` available.
PYTHON="${PYTHON:-python3}"
exec "$PYTHON" -m build --wheel --outdir dist/
