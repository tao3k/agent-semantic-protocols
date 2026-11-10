# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Relation-batch and progressive-turn conformance validators."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from typing import Any

from asp_proofs._polyglot_search_conformance_model import (
    RELATION_ID_PATTERN,
    ConformanceReceipt,
    Violation,
    canonical_json,
    mapping,
    receipt,
)


def validate_relation_batch(
    packet: Mapping[str, Any],
    *,
    expected_snapshot_digest: str,
    expected_provider_digest: str,
    state_digest: str,
) -> ConformanceReceipt:
    violations: list[Violation] = []
    if packet.get("abiVersion") != 1:
        violations.append(
            Violation("relation-abi-drift", "/abiVersion", "ABI version must be 1.")
        )
    if packet.get("sourceSnapshotDigest") != expected_snapshot_digest:
        violations.append(
            Violation(
                "relation-snapshot-drift",
                "/sourceSnapshotDigest",
                "Snapshot digest differs.",
            )
        )
    producer = mapping(packet.get("producer"))
    if producer.get("artifactDigest") != expected_provider_digest:
        violations.append(
            Violation(
                "relation-provider-drift",
                "/producer/artifactDigest",
                "Provider digest differs.",
            )
        )
    relations = packet.get("relations")
    if not _is_sequence(relations):
        relations = []
        violations.append(
            Violation("relations-required", "/relations", "Relations must be an array.")
        )
    relation_ids: set[str] = set()
    for index, raw_relation in enumerate(relations):
        _validate_relation(raw_relation, index, relation_ids, violations)
    return receipt(
        subject_kind="relation-batch",
        raw_input=canonical_json(packet).encode(),
        state_digest=state_digest,
        parsed_summary={"relationIds": sorted(relation_ids)},
        violations=violations,
    )


def _validate_relation(
    raw_relation: object,
    index: int,
    relation_ids: set[str],
    violations: list[Violation],
) -> None:
    relation = mapping(raw_relation)
    path = f"/relations/{index}"
    relation_id = relation.get("relationId")
    if not isinstance(relation_id, str) or not RELATION_ID_PATTERN.fullmatch(
        relation_id
    ):
        violations.append(
            Violation(
                "relation-id-not-namespaced",
                f"{path}/relationId",
                "Relation ID must be namespaced.",
            )
        )
    elif relation_id in relation_ids:
        violations.append(
            Violation(
                "duplicate-relation-id",
                f"{path}/relationId",
                "Relation ID is duplicated.",
            )
        )
    else:
        relation_ids.add(relation_id)
    columns = relation.get("columns")
    column_names = (
        [column.get("name") for column in columns if isinstance(column, Mapping)]
        if _is_sequence(columns)
        else []
    )
    if len(column_names) != len(set(column_names)) or not column_names:
        violations.append(
            Violation(
                "relation-columns-invalid",
                f"{path}/columns",
                "Columns must be nonempty and unique.",
            )
        )
    rows = relation.get("rows")
    if not _is_sequence(rows):
        rows = []
        violations.append(
            Violation("relation-rows-invalid", f"{path}/rows", "Rows must be an array.")
        )
    for row_index, raw_row in enumerate(rows):
        if set(mapping(raw_row)) != set(column_names):
            violations.append(
                Violation(
                    "relation-row-schema-mismatch",
                    f"{path}/rows/{row_index}",
                    "Row keys must equal the declared column set.",
                )
            )
    if relation_id == "asp.turbo_feature" and relation.get("authority") != "candidate":
        violations.append(
            Violation(
                "turbo-feature-authority-violation",
                f"{path}/authority",
                "Graph Turbo features have candidate authority.",
            )
        )


def validate_progressive_turn(
    packet: Mapping[str, Any],
    *,
    expected_semantic_digest: str,
    state_digest: str,
) -> ConformanceReceipt:
    violations: list[Violation] = []
    visible = packet.get("visibleCandidates")
    if not _is_sequence(visible):
        visible = []
        violations.append(
            Violation(
                "visible-candidates-required",
                "/visibleCandidates",
                "Visible candidates must be an array.",
            )
        )
    visible_ids = [
        candidate.get("candidateId")
        for candidate in visible
        if isinstance(candidate, Mapping)
        and isinstance(candidate.get("candidateId"), str)
    ]
    if len(visible_ids) != len(set(visible_ids)):
        violations.append(
            Violation(
                "duplicate-visible-candidate",
                "/visibleCandidates",
                "Visible candidate IDs must be unique.",
            )
        )
    _validate_turn_budgets(packet, visible_ids, violations)
    semantic_digest = packet.get("semanticDigest")
    if semantic_digest != expected_semantic_digest:
        violations.append(
            Violation(
                "turn-semantic-drift",
                "/semanticDigest",
                "Turn semantic digest differs.",
            )
        )
    _validate_continuation(packet, semantic_digest, violations)
    selected = packet.get("selectedCandidateIds")
    return receipt(
        subject_kind="progressive-turn",
        raw_input=canonical_json(packet).encode(),
        state_digest=state_digest,
        parsed_summary={
            "visibleCandidateIds": visible_ids,
            "selectedCandidateIds": list(selected) if _is_sequence(selected) else [],
        },
        violations=violations,
    )


def _validate_turn_budgets(packet, visible_ids, violations) -> None:
    exposure = packet.get("exposureBudget")
    if not isinstance(exposure, int) or len(visible_ids) > exposure or exposure > 10:
        violations.append(
            Violation(
                "frontier-exposure-budget-exceeded",
                "/visibleCandidates",
                "Visible frontier exceeds its budget.",
            )
        )
    selected = packet.get("selectedCandidateIds")
    if not _is_sequence(selected):
        selected = []
        violations.append(
            Violation(
                "selection-required",
                "/selectedCandidateIds",
                "Selection must be an array.",
            )
        )
    selection = packet.get("selectionBudget")
    if not isinstance(selection, int) or len(selected) > selection or selection > 3:
        violations.append(
            Violation(
                "selection-budget-exceeded",
                "/selectedCandidateIds",
                "Selection exceeds its budget.",
            )
        )
    if not set(selected).issubset(visible_ids):
        violations.append(
            Violation(
                "unexposed-candidate-selected",
                "/selectedCandidateIds",
                "Selection contains an unexposed candidate.",
            )
        )


def _validate_continuation(packet, semantic_digest, violations) -> None:
    omitted = packet.get("omittedItemCount")
    continuation = packet.get("continuation")
    if (
        isinstance(omitted, int)
        and omitted > 0
        and not isinstance(continuation, Mapping)
    ):
        violations.append(
            Violation(
                "omission-continuation-required",
                "/continuation",
                "Omitted items require a continuation.",
            )
        )
    if (
        isinstance(continuation, Mapping)
        and continuation.get("semanticDigest") != semantic_digest
    ):
        violations.append(
            Violation(
                "stale-continuation",
                "/continuation/semanticDigest",
                "Continuation is bound to another semantic result.",
            )
        )


def _is_sequence(value: object) -> bool:
    return isinstance(value, Sequence) and not isinstance(value, (str, bytes))
