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

# Skips that a required profile is allowed to carry.
#
# Every skip, xfail, or collection error in a required profile must be listed
# here with the reason it cannot run in the qualification environment; any
# other issue fails the profile run. Keys are matched against the end of the
# reported node id, so a match does not depend on the pytest rootdir.
APPROVED_SKIPS: dict[str, str] = {
    (
        "test_httpx_transport_sdk_construction.py::"
        "test_google_genai_sync_client_args_accept_eggfetch_transport"
    ): (
        "google-genai is absent from the qualification environment: the current "
        "SDK constructs SyncHttpx2Client, so the httpx.Client isinstance check "
        "fails (upstream API drift, not an eggfetch regression)."
    ),
}


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


def _audit() -> tuple[list[tuple[str, str]], list[str]]:
    """Split every audited issue into approved and unapproved node ids."""
    approved: list[tuple[str, str]] = []
    unapproved: list[str] = []
    for nodeid in _skipped + _xfailed + _collection_errors:
        reason = next(
            (
                reason
                for pattern, reason in APPROVED_SKIPS.items()
                if nodeid.endswith(pattern)
            ),
            None,
        )
        if reason is None:
            unapproved.append(nodeid)
        else:
            approved.append((nodeid, reason))
    return approved, unapproved


@pytest.hookimpl(trylast=True)
def pytest_terminal_summary(terminalreporter, exitstatus):
    """Print the skip audit and the required-profile verdict."""
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

    approved, unapproved = _audit()
    if approved:
        terminalreporter.write_line(
            f"Approved skips ({len(approved)}):", yellow=True
        )
        for nodeid, reason in approved:
            terminalreporter.write_line(f"  - {nodeid}", yellow=True)
            terminalreporter.write_line(f"      {reason}", yellow=True)

    terminalreporter.write_line(f"Profile: {_profile}")

    if is_required and unapproved:
        terminalreporter.write_line(
            f"\nFAIL: Required compatibility profile has "
            f"{len(unapproved)} unapproved skip/xfail/collection issue(s)",
            red=True,
        )


@pytest.hookimpl(trylast=True)
def pytest_sessionfinish(session, exitstatus):
    """Fail the session when a required profile carries an unapproved skip.

    The verdict is computed here, not in pytest_terminal_summary: pytest runs
    this hook before the terminal reporter's own session finish, so a verdict
    recorded during the terminal summary was never applied to the session exit
    status. The audited lists are already complete at this point because
    pytest_runtest_logreport recorded them during the run.
    """
    if os.environ.get("EGGFETCH_COMPAT_REQUIRED", "0") != "1":
        return
    _, unapproved = _audit()
    if unapproved:
        session.exitstatus = 1
