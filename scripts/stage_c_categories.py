"""Shared Stage C category registry for qualification.

This is the single source of truth for all eight required Stage C
downstream categories. Every qualification script, manifest validator,
matrix generator, result aggregator, evidence validator, and status
generator must import or parse this registry.

Do not duplicate category sets in multiple scripts.

Per plan §8.1:
  The manifest validator, matrix generator, result aggregator,
  evidence validator, and status generator must import or parse the
  same registry.
"""

from __future__ import annotations

# The eight required Stage C categories.
# Order is preserved for deterministic output.
STAGE_C_CATEGORIES: list[str] = [
    "contract-tests",
    "mock-transport-request-matching",
    "framework-test-client",
    "asgi-test-client",
    "sdk-async-client",
    "streaming-sse-consumption",
    "custom-auth-flow",
    "event-hooks-instrumentation",
]

# Frozen set for fast membership checks.
STAGE_C_CATEGORY_SET: frozenset[str] = frozenset(STAGE_C_CATEGORIES)


def validate_category_coverage(
    covered: set[str],
    *,
    non_package_categories: set[str] | None = None,
) -> list[str]:
    """Validate that all required Stage C categories are covered.

    Args:
        covered: Categories covered by release-blocking packages.
        non_package_categories: Categories covered by non-package jobs
            (e.g., API oracles for contract-tests).

    Returns:
        List of error strings. Empty means all categories are covered.
    """
    non_pkg = non_package_categories or set()
    missing = STAGE_C_CATEGORY_SET - covered - non_pkg
    if missing:
        return [
            f"categories not covered by release-blocking packages: "
            f"{', '.join(sorted(missing))}"
        ]
    return []
