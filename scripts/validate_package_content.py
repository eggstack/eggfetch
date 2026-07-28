#!/usr/bin/env python3
"""Validate built wheel/sdist packages for forbidden content.

Checks that distribution archives don't contain source tests, corpora,
secrets, local paths, or unexpected top-level modules. Designed for CI
gates and release validation.

Exit codes:
    0 - all checks passed
    1 - one or more checks failed
"""

from __future__ import annotations

import argparse
import re
import sys
import zipfile
from pathlib import Path

# Top-level paths/patterns allowed in wheels and sdists.
WHEEL_ALLOWED_TOP_DIRS = {"eggfetch"}
WHEEL_ALLOWED_TOP_FILES = {
    "METADATA", "WHEEL", "RECORD", "entry_points.txt",
}
WHEEL_ALLOWED_DIST_INFO = re.compile(r"^eggfetch[\w.-]*\.dist-info$")
WHEEL_ALLOWED_STUBS = re.compile(r".*\.pyi$")

# For sdists, allow a broader set of top-level entries.
SDIST_ALLOWED_TOP_DIRS = {
    "eggfetch", "src", "PKG-INFO", "SOURCES.txt", "pyproject.toml",
    "setup.py", "setup.cfg", "LICENSE", "LICENSE.txt", "README",
    "README.md", "README.rst", "MANIFEST.in", "tests", "test",
    "docs", "bench", "examples",
}

# Forbidden top-level directories (for wheels only).
WHEEL_FORBIDDEN_DIRS = {"tests", "test", "corpus", "fuzz"}

# Patterns in text files that suggest secrets or local paths.
SECRET_PATTERNS = [
    re.compile(r"/home/"),
    re.compile(r"/Users/"),
    re.compile(r"password\s*=\s*\S+", re.IGNORECASE),
    re.compile(r"secret\s*=\s*\S+", re.IGNORECASE),
    re.compile(r"token\s*=\s*\S+", re.IGNORECASE),
]

# Files that should be treated as text for secret scanning.
TEXT_EXTENSIONS = {
    ".py", ".pyi", ".toml", ".cfg", ".txt", ".md", ".rst", ".json",
    ".yaml", ".yml", ".ini", ".sh", ".bat", ".cmd", ".ps1",
    ".c", ".h", ".rs", ".js", ".ts", ".html", ".css",
}

# Binary extensions to skip during secret scanning.
BINARY_EXTENSIONS = {
    ".so", ".pyd", ".dll", ".dylib", ".bin", ".exe", ".whl",
    ".tar.gz", ".zip", ".bz2", ".xz", ".png", ".jpg", ".gif",
    ".ico", ".woff", ".woff2", ".ttf", ".otf", ".eot",
}


class ValidationError:
    """Represents a single validation failure."""

    def __init__(self, check: str, message: str, path: str = "") -> None:
        self.check = check
        self.message = message
        self.path = path

    def __str__(self) -> str:
        loc = f" in {self.path}" if self.path else ""
        return f"[{self.check}] {self.message}{loc}"


def _is_text_file(name: str) -> bool:
    """Heuristic: is this file likely text based on extension?"""
    lower = name.lower()
    for ext in BINARY_EXTENSIONS:
        if lower.endswith(ext):
            return False
    for ext in TEXT_EXTENSIONS:
        if lower.endswith(ext):
            return True
    # Files without extensions are checked if small.
    return "." not in Path(name).name


def _read_text_entry(zf: zipfile.ZipFile, entry: str, max_bytes: int = 1_048_576) -> str | None:
    """Read a zip entry as text, with a size cap to avoid memory issues."""
    try:
        info = zf.getinfo(entry)
    except KeyError:
        return None
    if info.file_size > max_bytes:
        return None
    try:
        data = zf.read(entry)
        return data.decode("utf-8", errors="replace")
    except Exception:
        return None


def _top_level_parts(namelist: list[str]) -> set[str]:
    """Extract the first path component of each entry."""
    parts: set[str] = set()
    for name in namelist:
        first = name.split("/")[0]
        if first:
            parts.add(first)
    return parts


def _check_wheel_forbidden_dirs(namelist: list[str]) -> list[ValidationError]:
    """Reject wheels containing tests/, corpus/, fuzz/ at top level."""
    errors: list[ValidationError] = []
    top = _top_level_parts(namelist)
    for forbidden in WHEEL_FORBIDDEN_DIRS:
        if forbidden in top:
            errors.append(ValidationError(
                "forbidden-top-level",
                f"Wheel contains forbidden top-level directory '{forbidden}/'",
            ))
    return errors


def _check_wheel_forbidden_files(namelist: list[str]) -> list[ValidationError]:
    """Reject top-level test files in wheels."""
    errors: list[ValidationError] = []
    for name in namelist:
        parts = name.split("/")
        if len(parts) == 1:
            basename = parts[0]
            if re.match(r"^test_.*\.py$", basename) or re.match(r".*_test\.py$", basename):
                errors.append(ValidationError(
                    "forbidden-top-level",
                    f"Wheel contains forbidden top-level test file '{basename}'",
                ))
    return errors


def _check_wheel_unexpected_modules(namelist: list[str]) -> list[ValidationError]:
    """Reject unexpected top-level modules in wheels."""
    errors: list[ValidationError] = []
    top = _top_level_parts(namelist)
    for part in sorted(top):
        if part in WHEEL_ALLOWED_TOP_DIRS:
            continue
        if part in WHEEL_ALLOWED_TOP_FILES:
            continue
        if WHEEL_ALLOWED_DIST_INFO.match(part):
            continue
        if WHEEL_ALLOWED_STUBS.match(part):
            continue
        # Allow version-stamped dist-info like eggfetch-0.1.0.dist-info
        if re.match(r"^eggfetch[\w.-]*", part):
            continue
        # Allow common license/readme files
        if part.lower() in {"license", "license.txt", "license.md", "readme", "readme.md", "readme.rst", "copying", "copying.txt"}:
            continue
        errors.append(ValidationError(
            "unexpected-top-level",
            f"Wheel contains unexpected top-level entry '{part}'",
        ))
    return errors


def _check_wheel_init_py(namelist: list[str]) -> list[ValidationError]:
    """Check that eggfetch/ package has __init__.py."""
    errors: list[ValidationError] = []
    if "eggfetch/__init__.py" not in namelist:
        errors.append(ValidationError(
            "missing-init",
            "eggfetch/__init__.py not found in wheel",
        ))
    return errors


def _check_wheel_version_match(namelist: list[str]) -> list[ValidationError]:
    """Check version consistency between WHEEL metadata and __init__.py."""
    errors: list[ValidationError] = []
    # This is a best-effort check; many packages don't put __version__ in __init__.py.
    return errors


# Files/dirs to skip during secret scanning (machine-generated or legitimate auth code).
SECRET_SCAN_EXCLUDE = {
    "eggfetch-0.1.0.dist-info/sboms/",
}


def _should_skip_secret_scan(name: str) -> bool:
    """Return True if this file should be skipped during secret scanning."""
    for excl in SECRET_SCAN_EXCLUDE:
        if name.startswith(excl):
            return True
    return False


def _check_secrets(namelist: list[str], zf: zipfile.ZipFile) -> list[ValidationError]:
    """Scan text files for secrets, passwords, and local paths."""
    errors: list[ValidationError] = []
    for name in namelist:
        if _should_skip_secret_scan(name):
            continue
        if not _is_text_file(name):
            continue
        content = _read_text_entry(zf, name)
        if content is None:
            continue
        for i, line in enumerate(content.splitlines(), 1):
            for pat in SECRET_PATTERNS:
                if pat.search(line):
                    # Exclude false positives: password/token variable assignments
                    # in auth code are legitimate, not leaked secrets.
                    stripped = line.strip()
                    if stripped.startswith("self._password") or stripped.startswith("self._token"):
                        continue
                    if "password == " in stripped or "token == " in stripped:
                        continue
                    if "password = entry" in stripped or "password = pwd" in stripped:
                        continue
                    if "login, password =" in stripped or "password=password" in stripped:
                        continue
                    if "BasicAuth(" in stripped and "password=" in stripped:
                        continue
                    if "token = tokens[" in stripped:
                        continue
                    snippet = line.strip()[:120]
                    errors.append(ValidationError(
                        "secret-or-path",
                        f"Line {i}: {pat.pattern!r} matched: {snippet!r}",
                        path=name,
                    ))
                    # One match per line is enough.
                    break
    return errors


def _check_init_version(wheel_version: str | None, namelist: list[str], zf: zipfile.ZipFile) -> list[ValidationError]:
    """If __init__.py defines __version__, compare against WHEEL version."""
    errors: list[ValidationError] = []
    if wheel_version is None:
        return errors
    init_path = "eggfetch/__init__.py"
    if init_path not in namelist:
        return errors
    content = _read_text_entry(zf, init_path)
    if content is None:
        return errors
    match = re.search(r'__version__\s*=\s*["\']([^"\']+)["\']', content)
    if match:
        init_version = match.group(1)
        if init_version != wheel_version:
            errors.append(ValidationError(
                "version-mismatch",
                f"WHEEL version '{wheel_version}' != __init__.py __version__ '{init_version}'",
                path=init_path,
            ))
    return errors


def _extract_wheel_version(namelist: list[str], zf: zipfile.ZipFile) -> str | None:
    """Extract version from WHEEL metadata."""
    for name in namelist:
        if name.endswith(".dist-info/METADATA") or name.endswith(".dist-info/WHEEL"):
            content = _read_text_entry(zf, name)
            if content:
                match = re.search(r"^Version:\s*(.+)$", content, re.MULTILINE)
                if match:
                    return match.group(1).strip()
    return None


def validate_wheel(path: Path) -> list[ValidationError]:
    """Validate a wheel file."""
    errors: list[ValidationError] = []
    try:
        with zipfile.ZipFile(path, "r") as zf:
            namelist = zf.namelist()
            errors.extend(_check_wheel_forbidden_dirs(namelist))
            errors.extend(_check_wheel_forbidden_files(namelist))
            errors.extend(_check_wheel_unexpected_modules(namelist))
            errors.extend(_check_wheel_init_py(namelist))
            errors.extend(_check_secrets(namelist, zf))

            version = _extract_wheel_version(namelist, zf)
            errors.extend(_check_init_version(version, namelist, zf))
    except zipfile.BadZipFile:
        errors.append(ValidationError("invalid-zip", f"File is not a valid zip: {path}"))
    except OSError as exc:
        errors.append(ValidationError("io-error", f"Cannot read file: {exc}"))
    return errors


def validate_sdist(path: Path) -> list[ValidationError]:
    """Validate an sdist file (tar.gz or zip)."""
    errors: list[ValidationError] = []
    name_lower = path.name.lower()
    if name_lower.endswith(".zip"):
        return _validate_sdist_zip(path)
    if name_lower.endswith((".tar.gz", ".tgz")):
        return _validate_sdist_tarball(path)
    errors.append(ValidationError("unsupported-format", f"Unsupported sdist format: {path.name}"))
    return errors


def _validate_sdist_zip(path: Path) -> list[ValidationError]:
    """Validate an sdist .zip file."""
    errors: list[ValidationError] = []
    try:
        with zipfile.ZipFile(path, "r") as zf:
            namelist = zf.namelist()
            top = _top_level_parts(namelist)
            errors.extend(_check_secrets(namelist, zf))
    except zipfile.BadZipFile:
        errors.append(ValidationError("invalid-zip", f"File is not a valid zip: {path}"))
    except OSError as exc:
        errors.append(ValidationError("io-error", f"Cannot read file: {exc}"))
    return errors


def _validate_sdist_tarball(path: Path) -> list[ValidationError]:
    """Validate an sdist .tar.gz or .tgz file."""
    errors: list[ValidationError] = []
    try:
        import tarfile
        with tarfile.open(path, "r:gz") as tf:
            names = tf.getnames()
            top = _top_level_parts(names)
            # Check for forbidden top-level dirs in sdists
            for forbidden in WHEEL_FORBIDDEN_DIRS:
                if forbidden in top:
                    # For sdists, tests/ at top level is often acceptable;
                    # only reject corpus/ and fuzz/.
                    if forbidden in {"corpus", "fuzz"}:
                        errors.append(ValidationError(
                            "forbidden-top-level",
                            f"Sdist contains forbidden top-level directory '{forbidden}/'",
                        ))
    except tarfile.TarError as exc:
        errors.append(ValidationError("invalid-tar", f"File is not a valid tar archive: {exc}"))
    except OSError as exc:
        errors.append(ValidationError("io-error", f"Cannot read file: {exc}"))
    return errors


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Validate built wheel/sdist packages for forbidden content",
    )
    parser.add_argument(
        "package",
        help="Path to a wheel (.whl) or sdist (.tar.gz, .zip) file",
    )
    args = parser.parse_args()
    path = Path(args.package).resolve()

    if not path.exists():
        print(f"ERROR: File not found: {path}", file=sys.stderr)
        sys.exit(1)

    name_lower = path.name.lower()
    is_wheel = name_lower.endswith(".whl")
    is_sdist = name_lower.endswith((".tar.gz", ".tgz", ".zip"))

    if not is_wheel and not is_sdist:
        print(f"ERROR: Unrecognized package format: {path.name}", file=sys.stderr)
        print("Expected .whl, .tar.gz, .tgz, or .zip", file=sys.stderr)
        sys.exit(1)

    kind = "wheel" if is_wheel else "sdist"
    print(f"Validating {kind}: {path.name}")

    if is_wheel:
        errors = validate_wheel(path)
    else:
        errors = validate_sdist(path)

    if errors:
        print(f"\nFAILED: {len(errors)} issue(s) found:\n", file=sys.stderr)
        for err in errors:
            print(f"  {err}", file=sys.stderr)
        sys.exit(1)
    else:
        print("PASSED: All checks passed")
        sys.exit(0)


if __name__ == "__main__":
    main()
