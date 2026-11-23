#!/usr/bin/env python3
"""Validate that the assembled wheel set covers the expected matrix.

Expected coverage:
  - 4 platforms: Linux x86_64 + aarch64, macOS arm64, Windows x86_64
  - 6 GIL-enabled Python versions: 3.10, 3.11, 3.12, 3.13, 3.14, 3.15
  - 2 free-threaded Python versions: 3.14t, 3.15t
  - 32 total wheels

Coverage is keyed by (platform, Python tag, ABI tag). A GIL-enabled wheel and
a free-threaded wheel for the same Python version are different artifacts
(cp314-cp314 versus cp314-cp314t), so they are counted separately. A wheel is
accepted only when its ABI tag matches its Python tag, or is that tag with a
"t" suffix for one of the free-threaded targets. The Stable ABI is rejected:
the ABI tag is what carries "abi3", and it is checked.

Parses wheel filenames (PEP 427) to extract tags. No third-party dependencies.
"""

import re
import sys
from pathlib import Path

EXPECTED_PLATFORMS = {
    "linux_x86_64",
    "linux_aarch64",
    "macosx_arm64",
    "win_amd64",
}

# Platform tag used in the self-test fixtures, per normalized platform.
PLATFORM_TAGS = {
    "linux_x86_64": "manylinux2014_x86_64",
    "linux_aarch64": "manylinux2014_aarch64",
    "macosx_arm64": "macosx_11_0_arm64",
    "win_amd64": "win_amd64",
}

# GIL-enabled interpreters: the ABI tag equals the Python tag.
GIL_PYTHON_VERSIONS = ("cp310", "cp311", "cp312", "cp313", "cp314", "cp315")

# Free-threaded interpreters: the Python tag stays cpXY and the ABI tag adds
# the "t" suffix (cp314-cp314t). PyO3 0.29 dropped 3.13t, so 3.14 is the
# first free-threaded target.
FREE_THREADED_PYTHON_VERSIONS = ("cp314", "cp315")

# (Python tag, ABI tag) pairs the release set may contain.
EXPECTED_PAIRS = {
    (version, version) for version in GIL_PYTHON_VERSIONS
} | {
    (version, f"{version}t") for version in FREE_THREADED_PYTHON_VERSIONS
}

KNOWN_PYTHON_TAGS = {version for version, _ in EXPECTED_PAIRS}

EXPECTED_WHEEL_COUNT = len(EXPECTED_PLATFORMS) * len(EXPECTED_PAIRS)

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
    return platform_tag


def normalize_python(py_tag: str) -> str:
    """Normalize a Python tag to our expected set."""
    # Handle compound tags like cp312.cp313
    for part in py_tag.split("."):
        if re.fullmatch(r"cp\d{3}", part):
            return part
    return py_tag


def classify_wheel(name: str) -> tuple[str | None, str | None, str | None, list[str]]:
    """Classify one wheel filename.

    Returns (platform, python tag, ABI tag, errors). The tags are returned even
    when the wheel is rejected, so callers can report them.
    """
    errors: list[str] = []

    info = parse_wheel_filename(name)
    if info is None:
        return None, None, None, [f"Could not parse wheel filename: {name}"]

    py = normalize_python(info["py"])
    abi = info["abi"]
    platform = normalize_platform(info["platform"])

    # The Stable ABI lives in the ABI tag, so it is checked there. A
    # free-threaded wheel carries cpXYt, never "abi3".
    if "abi3" in abi:
        errors.append(
            "ABI3 wheels found (not allowed without explicit configuration): " + name
        )
    elif py not in KNOWN_PYTHON_TAGS:
        errors.append(f"Unexpected Python tag {py!r} in {name}")
    elif (py, abi) not in EXPECTED_PAIRS:
        errors.append(f"Unexpected Python/ABI combination {py}-{abi} in {name}")

    if platform not in EXPECTED_PLATFORMS:
        errors.append(f"Unexpected platform {platform!r} in {name}")

    return platform, py, abi, errors


def validate_wheel_names(names: list[str]) -> tuple[list[str], set[tuple[str, str, str]]]:
    """Validate wheel filenames against the expected matrix.

    Returns (errors, observed coverage keys).
    """
    errors: list[str] = []
    observed: set[tuple[str, str, str]] = set()

    for name in sorted(names):
        platform, py, abi, wheel_errors = classify_wheel(name)
        errors.extend(wheel_errors)
        if wheel_errors:
            continue
        observed.add((platform, py, abi))

    expected = {
        (platform, py, abi)
        for platform in EXPECTED_PLATFORMS
        for (py, abi) in EXPECTED_PAIRS
    }

    for platform, py, abi in sorted(expected - observed):
        errors.append(f"Missing wheel: {platform} {py}-{abi}")

    for platform, py, abi in sorted(observed - expected):
        errors.append(f"Unexpected wheel: {platform} {py}-{abi}")

    if len(observed) != EXPECTED_WHEEL_COUNT:
        errors.append(
            f"Expected {EXPECTED_WHEEL_COUNT} wheels, observed {len(observed)}"
        )

    return errors, observed


def _complete_wheel_set() -> list[str]:
    """Every wheel the release set is expected to contain."""
    names = []
    for platform in sorted(EXPECTED_PLATFORMS):
        for py, abi in sorted(EXPECTED_PAIRS):
            names.append(f"eggfetch-0.4.0-{py}-{abi}-{PLATFORM_TAGS[platform]}.whl")
    return names


def self_test() -> int:
    """Self-test of the coverage rules.

    Asserts which ABI tags are allowed and rejected, and that the coverage
    check accepts a complete set and rejects an incomplete one. This exercises
    the rules only; it is not a release result.
    """
    failures: list[str] = []

    def expect(condition: bool, message: str) -> None:
        if not condition:
            failures.append(message)

    def expect_rejected(name: str, needle: str) -> None:
        _, _, _, errors = classify_wheel(name)
        expect(bool(errors), f"expected {name} to be rejected")
        expect(
            any(needle in error for error in errors),
            f"expected {name} to be rejected with {needle!r}, got {errors}",
        )

    # Accepted tags.
    for name in (
        "eggfetch-0.4.0-cp310-cp310-manylinux2014_x86_64.whl",
        "eggfetch-0.4.0-cp314-cp314-macosx_11_0_arm64.whl",
        "eggfetch-0.4.0-cp314-cp314t-macosx_11_0_arm64.whl",
        "eggfetch-0.4.0-cp315-cp315t-win_amd64.whl",
    ):
        _, _, _, errors = classify_wheel(name)
        expect(not errors, f"expected {name} to be accepted, got {errors}")

    # Rejected tags: the Stable ABI, a mismatched ABI, an unsupported
    # free-threaded target, an unknown Python tag, an unknown platform.
    expect_rejected(
        "eggfetch-0.4.0-cp310-abi3-manylinux2014_x86_64.whl", "ABI3"
    )
    expect_rejected(
        "eggfetch-0.4.0-cp314-cp313-manylinux2014_x86_64.whl",
        "Python/ABI combination",
    )
    expect_rejected(
        "eggfetch-0.4.0-cp313-cp313t-manylinux2014_x86_64.whl",
        "Python/ABI combination",
    )
    expect_rejected(
        "eggfetch-0.4.0-cp39-cp39-manylinux2014_x86_64.whl", "Unexpected Python tag"
    )
    expect_rejected(
        "eggfetch-0.4.0-py3-none-any.whl", "Unexpected Python tag"
    )
    expect_rejected(
        "eggfetch-0.4.0-cp314-cp314t-macosx_10_9_x86_64.whl", "Unexpected platform"
    )

    # A complete set passes; a set missing one free-threaded wheel fails.
    complete = _complete_wheel_set()
    expect(len(complete) == EXPECTED_WHEEL_COUNT, "fixture set has the wrong size")
    errors, observed = validate_wheel_names(complete)
    expect(not errors, f"complete set rejected: {errors}")
    expect(len(observed) == EXPECTED_WHEEL_COUNT, "complete set coverage is wrong")

    incomplete = [
        name
        for name in complete
        if not name.endswith(f"cp315-cp315t-{PLATFORM_TAGS['linux_aarch64']}.whl")
    ]
    errors, _ = validate_wheel_names(incomplete)
    expect(
        any("Missing wheel" in error for error in errors),
        f"incomplete set not reported as missing: {errors}",
    )

    # A free-threaded wheel does not stand in for its GIL-enabled twin.
    swapped = [
        name.replace("-cp314-cp314-", "-cp314-cp314t-")
        for name in complete
    ]
    errors, _ = validate_wheel_names(swapped)
    expect(
        any("Missing wheel" in error for error in errors),
        f"swapped set not reported as missing: {errors}",
    )

    if failures:
        for failure in failures:
            print(f"FAIL: {failure}", file=sys.stderr)
        return 1

    print("Wheel coverage validator self-test passed")
    return 0


def main() -> int:
    if len(sys.argv) == 2 and sys.argv[1] == "--self-test":
        return self_test()

    if len(sys.argv) != 2:
        print(f"Usage: {sys.argv[0]} <wheels-directory> | --self-test", file=sys.stderr)
        return 1

    wheels_dir = Path(sys.argv[1])
    if not wheels_dir.is_dir():
        print(f"FAIL: {wheels_dir} is not a directory", file=sys.stderr)
        return 1

    wheel_files = sorted(wheels_dir.glob("*.whl"))
    if not wheel_files:
        print(f"FAIL: no wheels found in {wheels_dir}", file=sys.stderr)
        return 1

    errors, observed = validate_wheel_names([wf.name for wf in wheel_files])

    if errors:
        for error in errors:
            print(f"FAIL: {error}", file=sys.stderr)
        return 1

    # Print observed coverage
    for platform in sorted(EXPECTED_PLATFORMS):
        for py, abi in sorted(EXPECTED_PAIRS):
            if (platform, py, abi) in observed:
                gil = "" if abi.endswith("t") else " (GIL)"
                print(f"  {platform} {py}-{abi}{gil}")
    print(f"\nAll {len(observed)} wheels present with expected coverage.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
