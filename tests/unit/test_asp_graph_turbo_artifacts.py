"""Artifact evaluation tests for ASP graph turbo."""

from __future__ import annotations

import json

from asp_graph_turbo.artifacts import (
    evaluate_search_artifacts,
    search_packet_to_graph_turbo_request,
)


def search_packet() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-search-packet",
        "languageId": "python",
        "method": "search/lexical",
        "view": "lexical",
        "query": "semantic_string_type",
        "searchSynthesis": {
            "algorithm": "seed-frontier",
            "scope": "lexical",
            "seeds": [
                {"kind": "owner", "target": "src/types.py"},
                {"kind": "owner", "target": "src/types.py"},
                {"kind": "owner", "target": "src/types.py"},
                {"kind": "tests", "target": "tests/test_types.py"},
            ],
            "windowSet": [
                {"kind": "owner", "target": "src/types.py"},
                {"kind": "tests", "target": "tests/test_types.py"},
            ],
        },
    }


def test_search_packet_can_be_converted_to_graph_turbo_request() -> None:
    request = search_packet_to_graph_turbo_request(search_packet(), budget=6)

    assert request is not None
    assert request["profile"] == "query-deps"
    assert request["budget"] == 6
    assert request["seedIds"]
    assert len(request["graph"]["nodes"]) == 3
    assert any(edge["relation"] == "covers" for edge in request["graph"]["edges"])


def test_artifact_evaluation_reports_cache_and_duplicate_metrics(tmp_path) -> None:
    search_dir = tmp_path / "search"
    search_dir.mkdir()
    prompt_dir = tmp_path / "prompt-output"
    prompt_dir.mkdir()
    (search_dir / "python-search-lexical-test.json").write_text(
        json.dumps(search_packet()),
        encoding="utf-8",
    )
    (prompt_dir / "python-owner.command.json").write_text(
        json.dumps(
            {
                "schemaId": "agent.semantic-protocols.client-prompt-output-command",
                "providerCommands": [
                    {
                        "argv": [
                            "py-harness",
                            "search",
                            "owner",
                            "src/" + "types.py",
                            "items",
                        ],
                        "languageId": "python",
                    }
                ],
            }
        ),
        encoding="utf-8",
    )

    report = evaluate_search_artifacts(tmp_path, budget=6)

    assert report["scanned"] == 1
    assert report["converted"] == 1
    assert report["secondPassCacheHits"] == 1
    assert report["averages"]["inputMaxDuplicate"] > report["averages"]["rankedMaxDuplicate"]
    coverage = report["historicalCommandCoverage"]
    assert coverage["labelCount"] == 1
    assert coverage["measurableLabels"] == 1
    assert coverage["coveredLabels"] == 1
    assert coverage["top5"] == 1
