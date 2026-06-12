"""Focused ASP graph turbo tests."""

from __future__ import annotations

from ._asp_graph_turbo_common import (
    _GRAPH_TURBO_SCHEMA,
    TypedGraph,
    rank_frontier,
    render_compact,
    result_to_packet,
    sample_failure_packet,
    schema_validator_for,
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
        "frontierActions=C1.query-code(selector=src/cache_cli/writeback.rs:10:24,owner=src/cache_cli/writeback.rs,symbol=write_prompt_output_artifact,source=H,language=rust)!query-code"
        in compact
    )
    assert packet["frontierActions"] == [
        {
            "rank": 1,
            "actionId": "C1",
            "actionKind": "query-code",
            "selector": "src/cache_cli/writeback.rs:10:24",
            "owner": "src/cache_cli/writeback.rs",
            "symbol": "write_prompt_output_artifact",
            "sourceNodeId": "hot:write",
            "next": "query-code",
            "capabilityId": "query",
            "target": "src/cache_cli/writeback.rs:10:24",
            "targetRole": "selector",
            "fields": {
                "languageId": "rust",
                "selector": "src/cache_cli/writeback.rs:10:24",
                "ownerPath": "src/cache_cli/writeback.rs",
                "symbol": "write_prompt_output_artifact",
                "sourceNodeId": "hot:write",
            },
        }
    ]
    assert (
        "queryProfiles=failure-frontier(F=>failure-facts+owners+hot-blocks),owner-query(O,K=>items+tests+dependency-usage),owner-tests(O=>covering-tests)"
        in compact
    )
    assert "\nscores=" not in compact
    assert "\npaths=" not in compact
    assert "\ncache=" not in compact
    assert "\ntrace=" not in compact
    assert "\nexplain=" not in compact
    assert "\nmetrics=" not in compact
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
