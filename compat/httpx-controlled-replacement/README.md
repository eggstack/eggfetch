# httpx Controlled Replacement Artifact

This package provides a local-only wheel with distribution name `httpx`
(version 0.28.1) backed by `eggfetch.compat.httpx`. It is used for Stage C
qualification testing and must **never** be published to public PyPI.

## Shared source

The runtime code lives once, in
[`../httpx-replacement-source/httpx/__init__.py`](../httpx-replacement-source/httpx/__init__.py).
This package maps its `httpx` package at that file through setuptools'
`package-dir` setting in `pyproject.toml`, so there is no second copy of the
replacement code in this source tree.

The convenience distribution `httpx-eggfetch-shim`
([`../httpx-shim/`](../httpx-shim/)) packages the same shared source, so the
two distributions differ only in their distribution metadata. Both set
`__eggfetch_shim__ = True`, which is the identity marker the downstream
qualification path (`scripts/run_isolated_downstream.py`) requires in order to
prove that `httpx` resolved to the eggfetch-backed package.

Because the shared source lives outside this project directory, build a
**wheel** (not an sdist):

```bash
PYTHON=path/to/python ./build.sh
```

## Guards

- `check_no_publish.py` — fails if any workflow under `.github/workflows/`
  would publish this artifact. Run it after changing the workflows.
- `verify_identity.py` — verifies the *installed* distribution is this
  controlled replacement (distribution name `httpx`, version `0.28.1`,
  `httpx.__eggfetch_shim__` true, `Client`/`AsyncClient` resolved from
  eggfetch). Run it in the environment where the wheel is installed.
