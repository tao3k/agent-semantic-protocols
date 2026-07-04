"""Run the AXLE proof-obligation demo for ASP branch legality."""

from __future__ import annotations

import argparse
import asyncio
import json
import sys
from dataclasses import asdict
from datetime import UTC, datetime
from pathlib import Path

from axle import AxleClient
from axle.exceptions import AxleApiError

from .console import emit
from .lean_axle_proof_demo_artifacts import (
    build_obligation,
    build_receipt,
    build_recipe,
    build_report,
)
from .lean_axle_proof_demo_io import (
    detect_local_lean_environment,
    materialize_proof_inputs,
    write_packet_projection,
    write_schema_projection,
    write_planning_artifacts,
    write_verification_artifacts,
)
from .semantic_search_packet_fixture_projection import (
    build_semantic_search_packet_fixture_projection,
)
from .semantic_search_packet_projection import build_semantic_search_packet_projection


async def verify_with_axle(
    formal_statement: str,
    candidate_proof: str,
    environment: str,
    timeout_seconds: float,
) -> dict:
    async with AxleClient() as client:
        response = await client.verify_proof(
            formal_statement=formal_statement,
            content=candidate_proof,
            environment=environment,
            ignore_imports=True,
            timeout_seconds=timeout_seconds,
        )
    return asdict(response)


async def run_demo(
    out_dir: Path,
    environment: str,
    timeout_seconds: float,
    search_packet_schema: Path,
    search_packet: Path,
    packet_source_kind: str,
) -> int:
    projection = build_semantic_search_packet_projection(search_packet_schema)
    schema_projection = {
        "sourceSchema": projection.source_schema,
        "formalLeanPath": "schema-projection-formal.lean",
        "candidateLeanPath": "schema-projection-candidate.lean",
        "facts": projection.facts,
    }
    write_schema_projection(
        out_dir,
        schema_projection,
        projection.formal_lean,
        projection.candidate_lean,
    )
    packet_projection_result = build_semantic_search_packet_fixture_projection(
        search_packet,
        source_kind=packet_source_kind,
    )
    packet_projection = {
        "sourceKind": packet_projection_result.source_kind,
        "sourcePacket": packet_projection_result.source_packet,
        "formalLeanPath": "packet-projection-formal.lean",
        "candidateLeanPath": "packet-projection-candidate.lean",
        "identityKind": packet_projection_result.identity_kind,
        "contractValid": packet_projection_result.contract_valid,
        "facts": packet_projection_result.facts,
    }
    write_packet_projection(
        out_dir,
        packet_projection,
        packet_projection_result.formal_lean,
        packet_projection_result.candidate_lean,
    )
    formal, candidate, statement_path, proof_path = materialize_proof_inputs(
        out_dir,
        projection.formal_lean,
        projection.candidate_lean,
        extra_formal_lean=[packet_projection_result.formal_lean],
        extra_candidate_lean=[packet_projection_result.candidate_lean],
    )
    obligation = build_obligation(datetime.now(UTC).isoformat())
    recipe = build_recipe(
        environment,
        timeout_seconds,
        str(statement_path),
        str(proof_path),
        formal,
        candidate,
    )
    write_planning_artifacts(out_dir, obligation, recipe)
    response_json = await verify_with_axle(formal, candidate, environment, timeout_seconds)
    receipt = build_receipt(
        obligation,
        recipe,
        environment,
        formal,
        candidate,
        response_json,
        schema_projection,
        packet_projection,
    )
    report = build_report(receipt)
    write_verification_artifacts(out_dir, receipt, report, response_json)
    emit(json.dumps(receipt, indent=2, sort_keys=True))
    return 0 if receipt["okay"] else 1


def parse_args() -> argparse.Namespace:
    local_env = detect_local_lean_environment()
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--out-dir",
        type=Path,
        default=Path(".data/asp-formal-verification-demo/search-packet-selector-identity"),
    )
    parser.add_argument(
        "--environment",
        default=local_env or "lean-4.31.0",
        help="AXLE Lean environment, for example lean-4.31.0",
    )
    parser.add_argument("--timeout-seconds", type=float, default=120)
    parser.add_argument(
        "--search-packet-schema",
        type=Path,
        default=Path("schemas/semantic-search-packet.v1.schema.json"),
    )
    parser.add_argument(
        "--search-packet",
        "--search-packet-fixture",
        dest="search_packet",
        type=Path,
        default=Path(
            "tests/fixtures/semantic_search_packet/bad_path_line_identity_packet.json"
        ),
        help="Semantic-search-packet JSON path from a contract fixture or provider output.",
    )
    parser.add_argument(
        "--packet-source-kind",
        choices=["contract-fixture", "provider-output"],
        default="contract-fixture",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        return asyncio.run(
            run_demo(
                args.out_dir,
                args.environment,
                args.timeout_seconds,
                args.search_packet_schema,
                args.search_packet,
                args.packet_source_kind,
            )
        )
    except AxleApiError as exc:
        emit(f"AXLE API error: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
