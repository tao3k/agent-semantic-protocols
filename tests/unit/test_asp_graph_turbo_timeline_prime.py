"""Prime suppression timeline tests for ASP graph turbo artifacts."""

from __future__ import annotations

from asp_graph_turbo.artifact_timeline import (
    TimelineParameters,
    evaluate_artifact_timeline,
)
from unit.asp_graph_turbo_timeline_support import write_timeline_prime


def test_timeline_reports_same_session_prime_suppression(tmp_path) -> None:
    search_dir = tmp_path / "search"
    search_dir.mkdir()
    write_timeline_prime(search_dir / "rust-search-prime-a.json", mtime=2000)
    write_timeline_prime(search_dir / "rust-search-prime-b.json", mtime=2010)
    write_timeline_prime(search_dir / "rust-search-prime-c.json", mtime=2200)

    report = evaluate_artifact_timeline(
        tmp_path,
        parameters=TimelineParameters(session_gap_seconds=100),
    )

    suppression = report["primeSuppression"]
    assert report["sessionCount"] == 2
    assert report["suppressiblePrimeSearches"] == 1
    assert suppression["policy"] == "same-session-fresh-prime"
    assert suppression["suppressibleSearches"] == 1
    assert suppression["candidateGroupCount"] == 1
    assert suppression["candidateGroups"][0]["count"] == 2
    assert suppression["candidateGroups"][0]["subject"] == "src/lib.rs"
    assert suppression["actionCount"] == 1
    assert suppression["actions"][0]["decision"] == "suppress"
    assert suppression["actions"][0]["replacement"] == "reuse-prime-frontier"
    assert suppression["actions"][0]["ageSeconds"] == 10
    assert suppression["actions"][0]["avoidCommand"] == (
        "asp rust search prime --view seeds ."
    )
