# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

from __future__ import annotations

import argparse
from dataclasses import asdict
from typing import Any, Sequence
from asp_proofs._cli_output import write_stdout

from asp_proofs.polyglot_search_semantics import (
    GraphEdge,
    GraphNode,
    PropertyGraph,
    Relation,
    SemanticTrace,
    canonical_json,
    digest,
    execute_semantics,
)


SCHEMA_ID = "agent.semantic-protocols.polyglot-search-trace.v1"
GENERATOR_ID = "asp-proofs-polyglot-search-trace"
GENERATOR_VERSION = "1"
SOURCE_SNAPSHOT_DIGEST = "sha256:" + "a" * 64
PROVIDER_ARTIFACT_DIGEST = "sha256:" + "b" * 64
RUNTIME_ARTIFACT_DIGEST = "sha256:" + "c" * 64
GQL_SOURCE = (
    "MATCH (f:Callable)-[:OWNED_BY]->(o:Owner) "
    "WHERE f.name = 'open_runtime_project_proxy' "
    "RETURN f AS function, o AS candidate"
)
LOGIC_SOURCE = (
    "?- gql_candidate(Function, Candidate), covers(Candidate, Obligation)."
)


def reference_graph() -> PropertyGraph:
    return PropertyGraph(
        nodes=(
            GraphNode(
                "1",
                ("Callable",),
                (("name", "open_runtime_project_proxy"),),
            ),
            GraphNode("2", ("Owner",), (("name", "runtime"),)),
            GraphNode("3", ("Owner",), (("name", "fallback"),)),
            GraphNode("4", ("Other",), ()),
        ),
        edges=(
            GraphEdge("1", "1", "2", "OWNED_BY"),
            GraphEdge("2", "1", "2", "OWNED_BY"),
            GraphEdge("3", "1", "3", "OWNED_BY"),
            GraphEdge("4", "2", "4", "OWNED_BY"),
        ),
    )


def reference_relations() -> tuple[Relation, ...]:
    return (
        Relation(
            predicate="covers",
            columns=("candidate", "obligation"),
            rows=(("2", 10), ("3", 11)),
        ),
    )


def build_reference_trace() -> SemanticTrace:
    return execute_semantics(
        gql_source=GQL_SOURCE,
        logic_source=LOGIC_SOURCE,
        graph=reference_graph(),
        relations=reference_relations(),
    )


def _row_object(row: Sequence[tuple[str, Any]]) -> dict[str, Any]:
    return {name: value for name, value in row}


def trace_payload(trace: SemanticTrace | None = None) -> dict[str, Any]:
    trace = trace or build_reference_trace()
    if trace.logic_binding is None:
        raise ValueError("reference trace requires a Logic binding")
    core: dict[str, Any] = {
        "schemaId": SCHEMA_ID,
        "schemaVersion": "1",
        "generator": {"id": GENERATOR_ID, "version": GENERATOR_VERSION},
        "sourceSnapshotDigest": SOURCE_SNAPSHOT_DIGEST,
        "providerArtifactDigest": PROVIDER_ARTIFACT_DIGEST,
        "runtimeArtifactDigest": RUNTIME_ARTIFACT_DIGEST,
        "gqlProfile": "asp-gql-core:0.1-one-hop",
        "logicProfile": "asp-logic-query-core:0.1-positive-conjunction",
        "gqlSourceDigest": trace.gql_binding.source_digest,
        "gqlAstDigest": trace.gql_binding.ast_digest,
        "gqlBindingDigest": trace.gql_binding.binding_digest,
        "logicSourceDigest": trace.logic_binding.source_digest,
        "logicAstDigest": trace.logic_binding.ast_digest,
        "logicBindingDigest": trace.logic_binding.binding_digest,
        "graphDigestBefore": trace.graph_digest_before,
        "graphDigestAfter": trace.graph_digest_after,
        "registryDigest": trace.registry_digest,
        "executionBindingDigest": trace.execution_binding_digest,
        "gqlRows": [_row_object(row) for row in trace.gql_rows],
        "logicRows": [_row_object(row) for row in trace.logic_rows],
        "multiplicity": trace.multiplicity,
        "ordering": trace.ordering,
        "semanticTraceDigest": trace.trace_digest,
    }
    core["payloadDigest"] = digest(core)
    return core


def render_json_fixture() -> str:
    return canonical_json(trace_payload()) + "\n"


def _lean_string(value: str) -> str:
    return canonical_json(value)


def render_lean_fixture() -> str:
    payload = trace_payload()
    gql_rows = [
        f"({_lean_string(row['function'])}, {_lean_string(row['candidate'])})"
        for row in payload["gqlRows"]
    ]
    logic_rows = [
        "(" + ", ".join(
            (
                _lean_string(row["Function"]),
                _lean_string(row["Candidate"]),
                str(row["Obligation"]),
            )
        ) + ")"
        for row in payload["logicRows"]
    ]
    return "\n".join(
        (
            "namespace ASPProof.Generated.PolyglotSearchTraceFixture",
            "",
            f"def schemaId : String := {_lean_string(payload['schemaId'])}",
            f"def schemaVersion : String := {_lean_string(payload['schemaVersion'])}",
            f"def generatorId : String := {_lean_string(payload['generator']['id'])}",
            f"def generatorVersion : String := {_lean_string(payload['generator']['version'])}",
            f"def gqlProfile : String := {_lean_string(payload['gqlProfile'])}",
            f"def logicProfile : String := {_lean_string(payload['logicProfile'])}",
            f"def sourceSnapshotDigest : String := {_lean_string(payload['sourceSnapshotDigest'])}",
            f"def providerArtifactDigest : String := {_lean_string(payload['providerArtifactDigest'])}",
            f"def runtimeArtifactDigest : String := {_lean_string(payload['runtimeArtifactDigest'])}",
            f"def gqlSourceDigest : String := {_lean_string(payload['gqlSourceDigest'])}",
            f"def gqlASTDigest : String := {_lean_string(payload['gqlAstDigest'])}",
            f"def gqlBindingDigest : String := {_lean_string(payload['gqlBindingDigest'])}",
            f"def logicSourceDigest : String := {_lean_string(payload['logicSourceDigest'])}",
            f"def logicASTDigest : String := {_lean_string(payload['logicAstDigest'])}",
            f"def logicBindingDigest : String := {_lean_string(payload['logicBindingDigest'])}",
            f"def graphDigestBefore : String := {_lean_string(payload['graphDigestBefore'])}",
            f"def graphDigestAfter : String := {_lean_string(payload['graphDigestAfter'])}",
            f"def registryDigest : String := {_lean_string(payload['registryDigest'])}",
            f"def executionBindingDigest : String := {_lean_string(payload['executionBindingDigest'])}",
            f"def semanticTraceDigest : String := {_lean_string(payload['semanticTraceDigest'])}",
            f"def payloadDigest : String := {_lean_string(payload['payloadDigest'])}",
            f"def multiplicity : String := {_lean_string(payload['multiplicity'])}",
            f"def ordering : String := {_lean_string(payload['ordering'])}",
            "def gqlRows : List (String × String) := [" + ", ".join(gql_rows) + "]",
            "def logicRows : List (String × String × Nat) := ["
            + ", ".join(logic_rows)
            + "]",
            "",
            "end ASPProof.Generated.PolyglotSearchTraceFixture",
            "",
        )
    )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="asp-proofs-polyglot-trace-fixture")
    parser.add_argument("--format", choices=("json", "lean"), required=True)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    write_stdout(render_json_fixture() if args.format == "json" else render_lean_fixture(), end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
