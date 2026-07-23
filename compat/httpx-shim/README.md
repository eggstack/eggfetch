# httpx-eggfetch-shim

Compatibility shim that provides `import httpx` backed by eggfetch's HTTPX-compatible layer.

## Purpose

Some downstream packages import `httpx` directly and cannot be modified. This shim allows them to run against eggfetch without code changes.

## Installation

```bash
# Remove the real httpx first
pip uninstall httpx httpcore

# Install the shim
pip install httpx-eggfetch-shim
```

## Conflict Declaration

This package declares a conflict with the real `httpx` package. pip will refuse to install both simultaneously.

## Uninstall

```bash
pip uninstall httpx-eggfetch-shim
# Then reinstall real httpx if needed
pip install httpx==0.28.1
```
