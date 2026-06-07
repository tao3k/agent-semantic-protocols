"""Tests for the ASP graph turbo Python package."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

from asp_graph_turbo import TypedGraph, rank_frontier, render_compact, result_to_packet
from unit.schema_validation import schema_validator_for


_REPO_ROOT = Path(__file__).resolve().parents[2]
_GRAPH_TURBO_SCHEMA = (
    _REPO_ROOT / "schemas" / "semantic-graph-turbo-result.v1.schema.json"
)
_GRAPH_TURBO_REQUEST_SCHEMA = (
    _REPO_ROOT / "schemas" / "semantic-graph-turbo-request.v1.schema.json"
)
_GRAPH_TURBO_FIXTURE = (
    _REPO_ROOT / "sandtables" / "fixtures" / "asp" / "graph-turbo-owner-query.json"
)


def sample_packet() -> dict[str, object]:
    return {
        "nodes": [
            {"id": "q:parser", "kind": "query", "role": "term", "value": "parser"},
            {"id": "owner:cli", "kind": "owner", "role": "path", "value": "src/cli.py"},
            {
                "id": "item:collect",
                "kind": "item",
                "role": "fn",
                "value": "collect_actions",
                "owner": "src/cli.py",
                "path": "src/cli.py",
                "ownerPath": "src/cli.py",
                "symbol": "collect_actions",
                "startLine": 10,
                "endLine": 20,
                "locator": "src/cli.py:10:20",
            },
            {
                "id": "hot:command",
                "kind": "hot",
                "role": "call",
                "value": "command_intent",
                "owner": "src/cli.py",
                "path": "src/cli.py",
                "ownerPath": "src/cli.py",
                "symbol": "command_intent",
                "startLine": 24,
                "endLine": 28,
                "locator": "src/cli.py:24:28",
            },
            {
                "id": "dep:jsonschema",
                "kind": "dependency",
                "role": "pkg",
                "value": "jsonschema",
            },
            {
                "id": "test:cli",
                "kind": "test",
                "role": "path",
                "value": "tests/test_cli.py",
            },
        ],
        "edges": [
            {"source": "q:parser", "target": "owner:cli", "relation": "matches"},
            {"source": "q:parser", "target": "item:collect", "relation": "matches"},
            {"source": "owner:cli", "target": "item:collect", "relation": "contains"},
            {"source": "item:collect", "target": "hot:command", "relation": "contains"},
            {"source": "owner:cli", "target": "dep:jsonschema", "relation": "uses"},
            {"source": "owner:cli", "target": "test:cli", "relation": "covers"},
        ],
    }


def sample_request(
    *, profile: str = "owner-query", budget: int = 8
) -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-graph-turbo-request",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "packetKind": "graph-turbo-request",
        "profile": profile,
        "algorithm": "typed-ppr-diverse",
        "seedIds": ["q:parser", "owner:cli"],
        "budget": budget,
        "kindBudgets": {"owner": 1, "dependency": 1, "test": 1},
        "windowMerge": {"enabled": True, "maxGapLines": 8},
        "pathBudget": 4,
        "pathMaxHops": 4,
        "cache": {"enabled": True},
        "graph": sample_packet(),
    }


def sample_failure_packet() -> dict[str, object]:
    return {
        "nodes": [
            {
                "id": "failure:cache",
                "kind": "failure",
                "role": "test-failure",
                "value": "cache_cli::writeback::prompt_output_replay",
                "failureKind": "test-failure",
                "languageId": "rust",
            },
            {
                "id": "assert:replay",
                "kind": "assert",
                "role": "failure",
                "value": "expected=hit,actual=miss",
                "languageId": "rust",
            },
            {
                "id": "owner:writeback",
                "kind": "owner",
                "role": "path",
                "value": "src/cache_cli/writeback.rs",
                "path": "src/cache_cli/writeback.rs",
                "languageId": "rust",
            },
            {
                "id": "hot:write",
                "kind": "hot",
                "role": "fn",
                "value": "write_prompt_output_artifact",
                "path": "src/cache_cli/writeback.rs",
                "ownerPath": "src/cache_cli/writeback.rs",
                "symbol": "write_prompt_output_artifact",
                "startLine": 10,
                "endLine": 24,
                "locator": "src/cache_cli/writeback.rs:10:24",
                "languageId": "rust",
            },
            {
                "id": "key:fingerprint",
                "kind": "key",
                "role": "signal",
                "value": "request_fingerprint",
                "languageId": "rust",
            },
            {
                "id": "evidence:file-hash",
                "kind": "evidence",
                "role": "signal",
                "value": "file_hash(observed=failure)",
                "languageId": "rust",
            },
            {
                "id": "test:writeback",
                "kind": "test",
                "role": "path",
                "value": "tests/unit/cache_cli/writeback.rs",
                "path": "tests/unit/cache_cli/writeback.rs",
                "languageId": "rust",
            },
        ],
        "edges": [
            {
                "source": "failure:cache",
                "target": "test:writeback",
                "relation": "fails",
            },
            {
                "source": "failure:cache",
                "target": "assert:replay",
                "relation": "explains",
            },
            {
                "source": "failure:cache",
                "target": "owner:writeback",
                "relation": "selects",
            },
            {"source": "assert:replay", "target": "hot:write", "relation": "checks"},
            {
                "source": "assert:replay",
                "target": "key:fingerprint",
                "relation": "checks",
            },
            {
                "source": "assert:replay",
                "target": "evidence:file-hash",
                "relation": "gates",
            },
            {
                "source": "owner:writeback",
                "target": "hot:write",
                "relation": "contains",
            },
            {"source": "hot:write", "target": "key:fingerprint", "relation": "relates"},
            {
                "source": "hot:write",
                "target": "evidence:file-hash",
                "relation": "validates",
            },
        ],
    }


def test_request_fixture_is_schema_owned_algorithm_input() -> None:
    packet = json.loads(_GRAPH_TURBO_FIXTURE.read_text(encoding="utf-8"))
    errors = list(schema_validator_for(_GRAPH_TURBO_REQUEST_SCHEMA).iter_errors(packet))

    assert errors == []
    assert packet["packetKind"] == "graph-turbo-request"
    assert packet["algorithm"] == "typed-ppr-diverse"
    assert packet["kindBudgets"]["owner"] == 2
    assert packet["windowMerge"]["maxGapLines"] == 8


def test_owner_query_profile_masks_dependency_edges() -> None:
    graph = TypedGraph.from_packet(sample_packet())
    result = rank_frontier(
        graph, profile="owner-query", seeds=["q:parser", "owner:cli"]
    )

    ranked = [node.id for node in result.ranked_nodes]

    assert "owner:cli" in ranked
    assert "item:collect" in ranked
    assert "hot:command" in ranked
    assert "test:cli" in ranked
    assert "dep:jsonschema" not in ranked
    assert ("dependency", "deps") not in [
        (entry.node.kind, entry.action) for entry in result.frontier
    ]


def test_owner_query_ranking_prefers_rare_query_token_match_text() -> None:
    nodes: list[dict[str, object]] = [
        {
            "id": "q:vec-collection",
            "kind": "query",
            "role": "term",
            "value": "Vec collection",
        },
        {
            "id": "item:collection",
            "kind": "item",
            "role": "symbol",
            "value": "collection",
            "path": "tokio/src/loom/std/mod.rs",
            "ownerPath": "tokio/src/loom/std/mod.rs",
            "symbol": "collection",
            "matchText": "collection helpers for loom std",
        },
    ]
    edges: list[dict[str, str]] = [
        {
            "source": "q:vec-collection",
            "target": "item:collection",
            "relation": "matches",
        }
    ]
    for index in range(8):
        node_id = f"item:vec-{index}"
        nodes.append(
            {
                "id": node_id,
                "kind": "item",
                "role": "symbol",
                "value": "vec",
                "path": f"tokio/src/fs/file_{index}.rs",
                "ownerPath": f"tokio/src/fs/file_{index}.rs",
                "symbol": "vec",
                "matchText": "let buffer: Vec<u8> = Vec::new();",
            }
        )
        edges.append(
            {
                "source": "q:vec-collection",
                "target": node_id,
                "relation": "matches",
            }
        )
    graph = TypedGraph.from_packet(
        {
            "nodes": nodes,
            "edges": edges,
        }
    )
    result = rank_frontier(
        graph,
        profile="owner-query",
        seeds=["q:vec-collection"],
        limit=4,
        kind_budgets={"query": 1, "item": 3},
    )

    ranked_items = [node.id for node in result.ranked_nodes if node.kind == "item"]

    assert ranked_items[0] == "item:collection"
    assert ranked_items[1].startswith("item:vec-")


def test_owner_query_projects_typed_collection_field_selector() -> None:
    graph = TypedGraph.from_packet(
        {
            "nodes": [
                {
                    "id": "q:vec-fields",
                    "kind": "query",
                    "role": "term",
                    "value": "Vec scalar collection fields",
                },
                {
                    "id": "owner:state",
                    "kind": "owner",
                    "role": "path",
                    "value": "src/state.rs",
                    "path": "src/state.rs",
                },
                {
                    "id": "item:vec",
                    "kind": "item",
                    "role": "symbol",
                    "value": "vec",
                    "path": "src/state.rs",
                    "ownerPath": "src/state.rs",
                    "symbol": "vec",
                    "locator": "src/state.rs:30:30",
                    "matchText": "let values = Vec::new();",
                },
                {
                    "id": "field:scalars",
                    "kind": "field",
                    "role": "struct-field",
                    "value": "scalars: Vec<Scalar>",
                    "path": "src/state.rs",
                    "ownerPath": "src/state.rs",
                    "symbol": "scalars",
                    "locator": "src/state.rs:12:12",
                    "matchText": "pub scalars: Vec<Scalar>,",
                    "fields": {
                        "fieldName": "scalars",
                        "typeName": "Vec",
                        "typeValue": "Vec<Scalar>",
                        "collectionKind": "Vec",
                        "elementShape": "scalar",
                    },
                },
                {
                    "id": "type:scalars",
                    "kind": "type",
                    "role": "field-type",
                    "value": "Vec<Scalar>",
                    "path": "src/state.rs",
                    "ownerPath": "src/state.rs",
                    "symbol": "Vec",
                    "locator": "src/state.rs:12:12",
                    "fields": {
                        "fieldName": "scalars",
                        "typeName": "Vec",
                        "typeValue": "Vec<Scalar>",
                        "collectionKind": "Vec",
                        "elementShape": "scalar",
                    },
                },
                {
                    "id": "collection:vec",
                    "kind": "collection",
                    "role": "family",
                    "value": "Vec",
                    "symbol": "Vec",
                    "fields": {"collectionKind": "Vec"},
                    "action": "evidence",
                },
                {
                    "id": "hot:scalars",
                    "kind": "hot",
                    "role": "field-range",
                    "value": "scalars",
                    "path": "src/state.rs",
                    "ownerPath": "src/state.rs",
                    "symbol": "scalars",
                    "locator": "src/state.rs:4:24",
                    "matchText": "pub scalars: Vec<Scalar>,",
                },
            ],
            "edges": [
                {
                    "source": "q:vec-fields",
                    "target": "owner:state",
                    "relation": "matches",
                },
                {"source": "q:vec-fields", "target": "item:vec", "relation": "matches"},
                {
                    "source": "q:vec-fields",
                    "target": "field:scalars",
                    "relation": "matches",
                },
                {
                    "source": "q:vec-fields",
                    "target": "type:scalars",
                    "relation": "matches",
                },
                {
                    "source": "q:vec-fields",
                    "target": "collection:vec",
                    "relation": "matches",
                },
                {
                    "source": "owner:state",
                    "target": "field:scalars",
                    "relation": "contains",
                },
                {
                    "source": "field:scalars",
                    "target": "type:scalars",
                    "relation": "has_type",
                },
                {
                    "source": "field:scalars",
                    "target": "collection:vec",
                    "relation": "collection_of",
                },
                {
                    "source": "field:scalars",
                    "target": "hot:scalars",
                    "relation": "contains",
                },
            ],
        }
    )
    result = rank_frontier(
        graph,
        profile="owner-query",
        seeds=["q:vec-fields"],
        limit=7,
        kind_budgets={
            "query": 1,
            "owner": 1,
            "item": 1,
            "field": 1,
            "type": 1,
            "collection": 1,
            "hot": 1,
        },
    )
    compact = render_compact(result)

    assert (
        "F=field:struct-field(scalars: Vec<Scalar>)@src/state.rs:12:12!code" in compact
    )
    assert "Y=type:field-type(Vec<Scalar>)@src/state.rs:12:12!code" in compact
    assert "C=collection:family(Vec)!evidence" in compact
    assert (
        "queryCoverage=matched=vec,scalar,collection,fields missing=- source=ranked-frontier"
        in compact
    )
    assert (
        "S1.selector(selector=src/state.rs:4:24,owner=src/state.rs,symbol=scalars,source=H)!query-selector"
        in compact
    )


def test_query_deps_profile_can_cross_dependency_edges() -> None:
    graph = TypedGraph.from_packet(sample_packet())
    result = rank_frontier(graph, profile="query-deps", seeds=["q:parser"])

    ranked = [node.id for node in result.ranked_nodes]

    assert "dep:jsonschema" in ranked


def test_owner_query_field_selector_uses_context_locator_without_ranked_hot_node() -> None:
    graph = TypedGraph.from_packet(
        {
            "nodes": [
                {
                    "id": "q:vec-fields",
                    "kind": "query",
                    "role": "term",
                    "value": "Vec scalar collection fields",
                },
                {
                    "id": "field:scalars",
                    "kind": "field",
                    "role": "struct-field",
                    "value": "scalars: Vec<Scalar>",
                    "path": "src/state.rs",
                    "ownerPath": "src/state.rs",
                    "symbol": "scalars",
                    "locator": "src/state.rs:12:12",
                    "fields": {
                        "fieldName": "scalars",
                        "typeName": "Vec",
                        "typeValue": "Vec<Scalar>",
                        "collectionKind": "Vec",
                        "elementShape": "scalar",
                        "contextLocator": "src/state.rs:4:24",
                    },
                },
            ],
            "edges": [
                {
                    "source": "q:vec-fields",
                    "target": "field:scalars",
                    "relation": "matches",
                },
            ],
        }
    )
    result = rank_frontier(
        graph,
        profile="owner-query",
        seeds=["q:vec-fields"],
        kind_budgets={"query": 1, "field": 1},
    )
    compact = render_compact(result)

    assert (
        "F=field:struct-field(scalars: Vec<Scalar>)@src/state.rs:12:12!code" in compact
    )
    assert (
        "S1.selector(selector=src/state.rs:4:24,owner=src/state.rs,symbol=scalars,source=F)!query-selector"
        in compact
    )


def test_compact_render_uses_asp_graph_frontier_contract() -> None:
    graph = TypedGraph.from_packet(sample_packet())
    result = rank_frontier(
        graph, profile="owner-query", seeds=["q:parser", "owner:cli"]
    )

    compact = render_compact(result)

    assert compact.startswith(
        "[graph-frontier] profile=owner-query alg=typed-ppr-diverse seed=Q,O budget=8\n"
    )
    assert (
        "legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next"
        in compact
    )
    assert "aliases=G:graph" in compact
    assert "Q=query:term(parser)!fzf" in compact
    assert "I=item:fn(collect_actions)@src/cli.py:10:20!code" in compact
    assert "H=hot:call(command_intent)@src/cli.py:24:28!code" in compact
    assert "G>{" in compact and "Q:matches" in compact and "O:selects" in compact
    assert "Q>{I:matches}" in compact
    assert "O>{" in compact and "T:covers" in compact
    assert "\nrank=" in compact
    assert "\nfrontier=" in compact
    assert "\nscores=" in compact
    assert "Q:" in compact and "O:" in compact and "T:" in compact
    assert (
        "\nprofiles=owner-query,query-deps,owner-tests,prime,read-frontier,failure-frontier\n"
        in compact
    )
    assert "\nomit=code,full-score-vector,full-graph\n" in compact
    assert "\navoid=raw-read,repeat-owner,broad-fzf\n" in compact
    assert (
        "\npipeChoice=bounded-fanout maxBranches=3 repeat=false owner=asp-graph-turbo\n"
        in compact
    )
    assert (
        "\npipePolicy=maxSearchPipe=1 rewrite=false branchRepeat=false stopAfterProjectedBranches=true missingTokenSearch=false postProjectionSearch=false\n"
        in compact
    )
    assert (
        "\nselectorPolicy=run-first reason=exact-selector-present before=search-reasoning\n"
        in compact
    )
    assert (
        "\nqueryCoverage=matched=- missing=parser source=ranked-frontier\n" in compact
    )
    assert (
        "frontierActions=S1.selector(selector=src/cli.py:10:20,owner=src/cli.py,symbol=collect_actions,source=I)!query-selector"
        in compact
    )
    assert (
        "R1.reasoning(owner=src/cli.py,source=I)!search-reasoning"
        in compact
    )
    assert compact.index("S1.selector(") < compact.index("R1.reasoning(")
    assert "R4.reasoning" not in compact
    assert "[graph-turbo]" not in compact
    assert "aliases:" not in compact


def test_owner_query_projection_prefers_symbol_diverse_branches() -> None:
    graph = TypedGraph.from_packet(
        {
            "nodes": [
                {
                    "id": "q:vec-fields",
                    "kind": "query",
                    "role": "term",
                    "value": "Vec collection fields",
                },
                {
                    "id": "item:fields",
                    "kind": "item",
                    "role": "symbol",
                    "value": "fields",
                    "path": "tokio/src/io/driver/scheduled_io.rs",
                    "ownerPath": "tokio/src/io/driver/scheduled_io.rs",
                    "symbol": "fields",
                    "startLine": 480,
                    "endLine": 480,
                    "locator": "tokio/src/io/driver/scheduled_io.rs:480:480",
                    "matchText": "access the waker fields",
                },
                {
                    "id": "item:collection-a",
                    "kind": "item",
                    "role": "symbol",
                    "value": "collection",
                    "weight": 1.5,
                    "path": "tokio/src/loom/std/mod.rs",
                    "ownerPath": "tokio/src/loom/std/mod.rs",
                    "symbol": "collection",
                    "startLine": 29,
                    "endLine": 29,
                    "locator": "tokio/src/loom/std/mod.rs:29:29",
                    "matchText": "collection implementation",
                },
                {
                    "id": "item:collection-b",
                    "kind": "item",
                    "role": "symbol",
                    "value": "collection",
                    "weight": 1.4,
                    "path": "tokio/src/process/mod.rs",
                    "ownerPath": "tokio/src/process/mod.rs",
                    "symbol": "collection",
                    "startLine": 315,
                    "endLine": 315,
                    "locator": "tokio/src/process/mod.rs:315:315",
                    "matchText": "collection of child processes",
                },
                {
                    "id": "item:vec",
                    "kind": "item",
                    "role": "symbol",
                    "value": "vec",
                    "path": "stress-test/examples/simple_echo_tcp.rs",
                    "ownerPath": "stress-test/examples/simple_echo_tcp.rs",
                    "symbol": "vec",
                    "startLine": 131,
                    "endLine": 131,
                    "locator": "stress-test/examples/simple_echo_tcp.rs:131:131",
                    "matchText": "Vec buffer",
                },
            ],
            "edges": [
                {
                    "source": "q:vec-fields",
                    "target": "item:fields",
                    "relation": "matches",
                },
                {
                    "source": "q:vec-fields",
                    "target": "item:collection-a",
                    "relation": "matches",
                },
                {
                    "source": "q:vec-fields",
                    "target": "item:collection-b",
                    "relation": "matches",
                },
                {"source": "q:vec-fields", "target": "item:vec", "relation": "matches"},
            ],
        }
    )
    result = rank_frontier(
        graph,
        profile="owner-query",
        seeds=["q:vec-fields"],
        limit=5,
        kind_budgets={"query": 1, "item": 4},
    )
    compact = render_compact(result)
    frontier_actions = next(
        line for line in compact.splitlines() if line.startswith("frontierActions=")
    )

    assert "symbol=fields" in frontier_actions
    assert frontier_actions.count("symbol=collection") == 1
    assert "symbol=vec" in frontier_actions


def test_owner_query_projection_stops_after_provider_field_branches() -> None:
    graph = TypedGraph.from_packet(
        {
            "nodes": [
                {
                    "id": "q:vec-fields",
                    "kind": "query",
                    "role": "term",
                    "value": "Vec collection fields",
                },
                {
                    "id": "field:snapshot-scalars",
                    "kind": "field",
                    "role": "struct-field",
                    "value": "scalars: Vec<Scalar>",
                    "path": "src/lib.rs",
                    "ownerPath": "src/lib.rs",
                    "symbol": "scalars",
                    "startLine": 3,
                    "endLine": 3,
                    "locator": "src/lib.rs:3:3",
                    "matchText": "Snapshot::scalars: Vec<Scalar>",
                    "fields": {
                        "containerName": "Snapshot",
                        "fieldName": "scalars",
                        "typeName": "Vec",
                        "typeValue": "Vec<Scalar>",
                        "collectionKind": "Vec",
                    },
                },
                {
                    "id": "type:snapshot-scalars-vec",
                    "kind": "type",
                    "role": "field-type",
                    "value": "Vec<Scalar>",
                    "path": "src/lib.rs",
                    "ownerPath": "src/lib.rs",
                    "symbol": "Vec",
                    "startLine": 3,
                    "endLine": 3,
                    "locator": "src/lib.rs:3:3",
                    "fields": {
                        "fieldName": "scalars",
                        "typeName": "Vec",
                        "typeValue": "Vec<Scalar>",
                        "collectionKind": "Vec",
                    },
                },
                {
                    "id": "collection:vec",
                    "kind": "collection",
                    "role": "family",
                    "value": "Vec",
                    "symbol": "Vec",
                },
                {
                    "id": "item:collection",
                    "kind": "item",
                    "role": "symbol",
                    "value": "collection",
                    "path": "src/lib.rs",
                    "ownerPath": "src/lib.rs",
                    "symbol": "collection",
                    "startLine": 4,
                    "endLine": 4,
                    "locator": "src/lib.rs:4:4",
                    "matchText": "lookup: HashMap<String, Scalar>",
                },
                {
                    "id": "item:vec",
                    "kind": "item",
                    "role": "symbol",
                    "value": "vec",
                    "path": "src/lib.rs",
                    "ownerPath": "src/lib.rs",
                    "symbol": "vec",
                    "startLine": 5,
                    "endLine": 5,
                    "locator": "src/lib.rs:5:5",
                    "matchText": "cursor: Cursor<Vec<u8>>",
                },
            ],
            "edges": [
                {
                    "source": "q:vec-fields",
                    "target": "field:snapshot-scalars",
                    "relation": "matches",
                },
                {
                    "source": "q:vec-fields",
                    "target": "type:snapshot-scalars-vec",
                    "relation": "matches",
                },
                {
                    "source": "q:vec-fields",
                    "target": "collection:vec",
                    "relation": "matches",
                },
                {
                    "source": "q:vec-fields",
                    "target": "item:collection",
                    "relation": "matches",
                },
                {"source": "q:vec-fields", "target": "item:vec", "relation": "matches"},
                {
                    "source": "field:snapshot-scalars",
                    "target": "type:snapshot-scalars-vec",
                    "relation": "has_type",
                },
                {
                    "source": "field:snapshot-scalars",
                    "target": "collection:vec",
                    "relation": "collection_of",
                },
                {
                    "source": "type:snapshot-scalars-vec",
                    "target": "collection:vec",
                    "relation": "collection_of",
                },
            ],
        }
    )
    result = rank_frontier(
        graph,
        profile="owner-query",
        seeds=["q:vec-fields"],
        limit=8,
        kind_budgets={"query": 1, "field": 2, "type": 2, "collection": 2, "item": 4},
    )
    compact = render_compact(result)
    frontier_actions = next(
        line for line in compact.splitlines() if line.startswith("frontierActions=")
    )

    assert "symbol=scalars" in frontier_actions
    assert "symbol=collection" not in frontier_actions
    assert "symbol=vec" not in frontier_actions
    assert frontier_actions.count(".selector(") == 1
    ranked_ids = [node.id for node in result.ranked_nodes]
    assert ranked_ids.index("field:snapshot-scalars") < ranked_ids.index("collection:vec")


def test_owner_query_projection_dedupes_item_hot_mapped_selectors() -> None:
    graph = TypedGraph.from_packet(
        {
            "nodes": [
                {
                    "id": "q:vec",
                    "kind": "query",
                    "role": "term",
                    "value": "Vec",
                },
                {
                    "id": "item:vec",
                    "kind": "item",
                    "role": "symbol",
                    "value": "vec",
                    "path": "src/read_dir.rs",
                    "ownerPath": "src/read_dir.rs",
                    "symbol": "vec",
                    "startLine": 3,
                    "endLine": 3,
                    "locator": "src/read_dir.rs:3:3",
                },
                {
                    "id": "hot:vec",
                    "kind": "hot",
                    "role": "range",
                    "value": "vec",
                    "path": "src/read_dir.rs",
                    "ownerPath": "src/read_dir.rs",
                    "symbol": "vec",
                    "startLine": 1,
                    "endLine": 15,
                    "locator": "src/read_dir.rs:1:15",
                },
            ],
            "edges": [
                {"source": "q:vec", "target": "item:vec", "relation": "matches"},
                {"source": "item:vec", "target": "hot:vec", "relation": "contains"},
            ],
        }
    )
    result = rank_frontier(
        graph,
        profile="owner-query",
        seeds=["q:vec"],
        limit=3,
        kind_budgets={"query": 1, "item": 1, "hot": 1},
    )
    compact = render_compact(result)
    frontier_actions = next(
        line for line in compact.splitlines() if line.startswith("frontierActions=")
    )

    assert (
        "selectorPolicy=run-first reason=exact-selector-present before=search-reasoning"
        in compact
    )
    assert frontier_actions.count("selector=src/read_dir.rs:1:15") == 1
    assert frontier_actions.count(".selector(") == 1
    assert frontier_actions.index("S1.selector(") < frontier_actions.index(
        "R1.reasoning("
    )


def test_failure_frontier_profile_ranks_hot_blocks_and_renders_search_failure() -> None:
    graph = TypedGraph.from_packet(sample_failure_packet())
    result = rank_frontier(
        graph,
        profile="failure-frontier",
        seeds=["failure:cache"],
        kind_budgets={"failure": 1, "assert": 1, "hot": 1, "key": 1, "evidence": 1},
    )
    compact = render_compact(result)
    packet = result_to_packet(result)
    errors = list(schema_validator_for(_GRAPH_TURBO_SCHEMA).iter_errors(packet))
    ranked = [node.id for node in result.ranked_nodes]

    assert errors == []
    assert "assert:replay" in ranked
    assert "hot:write" in ranked
    assert "key:fingerprint" in ranked
    assert "evidence:file-hash" in ranked
    assert compact.startswith(
        "[search-failure] kind=test-failure profile=failure-frontier alg=typed-ppr-diverse seed=F budget=8\n"
    )
    assert (
        "F=failure:test-failure(cache_cli::writeback::prompt_output_replay)!failure"
        in compact
    )
    assert "A=assert:failure(expected=hit,actual=miss)!evidence" in compact
    assert (
        "H=hot:fn(write_prompt_output_artifact)@src/cache_cli/writeback.rs:10:24!code"
        in compact
    )
    assert "K=key:signal(request_fingerprint)!evidence" in compact
    assert "E=evidence:signal(file_hash(observed=failure))!evidence" in compact
    assert "\nfrontier=A.evidence,H.code,K.evidence,E.evidence\n" in compact
    assert "frontier=F.failure" not in compact
    assert "T.code" not in compact.split("\nfrontier=", 1)[1].split("\n", 1)[0]
    assert (
        "frontierActions=H.code=>asp rust query --selector src/cache_cli/writeback.rs:10:24 --code ."
        in compact
    )
    assert (
        "queryProfiles=failure-frontier(F=>failure-facts+owners+hot-blocks),owner-query(O,K=>items+tests+dependency-usage),owner-tests(O=>covering-tests)"
        in compact
    )
    assert "\nomit=full-source,unrelated-functions,wide-windows\n" in compact
    assert "\navoid=manual-window-scan,duplicate-read,raw-read,broad-fzf\n" in compact
    assert packet["profile"] == "failure-frontier"
    assert packet["omit"] == ["full-source", "unrelated-functions", "wide-windows"]
    assert packet["avoid"] == [
        "manual-window-scan",
        "duplicate-read",
        "raw-read",
        "broad-fzf",
    ]


def test_result_packet_is_schema_owned_ranking_evidence() -> None:
    graph = TypedGraph.from_packet(sample_packet())
    result = rank_frontier(
        graph,
        profile="owner-query",
        seeds=["q:parser", "owner:cli", "q:parser"],
    )
    packet = result_to_packet(result)
    errors = list(schema_validator_for(_GRAPH_TURBO_SCHEMA).iter_errors(packet))

    assert errors == []
    assert packet["schemaId"] == "agent.semantic-protocols.semantic-graph-turbo-result"
    assert packet["algorithm"] == "typed-ppr-diverse"
    assert packet["seedIds"] == ["q:parser", "owner:cli"]
    assert packet["budget"] == 8
    assert packet["kindBudgets"] == {}
    assert packet["profiles"] == [
        "owner-query",
        "query-deps",
        "owner-tests",
        "prime",
        "read-frontier",
        "failure-frontier",
    ]
    assert packet["profileCompatibility"][0]["profile"] == "owner-query"
    assert packet["profileCompatibility"][0]["compatible"] is True
    assert {
        "sourceKind": "item",
        "targetKind": "hot",
    } in packet["profileCompatibility"][0]["allowedTransitions"]
    assert {
        "sourceKind": "field",
        "targetKind": "hot",
    } in packet["profileCompatibility"][0]["allowedTransitions"]
    assert packet["profileCompatibility"][0]["kindBonus"]["field"] == 0.4
    assert packet["profileCompatibility"][0]["kindBonus"]["hot"] == 0.35
    assert packet["mergedWindows"] == []
    assert packet["sourceSinkFrontier"]["sourceIds"] == ["q:parser", "owner:cli"]
    assert any(
        path["pathKind"] == "constrained-shortest" for path in packet["typedPaths"]
    )
    assert packet["flowLite"]["rankedPathIds"][0] == packet["typedPaths"][0]["id"]
    assert packet["packetFingerprint"].startswith("sha256:")
    assert packet["graphCache"]["backend"] == "scipy-csr"
    assert packet["edges"][0]["weight"] > 0
    assert packet["algorithmTrace"][2]["step"] == "profile-policy"
    assert packet["algorithmTrace"][3]["step"] == "typed-ppr"
    assert any(
        explanation["nodeId"] == "q:parser"
        for explanation in packet["rankExplanations"]
    )
    assert packet["algorithmMetrics"]["pathCount"] == len(packet["typedPaths"])
    assert packet["rank"] == [node["id"] for node in packet["rankedNodes"]]
    assert any(entry["action"] == "fzf" for entry in packet["frontier"])
    assert packet["omit"] == ["code", "full-score-vector", "full-graph"]
    assert packet["avoid"] == ["raw-read", "repeat-owner", "broad-fzf"]


def test_tools_graph_turbo_cli_uses_request_packet_defaults(tmp_path: Path) -> None:
    packet = tmp_path / "graph.json"
    packet.write_text(
        json.dumps(sample_request(profile="query-deps", budget=4)), encoding="utf-8"
    )

    completed = subprocess.run(
        [
            sys.executable,
            "-m",
            "tools",
            "graph",
            "turbo",
            str(packet),
            "--format",
            "json",
        ],
        check=True,
        text=True,
        capture_output=True,
    )
    payload = json.loads(completed.stdout)
    errors = list(schema_validator_for(_GRAPH_TURBO_SCHEMA).iter_errors(payload))

    assert errors == []
    assert payload["packetKind"] == "graph-turbo-result"
    assert payload["profile"] == "query-deps"
    assert payload["budget"] == 4
    assert payload["kindBudgets"] == {"owner": 1, "dependency": 1, "test": 1}
    assert payload["algorithmMetrics"]["cacheStatus"] in {"hit", "miss"}
    assert payload["sourceSinkFrontier"]["sourceIds"] == ["q:parser", "owner:cli"]
    assert "dep:jsonschema" in payload["rank"]
    assert any(entry["action"] == "deps" for entry in payload["frontier"])
