"""Direct tests for the supported native ``eggfetch`` package surface."""

import importlib.metadata
import json
from pathlib import Path

import eggfetch
from eggfetch import _native


MANIFEST = Path(__file__).with_name("native_api_manifest.json")


def test_root_exports_are_complete_and_import_star_is_bounded() -> None:
    manifest = json.loads(MANIFEST.read_text())
    expected = manifest["exports"]

    assert isinstance(eggfetch.__all__, list)
    assert list(eggfetch.__all__) == expected
    assert all(hasattr(eggfetch, name) for name in expected)

    namespace: dict[str, object] = {}
    exec("from eggfetch import *", {}, namespace)
    assert set(namespace) == set(expected)


def test_native_module_has_no_second_export_contract() -> None:
    assert not hasattr(_native, "__all__")

    # An implementation-only registration on the private extension must not
    # become a public root export without an explicit __init__.py change.
    _native._test_implementation_extra = object()
    try:
        assert not hasattr(eggfetch, "_test_implementation_extra")
    finally:
        del _native._test_implementation_extra


def test_documented_exception_hierarchy_and_mro() -> None:
    manifest = json.loads(MANIFEST.read_text())
    for name, expected_mro in manifest["exception_mro"].items():
        exception = getattr(eggfetch, name)
        assert exception.__module__ == "eggfetch"
        assert [item.__name__ for item in exception.__mro__] == expected_mro


def test_native_runtime_version_matches_installed_distribution() -> None:
    assert eggfetch.__version__ == importlib.metadata.version("eggfetch")


def test_public_classes_are_discoverable() -> None:
    manifest = json.loads(MANIFEST.read_text())
    for name in manifest["symbol_kinds"]["class"]:
        value = getattr(eggfetch, name)
        assert isinstance(value, type)
        assert value.__module__
