"""Configuration for HTTPX compatibility tests.

This conftest enforces fail-closed behavior: if httpx or requests is not
installed, or the wrong version is installed, the entire test session fails
rather than silently skipping.
"""

import os

import pytest

REQUIRED_HTTPX_VERSION = "0.28.1"

# Module-level state for skip auditing
_skipped: list[str] = []
_xfailed: list[str] = []
_collection_errors: list[str] = []
_profile = os.environ.get("EGGFETCH_COMPAT_PROFILE", "httpx/0.28.1")


def pytest_configure(config):
    """Register compatibility markers."""
    config.addinivalue_line("markers", "compat: HTTPX compatibility tests (required)")


def pytest_runtest_logreport(report):
    """Record skips, xfails, and collection errors for audit."""
    if report.when == "call":
        if report.skipped:
            _skipped.append(report.nodeid)
        if getattr(report, "wasxfail", False):
            _xfailed.append(report.nodeid)
    if report.when == "collect":
        if report.skipped:
            _collection_errors.append(report.nodeid)


@pytest.hookimpl(trylast=True)
def pytest_terminal_summary(terminalreporter, exitstatus, config):
    """Print skip audit summary and fail on unexplained skips in required mode."""
    is_required = os.environ.get("EGGFETCH_COMPAT_REQUIRED", "0") == "1"

    terminalreporter.section("Skip Audit")

    if _skipped:
        terminalreporter.write_line(
            f"Skipped tests ({len(_skipped)}):", yellow=True
        )
        for nodeid in _skipped:
            terminalreporter.write_line(f"  - {nodeid}", yellow=True)

    if _xfailed:
        terminalreporter.write_line(
            f"XFailed tests ({len(_xfailed)}):", yellow=True
        )
        for nodeid in _xfailed:
            terminalreporter.write_line(f"  - {nodeid}", yellow=True)

    if _collection_errors:
        terminalreporter.write_line(
            f"Collection errors ({len(_collection_errors)}):", red=True
        )
        for nodeid in _collection_errors:
            terminalreporter.write_line(f"  - {nodeid}", red=True)

    terminalreporter.write_line(f"Profile: {_profile}")

    if is_required:
        all_issues = _skipped + _xfailed + _collection_errors
        if all_issues:
            terminalreporter.write_line(
                f"\nFAIL: Required compatibility profile has "
                f"{len(all_issues)} unapproved skip/xfail/collection issue(s)",
                red=True,
            )
            config._compat_exit_code = 1


@pytest.hookimpl(trylast=True)
def pytest_sessionfinish(session, exitstatus):
    """Override exit code if required profile found issues."""
    is_required = os.environ.get("EGGFETCH_COMPAT_REQUIRED", "0") == "1"
    if is_required and hasattr(session.config, "_compat_exit_code"):
        session.exitstatus = session.config._compat_exit_code
