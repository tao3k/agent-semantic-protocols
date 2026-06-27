"""Public route verification API for semantic sandtable receipts."""

from __future__ import annotations

from typing import Any

from .route_verification_common import RouteVerificationResult
from .route_verification_quality import quality_findings
from .route_verification_trace import build_trace
from .utils import dict_value


def evaluate_route_verification(
    commands: list[dict[str, Any]],
    expectation: dict[str, Any] | None = None,
) -> RouteVerificationResult:
    expected = dict_value(expectation)
    trace = build_route_verification_trace(commands, expected)
    return RouteVerificationResult(
        trace=trace,
        quality_findings=quality_findings(trace, expected),
    )


def build_route_verification_trace(
    commands: list[dict[str, Any]],
    expectation: dict[str, Any] | None = None,
) -> dict[str, Any]:
    return build_trace(commands, dict_value(expectation))
