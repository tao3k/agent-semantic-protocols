"""Evidence reliability loop analysis for graph-turbo results."""

from __future__ import annotations

from collections.abc import Iterable, Mapping
from typing import Any

from .model import GraphResult, Node, OrientedEdge

_EVIDENCE_QUALITY_PROFILES = {"evidence-quality", "rust-evidence-quality"}


def evidence_reliability_report(result: GraphResult) -> dict[str, object]:
    if result.profile.name not in _EVIDENCE_QUALITY_PROFILES:
        return _empty_report(result.profile.name)

    nodes = {node.id: node for node in result.ranked_nodes}
    findings: list[dict[str, object]] = []
    for node in result.ranked_nodes:
        if node.kind == "evidence-gap":
            findings.append(_gap_finding(node))
        elif node.kind == "invariant-candidate":
            findings.extend(_invariant_findings(node, nodes, result.selected_edges))

    blocking_count = sum(1 for finding in findings if finding["blocking"])
    warning_count = sum(1 for finding in findings if finding["severity"] == "warning")
    score = max(0.0, 1.0 - (blocking_count * 0.35) - (warning_count * 0.1))
    gates = _unique(
        str(finding["action"]) for finding in findings if finding["blocking"]
    )
    return {
        "profile": result.profile.name,
        "reliable": blocking_count == 0,
        "score": round(score, 4),
        "findingCount": len(findings),
        "blockingCount": blocking_count,
        "gates": list(gates),
        "findings": findings,
    }


def _empty_report(profile_name: str) -> dict[str, object]:
    return {
        "profile": profile_name,
        "reliable": True,
        "score": 1.0,
        "findingCount": 0,
        "blockingCount": 0,
        "gates": [],
        "findings": [],
    }


def _gap_finding(node: Node) -> dict[str, object]:
    severity = _severity(node)
    return {
        "id": f"evidence-gap:{node.id}",
        "kind": "evidence-gap",
        "severity": "error" if severity in {"error", "warning"} else severity,
        "nodeId": node.id,
        "action": "collect-evidence",
        "message": node.value,
        "blocking": severity != "info",
        "evidence": _node_evidence(node),
    }


def _invariant_findings(
    node: Node,
    nodes: Mapping[str, Node],
    edges: Iterable[OrientedEdge],
) -> tuple[dict[str, object], ...]:
    findings: list[dict[str, object]] = []
    verified_receipts = _related_nodes(node.id, "verified-by", nodes, edges)
    if not any(related.kind == "verification-receipt" for related in verified_receipts):
        findings.append(
            {
                "id": f"missing-verification-receipt:{node.id}",
                "kind": "missing-verification-receipt",
                "severity": "error",
                "nodeId": node.id,
                "action": "collect-receipt",
                "message": "invariant candidate has no ranked verification receipt",
                "blocking": True,
                "evidence": _node_evidence(node),
            }
        )
    for waiver in _related_nodes(node.id, "waived-by", nodes, edges):
        findings.append(
            {
                "id": f"waiver-review:{waiver.id}",
                "kind": "waiver-review",
                "severity": "warning",
                "nodeId": waiver.id,
                "action": "review-waiver",
                "message": waiver.value,
                "blocking": False,
                "evidence": _node_evidence(waiver),
            }
        )
    for action in _related_nodes(node.id, "requires-evidence", nodes, edges):
        if action.kind != "review-action":
            continue
        findings.append(
            {
                "id": f"review-action:{action.id}",
                "kind": "review-action",
                "severity": "info",
                "nodeId": action.id,
                "action": "run-review-action",
                "message": action.value,
                "blocking": False,
                "evidence": _node_evidence(action),
            }
        )
    return tuple(findings)


def _related_nodes(
    node_id: str,
    relation: str,
    nodes: Mapping[str, Node],
    edges: Iterable[OrientedEdge],
) -> tuple[Node, ...]:
    related: list[Node] = []
    for edge in edges:
        if edge.relation != relation:
            continue
        for candidate_id in _opposite_ids(edge, node_id):
            candidate = nodes.get(candidate_id)
            if candidate is not None:
                related.append(candidate)
    return tuple(related)


def _opposite_ids(edge: OrientedEdge, node_id: str) -> tuple[str, ...]:
    ids: list[str] = []
    if edge.source == node_id:
        ids.append(edge.target)
    if edge.target == node_id:
        ids.append(edge.source)
    if edge.original_source == node_id:
        ids.append(edge.original_target)
    if edge.original_target == node_id:
        ids.append(edge.original_source)
    return tuple(_unique(ids))


def _severity(node: Node) -> str:
    severity = _field(node.fields, "severity")
    if severity in {"error", "warning", "info"}:
        return severity
    return "error"


def _node_evidence(node: Node) -> list[str]:
    evidence = [f"node:{node.id}", f"kind:{node.kind}"]
    owner_path = _field(node.fields, "ownerPath") or _field(node.fields, "path")
    if owner_path is not None:
        evidence.append(f"owner:{owner_path}")
    return evidence


def _field(fields: Mapping[str, Any], key: str) -> str | None:
    value = fields.get(key)
    if isinstance(value, str) and value:
        return value
    nested = fields.get("fields")
    if isinstance(nested, Mapping):
        value = nested.get(key)
        if isinstance(value, str) and value:
            return value
    return None


def _unique(values: Iterable[str]) -> tuple[str, ...]:
    seen: set[str] = set()
    unique_values: list[str] = []
    for value in values:
        if value in seen:
            continue
        seen.add(value)
        unique_values.append(value)
    return tuple(unique_values)
