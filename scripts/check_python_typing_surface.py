#!/usr/bin/env python3
"""Check the reviewed native PEP 561 surface against its runtime manifest.

This is intentionally structural rather than a second signature oracle.  The
runtime API checker owns runtime signatures; this check proves that the
reviewed stubs represent every supported export, preserve the exception base
classes, and do not accidentally expose an unreviewed public stub-only name.
"""

from __future__ import annotations

import ast
import json
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
PACKAGE = ROOT / "crates/eggfetch-python/python/eggfetch"
MANIFEST = ROOT / "crates/eggfetch-python/tests/native_api_manifest.json"
REQUIRED_FILES = ("py.typed", "__init__.pyi", "_native.pyi")
INTENTIONAL_TYPING_ONLY = {"Auth", "AsyncBody", "BytesLike", "Cert", "HeadersInput", "SyncBody", "Verify"}


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


def main() -> int:
    manifest = json.loads(MANIFEST.read_text())
    expected = set(manifest["exports"])
    errors: list[str] = []

    for filename in REQUIRED_FILES:
        if not (PACKAGE / filename).is_file():
            errors.append(f"missing typing file: eggfetch/{filename}")

    try:
        init_tree = ast.parse((PACKAGE / "__init__.pyi").read_text(), filename="__init__.pyi")
        native_tree = ast.parse((PACKAGE / "_native.pyi").read_text(), filename="_native.pyi")
    except (FileNotFoundError, SyntaxError) as error:
        errors.append(f"stub syntax error: {error}")
        init_tree = ast.Module(body=[])
        native_tree = ast.Module(body=[])

    declarations = _public_declarations(native_tree)
    # ``__version__`` is intentionally a dunder value but is part of the
    # reviewed public manifest and is explicitly imported by __init__.pyi.
    if any(
        isinstance(node, (ast.Assign, ast.AnnAssign))
        and any(
            isinstance(target, ast.Name) and target.id == "__version__"
            for target in (node.targets if isinstance(node, ast.Assign) else [node.target])
        )
        for node in native_tree.body
    ):
        declarations["__version__"] = next(
            node
            for node in native_tree.body
            if isinstance(node, (ast.Assign, ast.AnnAssign))
            and any(
                isinstance(target, ast.Name) and target.id == "__version__"
                for target in (node.targets if isinstance(node, ast.Assign) else [node.target])
            )
        )
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

    if errors:
        for error in errors:
            print(f"FAIL: {error}")
        return 1

    print(
        "Python typing surface passed "
        f"({len(expected)} runtime exports, {len(manifest['exception_bases'])} exception bases)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
