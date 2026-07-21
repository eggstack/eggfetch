#!/usr/bin/env python3
"""Generate a normalized JSON manifest of a Python package's public API.

Records: public top-level names, object kind, inspect.signature() output,
parameter details, class bases and MRO, public methods and properties,
module of origin, deprecation markers, and package version.

The manifest is stable across runs: memory addresses, object reprs,
platform-dependent paths, and ordering are normalized.
"""

import argparse
import importlib
import inspect
import json
import sys
import types
from pathlib import Path


def _safe_signature(obj):
    """Return a normalized signature dict, or None."""
    try:
        sig = inspect.signature(obj)
    except (ValueError, TypeError):
        return None
    params = []
    for name, param in sig.parameters.items():
        p = {"name": name, "kind": param.kind.name}
        if param.default is not inspect.Parameter.empty:
            p["default"] = repr(param.default)
        if param.annotation is not inspect.Parameter.empty:
            p["annotation"] = repr(param.annotation)
        params.append(p)
    return {"parameters": params, "return_annotation": repr(sig.return_annotation) if sig.return_annotation is not inspect.Parameter.empty else None}


def _get_kind(obj):
    """Classify a Python object into a manifest kind."""
    if inspect.ismodule(obj):
        return "module"
    if inspect.isclass(obj):
        if issubclass(obj, BaseException):
            return "exception"
        return "class"
    if inspect.isfunction(obj) or inspect.isbuiltin(obj):
        return "function"
    if inspect.ismethod(obj):
        return "method"
    if isinstance(obj, property):
        return "property"
    if isinstance(obj, (types.SimpleNamespace,)):
        return "object"
    if callable(obj):
        return "callable"
    return "constant"


def _get_bases_and_mro(obj):
    """Return normalized base class names and MRO names."""
    if not inspect.isclass(obj):
        return [], []
    bases = []
    for b in obj.__bases__:
        bases.append(b.__qualname__ if hasattr(b, "__qualname__") else b.__name__)
    mro = []
    for c in obj.__mro__:
        if c is object:
            continue
        mro.append(c.__qualname__ if hasattr(c, "__qualname__") else c.__name__)
    return bases, mro


def _get_public_methods_and_properties(cls):
    """Return public methods and properties of a class."""
    methods = []
    props = []
    if not inspect.isclass(cls):
        return methods, props
    for name in sorted(dir(cls)):
        if name.startswith("_"):
            continue
        try:
            attr = getattr(cls, name)
        except Exception:
            continue
        if isinstance(attr, property):
            sig = None
            if attr.fget:
                try:
                    sig = _safe_signature(attr.fget)
                except Exception:
                    pass
            props.append({"name": name, "signature": sig})
        elif callable(attr):
            sig = None
            try:
                sig = _safe_signature(attr)
            except Exception:
                pass
            methods.append({"name": name, "signature": sig})
    return methods, props


def _normalize_manifest(manifest):
    """Sort the manifest for stability."""
    manifest["symbols"] = sorted(manifest["symbols"], key=lambda s: s["name"])
    for sym in manifest["symbols"]:
        if "methods" in sym:
            sym["methods"] = sorted(sym["methods"], key=lambda m: m["name"])
        if "properties" in sym:
            sym["properties"] = sorted(sym["properties"], key=lambda p: p["name"])
        if "bases" in sym:
            sym["bases"] = sorted(sym["bases"])
        if "mro" in sym:
            sym["mro"] = sorted(sym["mro"])
    return manifest


def generate_manifest(package_name: str) -> dict:
    """Generate a manifest dict for the given package."""
    try:
        mod = importlib.import_module(package_name)
    except ImportError as e:
        print(f"Error: Cannot import '{package_name}': {e}", file=sys.stderr)
        sys.exit(1)

    version = getattr(mod, "__version__", "unknown")
    symbols = []

    public_names = getattr(mod, "__all__", None)
    if public_names is None:
        public_names = sorted(
            name for name in dir(mod)
            if not name.startswith("_") and name not in ("__all__", "__version__")
        )

    for name in public_names:
        try:
            obj = getattr(mod, name)
        except Exception:
            continue

        kind = _get_kind(obj)
        sig = None
        bases = []
        mro = []
        methods = []
        props = []
        module_of_origin = getattr(obj, "__module__", package_name)
        deprecated = False

        if kind in ("class", "exception"):
            sig = _safe_signature(obj.__init__) if hasattr(obj, "__init__") else None
            bases, mro = _get_bases_and_mro(obj)
            methods, props = _get_public_methods_and_properties(obj)
        elif kind == "function":
            sig = _safe_signature(obj)
        elif kind == "method":
            sig = _safe_signature(obj)

        # Check for deprecation
        if hasattr(obj, "__deprecated__"):
            deprecated = True
        if inspect.isclass(obj) and hasattr(obj, "deprecated"):
            try:
                deprecated = bool(getattr(obj, "deprecated"))
            except Exception:
                pass

        entry = {
            "name": name,
            "kind": kind,
            "signature": sig,
            "module_of_origin": module_of_origin,
            "deprecated": deprecated,
        }
        if bases:
            entry["bases"] = bases
        if mro:
            entry["mro"] = mro
        if methods:
            entry["methods"] = methods
        if props:
            entry["properties"] = props
        symbols.append(entry)

    manifest = {
        "schema_version": "1",
        "package": package_name,
        "version": version,
        "symbols": symbols,
    }
    return _normalize_manifest(manifest)


def main():
    parser = argparse.ArgumentParser(description="Generate a public API manifest for a Python package")
    parser.add_argument("--package", required=True, help="Python package to inspect")
    parser.add_argument("--output", required=True, help="Output JSON file path")
    args = parser.parse_args()

    manifest = generate_manifest(args.package)
    output_path = Path(args.output)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    with open(output_path, "w") as f:
        json.dump(manifest, f, indent=2, sort_keys=False)
        f.write("\n")
    print(f"Manifest written to {output_path} ({len(manifest['symbols'])} symbols)")


if __name__ == "__main__":
    main()
