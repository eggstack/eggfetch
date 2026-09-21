#!/usr/bin/env python3
"""Check the supported native Python API against its reviewed manifest."""
from __future__ import annotations

import importlib.metadata
import inspect
import json
import argparse
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "crates/eggfetch-python/tests/native_api_manifest.json"


def _parameter_contract(value: object) -> list[tuple[str, str, bool]]:
    """Return the reviewed name/order/default contract for a callable."""
    result: list[tuple[str, str, bool]] = []
    for parameter in inspect.signature(value).parameters.values():
        if parameter.name == "self":
            continue
        result.append(
            (
                parameter.name,
                parameter.kind.name,
                parameter.default is not inspect.Parameter.empty,
            )
        )
    return result


def _without_extensions(contract: list[tuple[str, str, bool]]) -> list[tuple[str, str, bool]]:
    return [item for item in contract if item[0] != "extensions"]


def _check_relational_runtime_contracts(eggfetch: object) -> list[str]:
    """Check relationships that individual manifest rows cannot express."""
    errors: list[str] = []
    client = getattr(eggfetch, "Client")
    async_client = getattr(eggfetch, "AsyncClient")

    # Client and AsyncClient intentionally share one constructor contract.
    if _parameter_contract(client) != _parameter_contract(async_client):
        errors.append("Client and AsyncClient constructor parameters diverge")

    mirror_methods = ("request", "stream", "get", "post", "put", "patch", "delete", "head", "options")
    for method_name in mirror_methods:
        client_method = getattr(client, method_name)
        async_method = getattr(async_client, method_name)
        if _parameter_contract(client_method) != _parameter_contract(async_method):
            errors.append(f"Client/AsyncClient.{method_name} parameter contract diverges")

    # Top-level helpers intentionally omit the reusable-client-only
    # ``extensions`` hook.  All remaining names/order/defaults must match the
    # corresponding Client convenience method.
    for method_name in ("request", "get", "post", "put", "patch", "delete", "head", "options"):
        top_level = getattr(eggfetch, method_name)
        expected = [
            *(_without_extensions(_parameter_contract(getattr(client, method_name)))),
            ("limits", "KEYWORD_ONLY", True),
        ]
        if _parameter_contract(top_level) != expected:
            errors.append(f"top-level {method_name} is not the reviewed Client mirror")

    body_methods = ("request", "post", "put", "patch")
    body_names = {"content", "data", "json", "files"}
    for method_name in body_methods:
        names = {item[0] for item in _parameter_contract(getattr(eggfetch, method_name))}
        if not body_names.issubset(names):
            errors.append(f"top-level {method_name} lost a body-capable keyword")
    for method_name in ("get", "delete", "head", "options"):
        names = {item[0] for item in _parameter_contract(getattr(eggfetch, method_name))}
        if names & body_names:
            errors.append(f"top-level {method_name} unexpectedly accepts body keywords")
    return errors


def _run_relational_self_tests() -> None:
    """Exercise the relational runtime guardrails with fake surfaces.

    Each promised drift class must produce the reviewed checker error
    category, not merely a raw signature inequality.  The baseline fake
    surface passes; every mutation below must fail with its documented
    message.
    """
    import types

    def reviewed(*, value=None):
        return value

    def mutated(*, other=None):
        return other

    assert _parameter_contract(reviewed) != _parameter_contract(mutated)
    assert _without_extensions(
        [("method", "POSITIONAL_OR_KEYWORD", False), ("extensions", "KEYWORD_ONLY", True)]
    ) == [("method", "POSITIONAL_OR_KEYWORD", False)]

    def _make_func(signature: str, name: str):
        namespace: dict[str, object] = {}
        exec(f"def {name}{signature}:\n    pass", namespace)
        func = namespace[name]
        assert callable(func)
        return func

    _BODY_SIG = "(self, url, *, content=None, data=None, json=None, files=None, timeout=None, extensions=None)"
    _NOBODY_SIG = "(self, url, *, timeout=None, extensions=None)"
    _REQ_SIG = "(self, method, url, *, content=None, data=None, json=None, files=None, timeout=None, extensions=None)"
    _STREAM_SIG = "(self, method, url, *, content=None, timeout=None, extensions=None)"
    _CTOR_SIG = "(self, *, a=None, b=None)"
    _TOP_REQ_SIG = "(method, url, *, content=None, data=None, json=None, files=None, timeout=None, limits=None)"
    _TOP_BODY_SIG = "(url, *, content=None, data=None, json=None, files=None, timeout=None, limits=None)"
    _TOP_NOBODY_SIG = "(url, *, timeout=None, limits=None)"

    def _baseline_fake() -> object:
        namespace: dict[str, object] = {}
        exec(f"def __init__{_CTOR_SIG}:\n    pass", namespace)
        baseline_init = namespace["__init__"]
        client_dict: dict[str, object] = {"__init__": baseline_init}
        async_dict: dict[str, object] = {"__init__": baseline_init}
        for method_name, signature in (
            ("request", _REQ_SIG),
            ("stream", _STREAM_SIG),
        ):
            client_dict[method_name] = _make_func(signature, method_name)
            async_dict[method_name] = _make_func(signature, method_name)
        for method_name in ("post", "put", "patch"):
            client_dict[method_name] = _make_func(_BODY_SIG, method_name)
            async_dict[method_name] = _make_func(_BODY_SIG, method_name)
        for method_name in ("get", "delete", "head", "options"):
            client_dict[method_name] = _make_func(_NOBODY_SIG, method_name)
            async_dict[method_name] = _make_func(_NOBODY_SIG, method_name)
        fake_client = type("FakeClient", (), client_dict)  # type: ignore[arg-type]
        fake_async = type("FakeAsyncClient", (), async_dict)  # type: ignore[arg-type]
        top_level: dict[str, object] = {
            "request": _make_func(_TOP_REQ_SIG, "request"),
            "get": _make_func(_TOP_NOBODY_SIG, "get"),
            "post": _make_func(_TOP_BODY_SIG, "post"),
            "put": _make_func(_TOP_BODY_SIG, "put"),
            "patch": _make_func(_TOP_BODY_SIG, "patch"),
            "delete": _make_func(_TOP_NOBODY_SIG, "delete"),
            "head": _make_func(_TOP_NOBODY_SIG, "head"),
            "options": _make_func(_TOP_NOBODY_SIG, "options"),
        }
        return types.SimpleNamespace(
            Client=fake_client, AsyncClient=fake_async, **top_level
        )

    def _assert_error(fake: object, needle: str) -> None:
        errors = _check_relational_runtime_contracts(fake)
        assert any(needle in error for error in errors), (
            f"expected {needle!r} in {errors!r}"
        )

    # Baseline passes.
    assert _check_relational_runtime_contracts(_baseline_fake()) == []

    # 1. Client constructor gains/drops/reorders a keyword while AsyncClient does not.
    fake = _baseline_fake()
    fake.Client.__init__ = _make_func("(self, *, a=None, b=None, extra=None)", "__init__")  # type: ignore[attr-defined]
    _assert_error(fake, "Client and AsyncClient constructor parameters diverge")
    fake = _baseline_fake()
    fake.Client.__init__ = _make_func("(self, *, a=None)", "__init__")  # type: ignore[attr-defined]
    _assert_error(fake, "Client and AsyncClient constructor parameters diverge")
    fake = _baseline_fake()
    fake.Client.__init__ = _make_func("(self, *, b=None, a=None)", "__init__")  # type: ignore[attr-defined]
    _assert_error(fake, "Client and AsyncClient constructor parameters diverge")

    # 2. One sync/async mirror method changes a parameter name/order/default.
    fake = _baseline_fake()
    fake.Client.get = _make_func("(self, url, *, timeout=None, extensions=None, renamed=None)", "get")  # type: ignore[attr-defined]
    _assert_error(fake, "Client/AsyncClient.get parameter contract diverges")
    fake = _baseline_fake()
    fake.Client.post = _make_func(  # type: ignore[attr-defined]
        "(self, url, *, data=None, content=None, json=None, files=None, timeout=None, extensions=None)",
        "post",
    )
    _assert_error(fake, "Client/AsyncClient.post parameter contract diverges")
    fake = _baseline_fake()
    fake.Client.delete = _make_func("(self, url, *, timeout, extensions=None)", "delete")  # type: ignore[attr-defined]
    _assert_error(fake, "Client/AsyncClient.delete parameter contract diverges")

    # 3. One top-level helper gains a client-only keyword or loses the required limits relationship.
    fake = _baseline_fake()
    fake.get = _make_func("(url, *, timeout=None, extensions=None, limits=None)", "get")  # type: ignore[attr-defined]
    _assert_error(fake, "top-level get is not the reviewed Client mirror")
    fake = _baseline_fake()
    fake.post = _make_func(  # type: ignore[attr-defined]
        "(url, *, content=None, data=None, json=None, files=None, timeout=None)",
        "post",
    )
    _assert_error(fake, "top-level post is not the reviewed Client mirror")

    # 4. A bodyless convenience helper gains content/data/json/files.
    for extra in ("content", "data", "json", "files"):
        fake = _baseline_fake()
        fake.get = _make_func(  # type: ignore[attr-defined]
            f"(url, *, {extra}=None, timeout=None, limits=None)",
            "get",
        )
        _assert_error(fake, "top-level get unexpectedly accepts body keywords")

    # 5. A body-capable helper loses one of those keywords.
    for missing in ("content", "data", "json", "files"):
        remaining = [name for name in ("content", "data", "json", "files") if name != missing]
        params = ", ".join(f"{name}=None" for name in remaining)
        fake = _baseline_fake()
        fake.post = _make_func(  # type: ignore[attr-defined]
            f"(url, *, {params}, timeout=None, limits=None)",
            "post",
        )
        _assert_error(fake, "top-level post lost a body-capable keyword")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args(argv)
    if args.self_test:
        _run_relational_self_tests()
    import eggfetch
    from eggfetch import _native

    manifest = json.loads(MANIFEST.read_text())
    expected = manifest["exports"]
    errors: list[str] = []
    errors.extend(_check_relational_runtime_contracts(eggfetch))
    if list(eggfetch.__all__) != expected:
        errors.append("eggfetch.__all__ differs from the reviewed native manifest")
    if hasattr(_native, "__all__"):
        errors.append("eggfetch._native must not define an independent __all__")
    for name in expected:
        if not hasattr(eggfetch, name):
            errors.append(f"missing root export: {name}")

    for kind, names in manifest["symbol_kinds"].items():
        for name in names:
            value = getattr(eggfetch, name, None)
            if kind == "value" and value is None:
                errors.append(f"missing value export: {name}")
            elif kind == "singleton" and not isinstance(value, eggfetch.NoAuth):
                errors.append(f"singleton kind drift: {name}")
            elif kind == "function" and not inspect.isroutine(value):
                errors.append(f"function kind drift: {name}")
            elif kind == "class" and not inspect.isclass(value):
                errors.append(f"class kind drift: {name}")
            elif kind == "exception" and not inspect.isclass(value):
                errors.append(f"exception kind drift: {name}")

    for name, base in manifest["exception_bases"].items():
        value = getattr(eggfetch, name, None)
        if value is None or value.__bases__[0].__name__ != base:
            errors.append(f"exception base drift: {name} != {base}")

    for name, expected_mro in manifest["exception_mro"].items():
        value = getattr(eggfetch, name, None)
        actual_mro = [item.__name__ for item in value.__mro__] if value else []
        if actual_mro != expected_mro:
            errors.append(f"exception MRO drift: {name} = {actual_mro!r}")

    for name, expected_signature in manifest["signatures"].items():
        try:
            owner, separator, member = name.rpartition(".")
            value = getattr(getattr(eggfetch, owner), member) if separator else getattr(eggfetch, name)
            actual_signature = str(inspect.signature(value))
            if actual_signature != expected_signature:
                errors.append(
                    f"signature drift: {name} = {actual_signature!r}, expected {expected_signature!r}"
                )
        except (AttributeError, TypeError, ValueError) as error:
            errors.append(f"signature unavailable for {name}: {error}")

    # The typing contract extends the runtime manifest with the complete
    # reviewed member inventory.  Keep this check live as well so a PyO3
    # getter or method cannot disappear while the stubs still pass their
    # static-only checks.
    for class_name, contract in manifest.get("members", {}).items():
        cls = getattr(eggfetch, class_name, None)
        if cls is None:
            errors.append(f"member contract owner missing: {class_name}")
            continue
        for property_name in contract.get("properties", {}):
            if not hasattr(cls, property_name):
                errors.append(f"runtime property missing: {class_name}.{property_name}")
        for method_name, method_contract in contract.get("methods", {}).items():
            try:
                value = getattr(cls, method_name)
                if not callable(value):
                    errors.append(f"runtime member is not callable: {class_name}.{method_name}")
                actual_signature = str(inspect.signature(value))
                if actual_signature != method_contract["signature"]:
                    errors.append(
                        f"member signature drift: {class_name}.{method_name} = "
                        f"{actual_signature!r}, expected {method_contract['signature']!r}"
                    )
            except (AttributeError, TypeError, ValueError) as error:
                errors.append(f"runtime member unavailable: {class_name}.{method_name}: {error}")

    # The re-exported native types remain discoverable from the supported
    # package even though PyO3 reports their implementation module as builtins.
    for name in manifest["symbol_kinds"]["class"]:
        if not getattr(eggfetch, name).__module__:
            errors.append(f"class module metadata missing: {name}")

    try:
        installed = importlib.metadata.version("eggfetch")
    except importlib.metadata.PackageNotFoundError:
        installed = None
    if installed is not None and eggfetch.__version__ != installed:
        errors.append(f"runtime version {eggfetch.__version__!r} != installed {installed!r}")
    if errors:
        for error in errors:
            print(f"FAIL: {error}")
        return 1
    print(f"Native API manifest passed ({len(expected)} exports)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
