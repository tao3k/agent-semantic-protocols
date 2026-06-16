"""Generate graph-turbo ablation packet variants for sandtable calibration."""

from __future__ import annotations

import argparse
import json
import sys
from collections.abc import Iterable, Mapping, Sequence

from .cli import _load_packet
from .policy import EDGE_WEIGHT_BY_RELATION

ABLATION_VARIANTS = (
    "full",
    "no-receipt",
    "no-read-memory",
    "no-quality-fields",
    "no-provider-facts",
    "relation-weight-flat",
    "no-query-seed-prior",
    "no-package-cohesion",
    "no-query-clause-coverage",
)

_QUALITY_FIELDS = frozenset({"confidence", "freshness", "provenance"})
_PROVIDER_FACT_KINDS = frozenset(
    {"build", "collection", "field", "package", "test", "type"}
)


def main(argv: Sequence[str] | None = None) -> int:
    args = _parse_args(argv)
    packet = _load_packet(args.packet)
    variants = _selected_variants(args.variant)
    ablation_set = build_ablation_set(packet, variants)
    if args.format == "json":
        sys.stdout.write(json.dumps(ablation_set, sort_keys=True) + "\n")
    else:
        sys.stdout.write(_render_text(ablation_set) + "\n")
    return 0


def build_ablation_set(
    packet: Mapping[str, object],
    variants: Iterable[str] = ABLATION_VARIANTS,
) -> dict[str, object]:
    """Return schema-shaped ablation variants for one graph-turbo request."""

    selected = tuple(variants)
    return {
        "schemaId": "agent.semantic-protocols.semantic-graph-turbo-ablation-set",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "packetKind": "graph-turbo-ablation-set",
        "sourceProfile": packet.get("profile"),
        "variants": [
            {
                "variant": variant,
                "changes": _variant_changes(variant),
                "packet": _variant_packet(packet, variant),
            }
            for variant in selected
        ],
    }


def _variant_packet(packet: Mapping[str, object], variant: str) -> dict[str, object]:
    mutable = json.loads(json.dumps(packet))
    if variant == "full":
        return mutable
    if variant == "no-receipt":
        return _without_node_kinds(mutable, {"receipt"})
    if variant == "no-read-memory":
        mutable["readMemory"] = {"seenSelectors": []}
        return mutable
    if variant == "no-quality-fields":
        _strip_quality_fields(mutable)
        return mutable
    if variant == "no-provider-facts":
        return _without_node_kinds(mutable, _PROVIDER_FACT_KINDS)
    if variant == "relation-weight-flat":
        _flatten_relation_weights(mutable)
        return mutable
    if variant == "no-query-seed-prior":
        return _with_query_adjustment_policy(mutable, "seedPrior", False)
    if variant == "no-package-cohesion":
        return _with_query_adjustment_policy(mutable, "packageCohesion", False)
    if variant == "no-query-clause-coverage":
        return _with_query_adjustment_policy(mutable, "queryClauseCoverage", False)
    raise ValueError(f"unknown graph-turbo ablation variant: {variant}")


def _without_node_kinds(
    packet: dict[str, object],
    removed_kinds: Iterable[str],
) -> dict[str, object]:
    graph = _graph(packet)
    nodes = _items(graph, "nodes")
    removed = frozenset(removed_kinds)
    kept_nodes = [
        node
        for node in nodes
        if not (isinstance(node.get("kind"), str) and node["kind"] in removed)
    ]
    kept_ids = {str(node.get("id")) for node in kept_nodes if node.get("id")}
    graph["nodes"] = kept_nodes
    graph["edges"] = [
        edge
        for edge in _items(graph, "edges")
        if edge.get("source") in kept_ids and edge.get("target") in kept_ids
    ]
    return packet


def _strip_quality_fields(packet: dict[str, object]) -> None:
    for edge in _items(_graph(packet), "edges"):
        for field in _QUALITY_FIELDS:
            edge.pop(field, None)
        nested = edge.get("fields")
        if isinstance(nested, dict):
            for field in _QUALITY_FIELDS:
                nested.pop(field, None)


def _flatten_relation_weights(packet: dict[str, object]) -> None:
    _strip_quality_fields(packet)
    for edge in _items(_graph(packet), "edges"):
        relation = edge.get("relation")
        base_weight = (
            EDGE_WEIGHT_BY_RELATION.get(relation, 1.0)
            if isinstance(relation, str)
            else 1.0
        )
        edge["weight"] = 1.0 / base_weight if base_weight > 0.0 else 1.0


def _with_query_adjustment_policy(
    packet: dict[str, object], key: str, enabled: bool
) -> dict[str, object]:
    policy = packet.get("queryAdjustmentPolicy")
    if not isinstance(policy, dict):
        policy = {}
        packet["queryAdjustmentPolicy"] = policy
    policy[key] = enabled
    return packet


def _graph(packet: dict[str, object]) -> dict[str, object]:
    graph = packet.get("graph")
    if not isinstance(graph, dict):
        graph = {}
        packet["graph"] = graph
    return graph


def _items(graph: dict[str, object], key: str) -> list[dict[str, object]]:
    value = graph.get(key)
    if not isinstance(value, list):
        return []
    return [item for item in value if isinstance(item, dict)]


def _selected_variants(variants: Sequence[str]) -> tuple[str, ...]:
    selected = ABLATION_VARIANTS if not variants else tuple(variants)
    unknown = [variant for variant in selected if variant not in ABLATION_VARIANTS]
    if unknown:
        raise SystemExit(f"unknown ablation variant: {','.join(unknown)}")
    return selected


def _variant_changes(variant: str) -> dict[str, object]:
    return {
        "full": {"description": "unchanged graph-turbo request packet"},
        "no-receipt": {"removedNodeKinds": ["receipt"]},
        "no-read-memory": {"readMemorySeenSelectors": "cleared"},
        "no-quality-fields": {"removedEdgeFields": sorted(_QUALITY_FIELDS)},
        "no-provider-facts": {"removedNodeKinds": sorted(_PROVIDER_FACT_KINDS)},
        "relation-weight-flat": {
            "removedEdgeFields": sorted(_QUALITY_FIELDS),
            "explicitEdgeWeight": "inverse relation base weight",
        },
        "no-query-seed-prior": {
            "queryAdjustmentPolicy": {"seedPrior": False},
        },
        "no-package-cohesion": {
            "queryAdjustmentPolicy": {"packageCohesion": False},
        },
        "no-query-clause-coverage": {
            "queryAdjustmentPolicy": {"queryClauseCoverage": False},
        },
    }[variant]


def _render_text(packet: Mapping[str, object]) -> str:
    variants = packet.get("variants")
    if not isinstance(variants, list):
        return "[graph-ablation] variants=0"
    names = [
        str(variant.get("variant"))
        for variant in variants
        if isinstance(variant, Mapping)
    ]
    return f"[graph-ablation] variants={len(names)} names={','.join(names)}"


def _parse_args(argv: Sequence[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("packet", nargs="?", default="-")
    parser.add_argument("--variant", action="append", default=[])
    parser.add_argument("--format", choices=["json", "text"], default="json")
    return parser.parse_args(argv)


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
