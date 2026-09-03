"""Verify immutable graph indexes preserve ranking semantics and invalidate on mutation."""

from __future__ import annotations

from asp_python_graphs.backend import build_sparse_backend, multi_source_hop_lengths
from asp_python_graphs.cache import backend_fingerprint
from asp_python_graphs.model import Edge, Node, TypedGraph
from asp_python_graphs.profiles import resolve_profile
from asp_python_graphs.query_local_evidence import (
    build_local_evidence_index,
    local_evidence_adjustment,
)
from asp_python_graphs.query_topology_membership import (
    build_topology_membership_index,
    topology_membership_adjustment,
)


def _graph() -> TypedGraph:
    return TypedGraph(
        nodes=(
            Node("owner:a", "owner", "path", "src/a.py"),
            Node("item:a", "item", "function", "a"),
            Node("package:root", "package", "package", "root"),
        ),
        edges=(
            Edge("owner:a", "item:a", "contains"),
            Edge("package:root", "owner:a", "member"),
        ),
    )


def test_precompiled_indexes_preserve_direct_adjustment_semantics() -> None:
    graph = _graph()
    local_index = build_local_evidence_index(graph)
    topology_index = build_topology_membership_index(graph)

    assert local_evidence_adjustment(
        graph, profile_name="owner-query", node_id="owner:a", index=local_index
    ) == local_evidence_adjustment(graph, profile_name="owner-query", node_id="owner:a")
    assert topology_membership_adjustment(
        graph,
        profile_name="owner-query",
        node_id="owner:a",
        index=topology_index,
    ) == topology_membership_adjustment(
        graph, profile_name="owner-query", node_id="owner:a"
    )


def test_graph_mutation_invalidates_indexes_and_fingerprints() -> None:
    graph = _graph()
    profile = resolve_profile("owner-query")
    local_before = build_local_evidence_index(graph)
    fingerprint_before = backend_fingerprint(graph, profile)

    graph.add_node(Node("test:a", "test", "test", "test_a"))
    graph.add_edge(Edge("owner:a", "test:a", "covers"))

    local_after = build_local_evidence_index(graph)
    assert local_after is not local_before
    assert local_after.local_kind_count["owner:a"] == 2
    assert backend_fingerprint(graph, profile) != fingerprint_before


def test_bounded_hop_lengths_follow_the_compiled_oriented_adjacency() -> None:
    graph = _graph()
    backend = build_sparse_backend(graph, resolve_profile("owner-query"))

    depths = multi_source_hop_lengths(backend, ["owner:a"], max_depth=1)

    assert depths["owner:a"] == 0
    assert all(depth <= 1 for depth in depths.values())
    assert set(depths).issubset(
        {"owner:a", *backend.neighbors_by_id.get("owner:a", ())}
    )
