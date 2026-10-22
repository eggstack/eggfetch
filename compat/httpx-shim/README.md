# httpx-eggfetch-shim

Compatibility shim that provides `import httpx` backed by eggfetch's HTTPX-compatible layer.

## Purpose

Some downstream packages import `httpx` directly and cannot be modified. This shim allows them to run against eggfetch without code changes.

## Shared source

The runtime code lives once, in
[`../httpx-replacement-source/httpx/__init__.py`](../httpx-replacement-source/httpx/__init__.py).
This package maps its `httpx` package at that file through setuptools'
`package-dir` setting in `pyproject.toml`, so there is no second copy of the
replacement code in this source tree.

The controlled replacement artifact
([`../httpx-controlled-replacement/`](../httpx-controlled-replacement/)) packages
the same shared source, so the two distributions differ only in their
distribution metadata. Both set `__eggfetch_shim__ = True`, which is the
identity marker the downstream qualification path
(`scripts/run_isolated_downstream.py`) requires in order to prove that `httpx`
resolved to the eggfetch-backed package.

Because the shared source lives outside this project directory, build a
**wheel** (not an sdist):

```bash
python -m build --wheel --outdir dist/
```

## Installation

```bash
# Remove the real httpx first
pip uninstall httpx httpcore

# Install the shim
pip install httpx-eggfetch-shim
```

## Conflict Declaration

This package does **not** currently declare a `Conflicts` entry against the
real `httpx` package, so pip will not refuse to install both simultaneously.
Uninstall the real `httpx` before installing the shim, as shown above.

## Uninstall

```bash
pip uninstall httpx-eggfetch-shim
# Then reinstall real httpx if needed
pip install httpx==0.28.1
```
