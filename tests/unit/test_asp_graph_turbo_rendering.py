"""Focused ASP graph turbo tests."""

from __future__ import annotations

from ._asp_graph_turbo_common import (
    TypedGraph,
    rank_frontier,
    render_compact,
    sample_packet,
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
    assert "Q>{O:matches,I:matches}" in compact
    assert "O>{" in compact and "T:covers" in compact
    assert "\nrank=" in compact
    assert "\nfrontier=" in compact
    assert "\nscores=" in compact
    assert "Q:" in compact and "O:" in compact and "T:" in compact
    profiles_line = next(
        line for line in compact.splitlines() if line.startswith("profiles=")
    )
    for profile in (
        "owner-query",
        "query-deps",
        "owner-tests",
        "prime",
        "read-frontier",
        "failure-frontier",
        "field-impact",
        "type-impact",
        "collection-impact",
        "failure-evidence",
        "test-selection",
        "affected",
    ):
        assert profile in profiles_line
    assert "\nomit=code,full-score-vector,full-graph\n" in compact
    assert "\navoid=raw-read,repeat-owner,broad-fzf,manual-window-scan\n" in compact
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
    assert "R1.reasoning(owner=src/cli.py,source=I)!search-reasoning" in compact
    assert compact.index("S1.selector(") < compact.index("R1.reasoning(")
    assert "R4.reasoning" not in compact
    assert "[graph-turbo]" not in compact
    assert "aliases:" not in compact
