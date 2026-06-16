from __future__ import annotations

from asp_graph_turbo import TypedGraph
from asp_graph_turbo.query_weights import (
    query_clause_coverage_adjustment,
    query_package_cohesion_adjustment,
)


def test_package_cohesion_uses_camelcase_query_tokens_in_paths() -> None:
    graph = TypedGraph.from_packet(
        {
            "nodes": [
                {
                    "id": "q:bytesmut",
                    "kind": "query",
                    "role": "term",
                    "value": "BytesMut reserve capacity owner",
                },
                {
                    "id": "item:other-bytesmut",
                    "kind": "item",
                    "role": "symbol",
                    "value": "BytesMut reserve capacity",
                    "path": "crates/other/src/buffer.rs",
                    "ownerPath": "crates/other/src/buffer.rs",
                    "symbol": "BytesMut",
                },
                {
                    "id": "item:bytes-path",
                    "kind": "item",
                    "role": "symbol",
                    "value": "reserve capacity owner",
                    "path": "registry/src/bytes-1.11.1/src/bytes_mut.rs",
                    "ownerPath": "registry/src/bytes-1.11.1/src/bytes_mut.rs",
                    "symbol": "reserve",
                },
            ],
            "edges": [],
        }
    )

    assert (
        query_package_cohesion_adjustment(
            graph,
            profile_name="owner-query",
            seed_ids=("q:bytesmut",),
            node=graph.nodes["item:other-bytesmut"],
        )
        < 0.0
    )
    assert (
        query_package_cohesion_adjustment(
            graph,
            profile_name="owner-query",
            seed_ids=("q:bytesmut",),
            node=graph.nodes["item:bytes-path"],
        )
        > 0.0
    )


def test_clause_coverage_splits_single_deep_natural_query() -> None:
    graph = TypedGraph.from_packet(
        {
            "nodes": [
                {
                    "id": "item:advance-owner",
                    "kind": "item",
                    "role": "symbol",
                    "value": "BufMut advance_mut unsafe trait owner",
                    "path": "registry/src/bytes-1.11.1/src/buf/buf_mut.rs",
                    "ownerPath": "registry/src/bytes-1.11.1/src/buf/buf_mut.rs",
                    "symbol": "advance_mut",
                },
                {
                    "id": "item:advance-only",
                    "kind": "item",
                    "role": "symbol",
                    "value": "BufMut advance_mut",
                    "path": "registry/src/bytes-1.11.1/src/buf/buf_mut.rs",
                    "ownerPath": "registry/src/bytes-1.11.1/src/buf/buf_mut.rs",
                    "symbol": "advance_mut",
                },
            ],
            "edges": [],
        }
    )
    single_clause = ("BufMut advance_mut unsafe trait owner boundary",)

    assert (
        query_clause_coverage_adjustment(
            profile_name="owner-query",
            query_clauses=single_clause,
            node=graph.nodes["item:advance-owner"],
        )
        > 0.0
    )
    assert (
        query_clause_coverage_adjustment(
            profile_name="owner-query",
            query_clauses=single_clause,
            node=graph.nodes["item:advance-only"],
        )
        == 0.0
    )
