#!/usr/bin/env python3
"""Self-test for the compatibility-profile skip auditor.

The auditor in ``crates/eggfetch-python/tests/compat/conftest.py`` must fail a
required compatibility profile that carries an unapproved skip. The full
compatibility profiles only run in Tier 2, so this self-test drives the real
conftest against synthetic sessions: it asserts the gate verdict only, and
makes no compatibility claim.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CONFTEST = ROOT / "crates" / "eggfetch-python" / "tests" / "compat" / "conftest.py"

UNAPPROVED_TEST = """import pytest


def test_unapproved_skip() -> None:
    pytest.skip("synthetic unapproved skip")
"""

APPROVED_TEST = """import pytest


def test_google_genai_sync_client_args_accept_eggfetch_transport() -> None:
    pytest.skip("synthetic approved skip")
"""

CLEAN_TEST = """def test_clean() -> None:
    assert True
"""


class SkipAuditorTests(unittest.TestCase):
    """Assert the required-profile verdict the skip auditor produces."""

    def setUp(self) -> None:
        self.workdir = Path(tempfile.mkdtemp(prefix="eggfetch-skip-auditor-"))
        self.addCleanup(shutil.rmtree, self.workdir, True)
        shutil.copy2(CONFTEST, self.workdir / "conftest.py")

    def _run(
        self, filename: str, body: str, *, required: bool
    ) -> subprocess.CompletedProcess[str]:
        (self.workdir / filename).write_text(body, encoding="utf-8")
        env = dict(os.environ)
        for name in ("EGGFETCH_COMPAT_REQUIRED", "EGGFETCH_COMPAT_PROFILE", "PYTEST_ADDOPTS"):
            env.pop(name, None)
        if required:
            env["EGGFETCH_COMPAT_REQUIRED"] = "1"
        return subprocess.run(
            [sys.executable, "-m", "pytest", filename, "-q", "-p", "no:cacheprovider"],
            cwd=self.workdir,
            env=env,
            capture_output=True,
            text=True,
            check=False,
        )

    def test_unapproved_skip_fails_required_profile(self) -> None:
        result = self._run("test_unapproved.py", UNAPPROVED_TEST, required=True)
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_approved_skip_passes_required_profile(self) -> None:
        result = self._run(
            "test_httpx_transport_sdk_construction.py", APPROVED_TEST, required=True
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_skip_is_tolerated_outside_required_mode(self) -> None:
        result = self._run("test_unapproved.py", UNAPPROVED_TEST, required=False)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_clean_run_passes_required_profile(self) -> None:
        result = self._run("test_clean.py", CLEAN_TEST, required=True)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
