"""Cross-language evidence-quality profile tests."""

from __future__ import annotations

import pytest
from asp_graph_turbo import render_compact

from ._common import (
    _GRAPH_TURBO_REQUEST_SCHEMA,
    _GRAPH_TURBO_SCHEMA,
    TypedGraph,
    rank_frontier,
    result_to_packet,
    schema_validator_for,
)
from ._language_evidence_packet import language_evidence_graph_turbo_request


@pytest.mark.parametrize(
    ("language_id", "provider_id", "namespace", "owner_path"),
    [
        (
            "python",
            "py-harness",
            "agent.semantic-protocols.languages.python.py-harness",
            "src/service.py",
        ),
        (
            "typescript",
            "ts-harness",
            "agent.semantic-protocols.languages.typescript.ts-harness",
            "src/service.ts",
        ),
        (
            "julia",
            "julia-lang-project-harness",
            "agent.semantic-protocols.languages.julia.julia-lang-project-harness",
            "src/Service.jl",
        ),
        (
            "gerbil-scheme",
            "gerbil-scheme-harness",
            "agent.semantic-protocols.languages.gerbil-scheme.gerbil-scheme-harness",
            "src/service.ss",
        ),
    ],
)
def test_evidence_quality_profile_supports_language_provider_packets(
    language_id: str,
    provider_id: str,
    namespace: str,
    owner_path: str,
) -> None:
    request = language_evidence_graph_turbo_request(
        language_id=language_id,
        provider_id=provider_id,
        namespace=namespace,
        owner_path=owner_path,
    )
    assert list(schema_validator_for(_GRAPH_TURBO_REQUEST_SCHEMA).iter_errors(request)) == []

    graph = TypedGraph.from_packet(request)
    result = rank_frontier(
        graph,
        profile="evidence-quality",
        seeds=[f"{language_id}:owner"],
        limit=8,
        cache_enabled=False,
    )
    packet = result_to_packet(result)
    compact = render_compact(result)

    assert packet["profile"] == "evidence-quality"
    assert f"{language_id}:gap:receipt" in packet["rank"]
    assert packet["evidenceReliability"]["reliable"] is False
    assert packet["evidenceReliability"]["gates"] == ["collect-evidence"]
    assert "reliability=fail" in compact
    assert "collect-evidence" in compact
    assert list(schema_validator_for(_GRAPH_TURBO_SCHEMA).iter_errors(packet)) == []
