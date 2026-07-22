"""Verify every symbol in __all__ is importable from eggfetch.compat.httpx."""

import importlib

from eggfetch.compat.httpx import __all__


def test_all_symbols_are_importable():
    """Every name in __all__ should be importable."""
    mod = importlib.import_module("eggfetch.compat.httpx")
    missing = []
    for name in __all__:
        if not hasattr(mod, name):
            missing.append(name)
    assert not missing, f"Missing symbols: {missing}"


def test_all_contains_expected_basics():
    """Smoke-check a known subset of __all__."""
    expected = {
        "Client",
        "AsyncClient",
        "Request",
        "Response",
        "URL",
        "QueryParams",
        "Headers",
        "Cookies",
        "Timeout",
        "Limits",
        "Proxy",
        "codes",
        "HTTPError",
        "HTTPStatusError",
        "RequestError",
        "TransportError",
        "TimeoutException",
        "get",
        "post",
        "put",
        "patch",
        "delete",
        "head",
        "options",
        "request",
        "stream",
    }
    missing = expected - set(__all__)
    assert not missing, f"__all__ missing expected symbols: {missing}"


def test_all_is_a_list():
    """__all__ should be a list (standard Python convention)."""
    assert isinstance(__all__, list)
