from __future__ import annotations

from asp_graph_turbo import TypedGraph
from asp_graph_turbo.query_weights import (
    query_package_cohesion_adjustment,
    query_package_cohesion_tokens,
)


def test_package_cohesion_scales_with_specific_path_token_coverage() -> None:
    graph = TypedGraph.from_packet(
        {
            "nodes": [
                {
                    "id": "q:wide",
                    "kind": "query",
                    "role": "term",
                    "value": "workspace ranking schema dependency parser",
                },
                {
                    "id": "owner:workspace-ranking",
                    "kind": "owner",
                    "role": "path",
                    "value": (
                        "languages/typescript-lang-project-harness/src/cli/"
                        "semantic-search/workspace-ranking.ts"
                    ),
                    "path": (
                        "languages/typescript-lang-project-harness/src/cli/"
                        "semantic-search/workspace-ranking.ts"
                    ),
                    "ownerPath": (
                        "languages/typescript-lang-project-harness/src/cli/"
                        "semantic-search/workspace-ranking.ts"
                    ),
                },
                {
                    "id": "owner:parser",
                    "kind": "owner",
                    "role": "path",
                    "value": (
                        "languages/typescript-lang-project-harness/src/parser/"
                        "package_index.ts"
                    ),
                    "path": (
                        "languages/typescript-lang-project-harness/src/parser/"
                        "package_index.ts"
                    ),
                    "ownerPath": (
                        "languages/typescript-lang-project-harness/src/parser/"
                        "package_index.ts"
                    ),
                },
                {
                    "id": "owner:schema-main",
                    "kind": "owner",
                    "role": "path",
                    "value": (
                        "languages/typescript-lang-project-harness/schemas/"
                        "semantic-dependency-topology.v1.schema.json"
                    ),
                    "path": (
                        "languages/typescript-lang-project-harness/schemas/"
                        "semantic-dependency-topology.v1.schema.json"
                    ),
                    "ownerPath": (
                        "languages/typescript-lang-project-harness/schemas/"
                        "semantic-dependency-topology.v1.schema.json"
                    ),
                },
                {
                    "id": "owner:schema-alt-1",
                    "kind": "owner",
                    "role": "path",
                    "value": (
                        "languages/typescript-lang-project-harness/schemas/"
                        "semantic-dependency-graph.v1.schema.json"
                    ),
                    "path": (
                        "languages/typescript-lang-project-harness/schemas/"
                        "semantic-dependency-graph.v1.schema.json"
                    ),
                    "ownerPath": (
                        "languages/typescript-lang-project-harness/schemas/"
                        "semantic-dependency-graph.v1.schema.json"
                    ),
                },
                {
                    "id": "owner:schema-alt-2",
                    "kind": "owner",
                    "role": "path",
                    "value": (
                        "languages/typescript-lang-project-harness/schemas/"
                        "semantic-dependency-source.v1.schema.json"
                    ),
                    "path": (
                        "languages/typescript-lang-project-harness/schemas/"
                        "semantic-dependency-source.v1.schema.json"
                    ),
                    "ownerPath": (
                        "languages/typescript-lang-project-harness/schemas/"
                        "semantic-dependency-source.v1.schema.json"
                    ),
                },
            ],
            "edges": [],
        }
    )

    package_tokens = query_package_cohesion_tokens(graph, ("q:wide",))
    workspace_adjustment = query_package_cohesion_adjustment(
        graph,
        profile_name="owner-query",
        seed_ids=("q:wide",),
        node=graph.nodes["owner:workspace-ranking"],
        package_tokens=package_tokens,
    )
    parser_adjustment = query_package_cohesion_adjustment(
        graph,
        profile_name="owner-query",
        seed_ids=("q:wide",),
        node=graph.nodes["owner:parser"],
        package_tokens=package_tokens,
    )
    schema_adjustment = query_package_cohesion_adjustment(
        graph,
        profile_name="owner-query",
        seed_ids=("q:wide",),
        node=graph.nodes["owner:schema-main"],
        package_tokens=package_tokens,
    )

    assert workspace_adjustment > parser_adjustment > schema_adjustment
    assert schema_adjustment == 0.0


def test_package_cohesion_keeps_common_path_tokens_when_they_are_the_only_anchor() -> (
    None
):
    graph = TypedGraph.from_packet(
        {
            "nodes": [
                {
                    "id": "q:schema",
                    "kind": "query",
                    "role": "term",
                    "value": "schema dependency",
                },
                {
                    "id": "owner:schema-a",
                    "kind": "owner",
                    "role": "path",
                    "value": "schemas/semantic-dependency-topology.v1.schema.json",
                    "path": "schemas/semantic-dependency-topology.v1.schema.json",
                    "ownerPath": "schemas/semantic-dependency-topology.v1.schema.json",
                },
                {
                    "id": "owner:schema-b",
                    "kind": "owner",
                    "role": "path",
                    "value": "schemas/semantic-dependency-graph.v1.schema.json",
                    "path": "schemas/semantic-dependency-graph.v1.schema.json",
                    "ownerPath": "schemas/semantic-dependency-graph.v1.schema.json",
                },
                {
                    "id": "owner:schema-c",
                    "kind": "owner",
                    "role": "path",
                    "value": "schemas/semantic-dependency-source.v1.schema.json",
                    "path": "schemas/semantic-dependency-source.v1.schema.json",
                    "ownerPath": "schemas/semantic-dependency-source.v1.schema.json",
                },
            ],
            "edges": [],
        }
    )

    schema_adjustment = query_package_cohesion_adjustment(
        graph,
        profile_name="owner-query",
        seed_ids=("q:schema",),
        node=graph.nodes["owner:schema-a"],
    )

    assert schema_adjustment > 0.0
