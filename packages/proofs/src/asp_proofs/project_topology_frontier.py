# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Expected-relation and frontier truth table for Project Topology V1."""

from __future__ import annotations

from ._topology_admission import require_topology


def _key(item: dict) -> tuple[str, str, str, str, int]:
    return (
        item["anchor"],
        item["target"],
        item["relation"],
        item["targetKind"],
        item["depth"],
    )


def _coverage(
    reference: str | None,
    relation: str,
    target_kind: str,
    scope: str,
    certificates: dict[str, dict],
) -> str:
    require_topology(reference is not None, "topology-frontier-coverage-missing")
    certificate = certificates.get(reference)
    require_topology(
        certificate is not None
        and certificate["relation"] == relation
        and certificate["targetKind"] == target_kind
        and certificate["scope"] == scope,
        "topology-frontier-coverage-mismatch",
    )
    return reference


def validate_relation_frontiers(
    library: dict, nodes: dict[str, dict], edges: dict[str, dict]
) -> None:
    """Require frontiers to equal exactly unresolved source-owned obligations."""

    positive = {
        (edge["from"], edge["to"], edge["relation"])
        for edge in edges.values()
        if edge["modality"] in {"parser-direct", "declared", "derived"}
    }
    certificates: dict[str, dict] = {}
    for certificate in library["coverageCertificates"]:
        certificate_id = certificate["id"]
        require_topology(
            certificate_id not in certificates, "topology-coverage-duplicate"
        )
        certificates[certificate_id] = certificate

    frontiers: dict[tuple[str, str, str, str, int], dict] = {}
    for frontier in library["frontiers"]:
        key = _key(frontier)
        require_topology(key not in frontiers, "topology-frontier-duplicate")
        frontiers[key] = frontier

    expectation_ids: set[str] = set()
    expectation_keys: set[tuple[str, str, str, str, int]] = set()
    unresolved: set[tuple[str, str, str, str, int]] = set()
    referenced_certificates: set[str] = set()
    for expected in library["expectedRelations"]:
        expected_id = expected["id"]
        require_topology(
            expected_id not in expectation_ids,
            "topology-expected-relation-duplicate",
        )
        expectation_ids.add(expected_id)
        key = _key(expected)
        require_topology(
            key not in expectation_keys, "topology-expected-relation-duplicate"
        )
        expectation_keys.add(key)
        anchor, target, relation, target_kind, _depth = key
        require_topology(
            anchor in nodes and nodes.get(target, {}).get("kind") == target_kind,
            "topology-expected-relation-endpoint-mismatch",
        )
        frontier = frontiers.get(key)
        if (anchor, target, relation) in positive:
            require_topology(frontier is None, "topology-positive-frontier-conflict")
            continue
        unresolved.add(key)
        require_topology(frontier is not None, "topology-frontier-missing")
        state = frontier["state"]
        reason = frontier["reason"]
        reference = frontier.get("coverageRef")
        coverage = expected["coverage"]
        if coverage == "none":
            require_topology(
                (state, reason, reference)
                == ("unknown", "binding-not-established", None),
                "topology-frontier-classification-mismatch",
            )
        elif coverage == "partial":
            require_topology(
                (state, reason) == ("unknown", "coverage-open"),
                "topology-frontier-classification-mismatch",
            )
            referenced_certificates.add(
                _coverage(reference, relation, target_kind, "partial", certificates)
            )
        else:
            require_topology(
                (state, reason)
                == ("certified-missing", "complete-coverage-no-witness"),
                "topology-frontier-classification-mismatch",
            )
            referenced_certificates.add(
                _coverage(reference, relation, target_kind, "complete", certificates)
            )

    require_topology(
        set(frontiers) == unresolved, "topology-frontier-expectation-mismatch"
    )
    require_topology(
        referenced_certificates == set(certificates),
        "topology-frontier-coverage-extraneous",
    )
