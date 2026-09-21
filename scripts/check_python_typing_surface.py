#!/usr/bin/env python3
"""Check the reviewed native PEP 561 surface against its runtime manifest.

The native API checker owns live PyO3 signatures.  This checker complements it
with the reviewed Python typing contract: class members, sync/async shape,
signature structure, and semantic annotations that runtime signatures cannot
encode (for example the awaited result of ``start_tls``).
"""

from __future__ import annotations

import ast
import argparse
import json
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
PACKAGE = ROOT / "crates/eggfetch-python/python/eggfetch"
MANIFEST = ROOT / "crates/eggfetch-python/tests/native_api_manifest.json"
REQUIRED_FILES = ("py.typed", "__init__.pyi", "_native.pyi")
INTENTIONAL_TYPING_ONLY = {
    "Auth", "AsyncBody", "BytesLike", "Cert", "HeadersInput", "SyncBody", "Verify"
}


def _public_declarations(tree: ast.Module) -> dict[str, ast.AST]:
    declarations: dict[str, ast.AST] = {}
    for node in tree.body:
        if isinstance(node, (ast.ClassDef, ast.FunctionDef, ast.AsyncFunctionDef)):
            if not node.name.startswith("_"):
                declarations[node.name] = node
        elif isinstance(node, (ast.Assign, ast.AnnAssign)):
            targets = node.targets if isinstance(node, ast.Assign) else [node.target]
            for target in targets:
                if isinstance(target, ast.Name) and not target.id.startswith("_"):
                    declarations[target.id] = node
    return declarations


def _native_imports(tree: ast.Module) -> set[str]:
    names: set[str] = set()
    for node in tree.body:
        if isinstance(node, ast.ImportFrom) and node.module == "_native":
            names.update(alias.name for alias in node.names if alias.name != "*")
    return names


def _declared_all(tree: ast.Module) -> list[str] | None:
    for node in tree.body:
        if isinstance(node, ast.Assign):
            if any(isinstance(target, ast.Name) and target.id == "__all__" for target in node.targets):
                value = ast.literal_eval(node.value)
                if isinstance(value, list) and all(isinstance(item, str) for item in value):
                    return value
    return None


def _base_name(node: ast.ClassDef) -> str | None:
    if not node.bases:
        return None
    base = node.bases[0]
    if isinstance(base, ast.Name):
        return base.id
    if isinstance(base, ast.Attribute):
        return base.attr
    return None


def _class_members(node: ast.ClassDef) -> dict[str, tuple[str, ast.AST]]:
    members: dict[str, tuple[str, ast.AST]] = {}
    for child in node.body:
        if isinstance(child, (ast.FunctionDef, ast.AsyncFunctionDef)):
            kind = "property" if any(
                isinstance(decorator, ast.Name) and decorator.id == "property"
                for decorator in child.decorator_list
            ) else "method"
            # Keep the first overload as the representative runtime shape;
            # overloads are one typed member, not duplicate runtime members.
            members.setdefault(child.name, (kind, child))
        elif isinstance(child, ast.AnnAssign) and isinstance(child.target, ast.Name):
            members[child.target.id] = ("property", child)
        elif isinstance(child, ast.Assign):
            for target in child.targets:
                if isinstance(target, ast.Name):
                    members[target.id] = ("property", child)
    return members


def _annotation_dump(annotation: ast.expr | None) -> str | None:
    if annotation is None:
        return None
    return ast.dump(annotation, annotate_fields=False, include_attributes=False)


def _signature_shape_from_ast(node: ast.FunctionDef | ast.AsyncFunctionDef) -> list[tuple[str, str, bool]]:
    args = node.args
    positional = [*args.posonlyargs, *args.args]
    positional_defaults = [False] * (len(positional) - len(args.defaults)) + [True] * len(args.defaults)
    shape = [("positional", arg.arg, has_default) for arg, has_default in zip(positional, positional_defaults)]
    if args.vararg is not None:
        shape.append(("vararg", args.vararg.arg, False))
    shape.extend(("keyword-only", arg.arg, default is not None) for arg, default in zip(args.kwonlyargs, args.kw_defaults))
    if args.kwarg is not None:
        shape.append(("kwarg", args.kwarg.arg, False))
    return shape


def _signature_shape_from_text(signature: str) -> list[tuple[str, str, bool]]:
    tree = ast.parse(f"def _signature{signature}: pass")
    function = tree.body[0]
    assert isinstance(function, ast.FunctionDef)
    return _signature_shape_from_ast(function)


def _without_self(shape: list[tuple[str, str, bool]]) -> list[tuple[str, str, bool]]:
    if shape and shape[0][1] == "self":
        return shape[1:]
    return shape


def _stub_signature_matches(node: ast.FunctionDef | ast.AsyncFunctionDef, signature: str) -> bool:
    return _without_self(_signature_shape_from_ast(node)) == _without_self(_signature_shape_from_text(signature))


def _find_method(class_node: ast.ClassDef, name: str) -> ast.FunctionDef | ast.AsyncFunctionDef | None:
    member = _class_members(class_node).get(name)
    if member and member[0] == "method":
        value = member[1]
        assert isinstance(value, (ast.FunctionDef, ast.AsyncFunctionDef))
        return value
    return None


def _find_signature_node(
    native_declarations: dict[str, ast.AST], symbol: str
) -> ast.FunctionDef | ast.AsyncFunctionDef | None:
    owner, separator, member = symbol.rpartition(".")
    if separator:
        class_node = native_declarations.get(owner)
        if isinstance(class_node, ast.ClassDef):
            return _find_method(class_node, member)
        return None
    value = native_declarations.get(symbol)
    if isinstance(value, (ast.FunctionDef, ast.AsyncFunctionDef)):
        return value
    if isinstance(value, ast.ClassDef):
        return _find_method(value, "__init__")
    return None


_MIRROR_METHODS = ("request", "stream", "get", "post", "put", "patch", "delete", "head", "options")
_TOP_LEVEL_METHODS = ("request", "get", "post", "put", "patch", "delete", "head", "options")
_BODY_METHODS = {"request", "post", "put", "patch"}
_BODY_PARAMETERS = {"content", "data", "json", "files"}


def _node_shape_without_self(node: ast.FunctionDef | ast.AsyncFunctionDef) -> list[tuple[str, str, bool]]:
    return _without_self(_signature_shape_from_ast(node))


def _drop_parameter(shape: list[tuple[str, str, bool]], name: str) -> list[tuple[str, str, bool]]:
    return [item for item in shape if item[1] != name]


def _find_top_level_function(
    declarations: dict[str, ast.AST], name: str
) -> ast.FunctionDef | ast.AsyncFunctionDef | None:
    value = declarations.get(name)
    return value if isinstance(value, (ast.FunctionDef, ast.AsyncFunctionDef)) else None


def _annotation_text(node: ast.FunctionDef | ast.AsyncFunctionDef, parameter_name: str) -> str | None:
    for argument in [*node.args.posonlyargs, *node.args.args, *node.args.kwonlyargs]:
        if argument.arg == parameter_name and argument.annotation is not None:
            return ast.unparse(argument.annotation)
    return None


def _check_relational_stub_contracts(declarations: dict[str, ast.AST], errors: list[str]) -> None:
    """Check sync/async/top-level relationships in the reviewed PEP 561 stub."""
    client = declarations.get("Client")
    async_client = declarations.get("AsyncClient")
    if not isinstance(client, ast.ClassDef) or not isinstance(async_client, ast.ClassDef):
        # Small focused fixtures may review an isolated native class.  Apply
        # the relational guardrail only when the reviewed package exposes the
        # paired client surface; a one-sided declaration is still drift.
        if client is not None or async_client is not None:
            errors.append("Client/AsyncClient class declarations are incomplete")
        return

    client_init = _find_method(client, "__init__")
    async_init = _find_method(async_client, "__init__")
    if client_init is None or async_init is None:
        errors.append("Client/AsyncClient constructor declarations are missing")
    elif _node_shape_without_self(client_init) != _node_shape_without_self(async_init):
        errors.append("Client and AsyncClient constructor typing shapes diverge")

    for method_name in _MIRROR_METHODS:
        sync_node = _find_method(client, method_name)
        async_node = _find_method(async_client, method_name)
        if sync_node is None or async_node is None:
            errors.append(f"missing Client/AsyncClient mirror method: {method_name}")
            continue
        if not isinstance(async_node, ast.AsyncFunctionDef) or isinstance(sync_node, ast.AsyncFunctionDef):
            errors.append(f"sync/async method kind drift: {method_name}")
        if _node_shape_without_self(sync_node) != _node_shape_without_self(async_node):
            errors.append(f"Client/AsyncClient typing shape diverges: {method_name}")
        sync_content = _annotation_text(sync_node, "content") or ""
        async_content = _annotation_text(async_node, "content") or ""
        if "AsyncBody" in sync_content or method_name not in _BODY_METHODS and "content" in sync_content:
            errors.append(f"sync {method_name} unexpectedly accepts AsyncBody")
        if method_name in _BODY_METHODS and "AsyncBody" not in async_content:
            errors.append(f"async {method_name} lost AsyncBody typing")

        if method_name not in _TOP_LEVEL_METHODS:
            continue
        top_level = _find_top_level_function(declarations, method_name)
        if top_level is None:
            errors.append(f"missing top-level helper declaration: {method_name}")
            continue
        expected = [
            *_drop_parameter(_node_shape_without_self(sync_node), "extensions"),
            ("keyword-only", "limits", True),
        ]
        if _node_shape_without_self(top_level) != expected:
            errors.append(f"top-level helper typing shape diverges: {method_name}")
        top_names = {item[1] for item in _node_shape_without_self(top_level)}
        if method_name in _BODY_METHODS and not _BODY_PARAMETERS.issubset(top_names):
            errors.append(f"top-level {method_name} lost body-capable typing")
        if method_name not in _BODY_METHODS and top_names & _BODY_PARAMETERS:
            errors.append(f"top-level {method_name} unexpectedly has body-capable typing")


def _run_relational_self_tests() -> None:
    """Prove the mutation-sensitive shape helpers detect representative drift."""
    assert _signature_shape_from_text("(*, value=None)") != _signature_shape_from_text(
        "(*, other=None)"
    )
    assert _drop_parameter(
        [("keyword-only", "value", True), ("keyword-only", "extensions", True)], "extensions"
    ) == [("keyword-only", "value", True)]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", type=Path, default=PACKAGE)
    parser.add_argument("--manifest", type=Path, default=MANIFEST)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args(argv)
    if args.self_test:
        _run_relational_self_tests()
    package = args.package
    manifest = json.loads(args.manifest.read_text())
    expected = set(manifest["exports"])
    errors: list[str] = []

    for filename in REQUIRED_FILES:
        if not (package / filename).is_file():
            errors.append(f"missing typing file: eggfetch/{filename}")

    try:
        init_tree = ast.parse((package / "__init__.pyi").read_text(), filename="__init__.pyi")
        native_tree = ast.parse((package / "_native.pyi").read_text(), filename="_native.pyi")
    except (FileNotFoundError, SyntaxError) as error:
        errors.append(f"stub syntax error: {error}")
        init_tree = ast.Module(body=[])
        native_tree = ast.Module(body=[])

    declarations = _public_declarations(native_tree)
    _check_relational_stub_contracts(declarations, errors)
    version_node = next(
        (
            node for node in native_tree.body
            if isinstance(node, (ast.Assign, ast.AnnAssign))
            and any(
                isinstance(target, ast.Name) and target.id == "__version__"
                for target in (node.targets if isinstance(node, ast.Assign) else [node.target])
            )
        ),
        None,
    )
    if version_node is not None:
        declarations["__version__"] = version_node

    imported = _native_imports(init_tree)
    missing_from_stub = expected - declarations.keys()
    missing_from_package = expected - imported
    if missing_from_stub:
        errors.append(f"runtime exports absent from _native.pyi: {sorted(missing_from_stub)}")
    if missing_from_package:
        errors.append(f"runtime exports absent from __init__.pyi: {sorted(missing_from_package)}")
    declared_all = _declared_all(init_tree)
    if declared_all != manifest["exports"]:
        errors.append("__init__.pyi __all__ differs from the reviewed manifest")

    stub_only = set(declarations) - expected - INTENTIONAL_TYPING_ONLY
    if stub_only:
        errors.append(f"unreviewed public stub-only names: {sorted(stub_only)}")

    for name, base in manifest["exception_bases"].items():
        node = declarations.get(name)
        if not isinstance(node, ast.ClassDef) or _base_name(node) != base:
            actual = _base_name(node) if isinstance(node, ast.ClassDef) else None
            errors.append(f"stub exception base drift: {name} = {actual!r}, expected {base!r}")

    members_manifest = manifest.get("members", {})
    custom_classes = set(manifest["symbol_kinds"]["class"]) - set(manifest["symbol_kinds"]["exception"])
    for class_name in sorted(custom_classes):
        class_node = declarations.get(class_name)
        contract = members_manifest.get(class_name)
        if not isinstance(class_node, ast.ClassDef):
            continue
        if contract is None:
            errors.append(f"missing reviewed member contract: {class_name}")
            continue
        stub_members = _class_members(class_node)
        expected_properties = contract.get("properties", {})
        expected_methods = contract.get("methods", {})
        allowed = set(contract.get("typing_only_members", []))
        expected_names = set(expected_properties) | set(expected_methods)
        for property_name, expected_type in expected_properties.items():
            member = stub_members.get(property_name)
            if member is None or member[0] != "property":
                errors.append(f"missing typed property: {class_name}.{property_name}")
                continue
            node = member[1]
            annotation = node.annotation if isinstance(node, ast.AnnAssign) else getattr(node, "returns", None)
            if _annotation_dump(annotation) != _annotation_dump(ast.parse(expected_type, mode="eval").body):
                actual = ast.unparse(annotation) if annotation is not None else None
                errors.append(
                    f"property annotation drift: {class_name}.{property_name} = {actual!r}, expected {expected_type!r}"
                )
        for method_name, method_contract in expected_methods.items():
            node = _find_method(class_node, method_name)
            if node is None:
                errors.append(f"missing typed method: {class_name}.{method_name}")
                continue
            expected_kind = method_contract["kind"]
            actual_kind = "async" if isinstance(node, ast.AsyncFunctionDef) else "sync"
            if actual_kind != expected_kind:
                errors.append(f"method kind drift: {class_name}.{method_name} = {actual_kind}, expected {expected_kind}")
            if not _stub_signature_matches(node, method_contract["signature"]):
                errors.append(f"method signature shape drift: {class_name}.{method_name}")
        unexpected = set(stub_members) - expected_names - {"__init__"} - allowed
        if unexpected:
            errors.append(f"unreviewed public stub-only members on {class_name}: {sorted(unexpected)}")

    for symbol, expected_signature in manifest["signatures"].items():
        node = _find_signature_node(declarations, symbol)
        if node is None:
            errors.append(f"stub signature unavailable: {symbol}")
        elif not _stub_signature_matches(node, expected_signature):
            errors.append(f"stub signature shape drift: {symbol}")

    for symbol, expected_return in manifest.get("semantic_contracts", {}).items():
        owner, _, member_name = symbol.rpartition(".")
        class_node = declarations.get(owner)
        member = _class_members(class_node).get(member_name) if isinstance(class_node, ast.ClassDef) else None
        if member is None:
            errors.append(f"semantic contract member missing: {symbol}")
            continue
        node = member[1]
        annotation = node.annotation if isinstance(node, ast.AnnAssign) else getattr(node, "returns", None)
        actual = ast.unparse(annotation) if annotation is not None else None
        if _annotation_dump(annotation) != _annotation_dump(ast.parse(expected_return, mode="eval").body):
            errors.append(f"semantic return/property drift: {symbol} = {actual!r}, expected {expected_return!r}")

    if errors:
        for error in errors:
            print(f"FAIL: {error}")
        return 1

    print(
        "Python typing surface passed "
        f"({len(expected)} runtime exports, {len(manifest['exception_bases'])} exception bases, "
        f"{len(members_manifest)} reviewed member contracts)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
