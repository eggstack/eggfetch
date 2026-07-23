#!/usr/bin/env python3
"""Validate the downstream compatibility manifest.

Reads compat/downstream/manifest.toml, validates each entry structure,
attempts to import each listed package, and reports availability.
Outputs a JSON summary to stdout. Idempotent and safe to run without
packages installed (reports missing ones).
"""

import importlib
import json
import sys
import tomllib
from pathlib import Path

MANIFEST_PATH = Path(__file__).resolve().parent.parent / "compat" / "downstream" / "manifest.toml"

REQUIRED_FIELDS = {
    "name",
    "version",
    "license",
    "category",
    "rationale",
    "usage",
    "test-subset",
    "expected-network-isolation",
    "optional-dependencies",
    "known-incompatibilities",
    "update-owner",
    "review-cadence",
}

VALID_CATEGORIES = {
    "contract-tests",
    "mock-transport-user",
    "framework-test-client",
    "framework-asgi-transport",
    "sdk-async-client",
    "sdk-sync-client",
    "streaming-upload-download",
    "custom-transport-subclass",
    "async-testing-support",
    "custom-auth-flow",
    "event-hook-instrumentation",
    "heavy-config-user",
}


def validate_manifest(path: Path) -> dict:
    """Load and validate the manifest structure. Returns a result dict."""
    errors = []

    if not path.exists():
        return {"status": "error", "errors": [f"Manifest not found: {path}"], "packages": []}

    with open(path, "rb") as f:
        data = tomllib.load(f)

    portfolio = data.get("portfolio", {})
    if portfolio.get("schema-version") != "1":
        errors.append(f"portfolio.schema-version must be '1', got '{portfolio.get('schema-version')}'")
    if portfolio.get("status") != "phase-5":
        errors.append(f"portfolio.status must be 'phase-5', got '{portfolio.get('status')}'")

    ref_profile = portfolio.get("reference-profile", "")
    if ref_profile:
        ref_path = path.parent / ref_profile
        if not ref_path.exists():
            errors.append(f"reference-profile not found: {ref_path}")

    packages = data.get("package", [])
    if not packages:
        errors.append("No [[package]] entries found")

    seen_names = set()
    for pkg in packages:
        name = pkg.get("name", "<unnamed>")
        missing = REQUIRED_FIELDS - set(pkg.keys())
        if missing:
            errors.append(f"{name}: missing fields {missing}")
        if name in seen_names:
            errors.append(f"{name}: duplicate package name")
        seen_names.add(name)

        version = pkg.get("version", "")
        if not version or not any(c.isdigit() for c in str(version)):
            errors.append(f"{name}: version not pinned or invalid: '{version}'")

        cat = pkg.get("category", "")
        if cat not in VALID_CATEGORIES:
            errors.append(f"{name}: unknown category '{cat}'")

    return {
        "status": "valid" if not errors else "invalid",
        "errors": errors,
        "packages": packages,
    }


def check_imports(packages: list[dict]) -> list[dict]:
    """Attempt to import each package and report availability."""
    results = []
    for pkg in packages:
        name = pkg["name"]
        version = pkg.get("version", "unknown")
        installed = False
        installed_version = None
        import_error = None

        try:
            mod = importlib.import_module(name.replace("-", "_"))
            installed = True
            installed_version = getattr(mod, "__version__", None)
        except ImportError as exc:
            import_error = str(exc)

        results.append({
            "name": name,
            "expected_version": version,
            "installed": installed,
            "installed_version": installed_version,
            "import_error": import_error,
        })

    return results


def main() -> int:
    manifest_result = validate_manifest(MANIFEST_PATH)
    import_results = check_imports(manifest_result["packages"])

    summary = {
        "manifest_status": manifest_result["status"],
        "manifest_errors": manifest_result["errors"],
        "total_packages": len(manifest_result["packages"]),
        "installed_count": sum(1 for r in import_results if r["installed"]),
        "missing_count": sum(1 for r in import_results if not r["installed"]),
        "packages": import_results,
    }

    print(json.dumps(summary, indent=2))

    if manifest_result["status"] == "invalid":
        print(f"\nManifest validation failed with {len(manifest_result['errors'])} error(s)", file=sys.stderr)
        return 1

    missing = [r["name"] for r in import_results if not r["installed"]]
    if missing:
        print(f"\n{len(missing)} package(s) not installed: {', '.join(missing)}", file=sys.stderr)
        return 0

    print(f"\nAll {summary['total_packages']} packages installed and available.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
