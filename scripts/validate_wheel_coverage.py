#!/usr/bin/env python3
"""Validate that the assembled wheel set covers the expected matrix.

Expected coverage:
  - 5 platforms: linux x86_64, linux aarch64, macOS x86_64, macOS arm64, Windows x86_64
  - 4 Python versions: 3.10, 3.11, 3.12, 3.13
  - 20 total wheels

Parses wheel filenames (PEP 427) to extract tags. No third-party dependencies.
"""

import re
import sys
from pathlib import Path

EXPECTED_PLATFORMS = {
    "linux_x86_64",
    "linux_aarch64",
    "macosx_x86_64",
    "macosx_arm64",
    "win_amd64",
}

EXPECTED_PYTHON_VERSIONS = {"cp310", "cp311", "cp312", "cp313"}

# Wheel filename pattern: {distribution}-{version}(-{build tag})?-{python tag}-{abi tag}-{platform tag}.whl
WHEEL_RE = re.compile(
    r"^(?P<name>.+?)-(?P<version>[^-]+)"
    r"(?:-(?P<build>\d[^-]*))?"
    r"-(?P<py>[^-]+)"
    r"-(?P<abi>[^-]+)"
    r"-(?P<platform>[^-]+)"
    r"\.whl$"
)


def parse_wheel_filename(filename: str) -> dict | None:
    """Parse a wheel filename and return its tags."""
    m = WHEEL_RE.match(filename)
    if not m:
        return None
    return m.groupdict()


def normalize_platform(platform_tag: str) -> str:
    """Normalize a platform tag to our expected set."""
    # Handle compound tags like manylinux_2_17_x86_64.manylinux2014_x86_64
    parts = platform_tag.split(".")
    for part in parts:
        # manylinux tags
        if "manylinux" in part and "x86_64" in part:
            return "linux_x86_64"
        if "manylinux" in part and "aarch64" in part:
            return "linux_aarch64"
        # macosx tags with deployment target (e.g., macosx_10_9_x86_64)
        if part.startswith("macosx_"):
            if "x86_64" in part:
                return "macosx_x86_64"
            if "arm64" in part:
                return "macosx_arm64"
        # windows tags
        if part == "win_amd64":
            return "win_amd64"
    # Direct match
    if platform_tag in EXPECTED_PLATFORMS:
        return platform_tag
    return platform_tag


def normalize_python(py_tag: str) -> str:
    """Normalize a Python tag to our expected set."""
    # Handle tags like cp312-cp312
    parts = py_tag.split(".")
    for part in parts:
        if part.startswith("cp") and len(part) == 5:
            return part
    # Handle abi3 tags - reject them per plan
    if "abi3" in py_tag:
        return "abi3"
    return py_tag


def main() -> int:
    if len(sys.argv) != 2:
        print(f"Usage: {sys.argv[0]} <wheels-directory>", file=sys.stderr)
        return 1

    wheels_dir = Path(sys.argv[1])
    if not wheels_dir.is_dir():
        print(f"FAIL: {wheels_dir} is not a directory", file=sys.stderr)
        return 1

    wheel_files = sorted(wheels_dir.glob("*.whl"))
    if not wheel_files:
        print(f"FAIL: no wheels found in {wheels_dir}", file=sys.stderr)
        return 1

    errors: list[str] = []
    observed: set[tuple[str, str]] = set()  # (platform, python)
    abi3_wheels: list[str] = []

    for wf in wheel_files:
        info = parse_wheel_filename(wf.name)
        if info is None:
            errors.append(f"Could not parse wheel filename: {wf.name}")
            continue

        py = normalize_python(info["py"])
        platform = normalize_platform(info["platform"])

        if py == "abi3":
            abi3_wheels.append(wf.name)
            continue

        if py not in EXPECTED_PYTHON_VERSIONS:
            errors.append(f"Unexpected Python tag {py!r} in {wf.name}")
            continue

        if platform not in EXPECTED_PLATFORMS:
            errors.append(f"Unexpected platform {platform!r} in {wf.name}")
            continue

        observed.add((platform, py))

    # Check for ABI3 wheels (forbidden per plan)
    if abi3_wheels:
        errors.append(
            f"ABI3 wheels found (not allowed without explicit configuration): "
            + ", ".join(abi3_wheels)
        )

    # Check for missing coverage
    expected = {(p, py) for p in EXPECTED_PLATFORMS for py in EXPECTED_PYTHON_VERSIONS}
    missing = expected - observed
    unexpected = observed - expected

    if missing:
        for platform, py in sorted(missing):
            errors.append(f"Missing wheel: {platform} py{py}")

    if unexpected:
        for platform, py in sorted(unexpected):
            errors.append(f"Unexpected wheel: {platform} py{py}")

    # Check total count
    if len(observed) != 20:
        errors.append(f"Expected 20 wheels, observed {len(observed)}")

    if errors:
        for e in errors:
            print(f"FAIL: {e}", file=sys.stderr)
        return 1

    # Print observed coverage
    for platform in sorted(EXPECTED_PLATFORMS):
        for py in sorted(EXPECTED_PYTHON_VERSIONS):
            print(f"  {platform} py{py[2:]}")
    print(f"\nAll {len(observed)} wheels present with expected coverage.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
