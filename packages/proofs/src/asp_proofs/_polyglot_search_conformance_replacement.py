"""Replacement-certificate conformance validator."""

from __future__ import annotations

from collections.abc import Mapping
from typing import Any

from asp_proofs._polyglot_search_conformance_model import (
    ConformanceReceipt,
    Violation,
    canonical_json,
    mapping,
    receipt,
)


def validate_replacement_certificate(
    packet: Mapping[str, Any], *, state_digest: str
) -> ConformanceReceipt:
    violations: list[Violation] = []
    runtime = mapping(packet.get("runtime"))
    runtime_digests = {
        runtime.get("candidateDigest"),
        runtime.get("installedDigest"),
        runtime.get("liveDigest"),
    }
    if None in runtime_digests or len(runtime_digests) != 1:
        violations.append(
            Violation(
                "replacement-runtime-drift",
                "/runtime",
                "Candidate, installed, and live digests must match.",
            )
        )
    for key, value in mapping(packet.get("safety")).items():
        if key.endswith("Count") and value != 0:
            violations.append(
                Violation(
                    "replacement-safety-gate-failed",
                    f"/safety/{key}",
                    "Safety counts must be zero.",
                )
            )
    if len(packet.get("conformanceReceiptDigests", [])) < 5:
        violations.append(
            Violation(
                "replacement-conformance-incomplete",
                "/conformanceReceiptDigests",
                "Five language receipts are required.",
            )
        )
    if len(packet.get("benchmarkReceiptDigests", [])) < 2:
        violations.append(
            Violation(
                "replacement-benchmark-slices-incomplete",
                "/benchmarkReceiptDigests",
                "Two independent benchmark slices are required.",
            )
        )
    for gate_name in ("quality", "efficiency", "canary"):
        if mapping(packet.get(gate_name)).get("passed") is not True:
            violations.append(
                Violation(
                    "replacement-gate-failed",
                    f"/{gate_name}/passed",
                    f"{gate_name} gate must pass.",
                )
            )
    bound_digest = packet.get("boundProjectionDigest")
    if not isinstance(bound_digest, str) or not bound_digest:
        violations.append(
            Violation(
                "replacement-projection-unbound",
                "/boundProjectionDigest",
                "Certificate must bind the exact projected packet.",
            )
        )
    return receipt(
        subject_kind="replacement-certificate",
        raw_input=canonical_json(packet).encode(),
        state_digest=state_digest,
        parsed_summary={"boundProjectionDigest": bound_digest},
        violations=violations,
    )
