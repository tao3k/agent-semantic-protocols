"""Time-window filter tests for ASP graph turbo artifact timelines."""

from __future__ import annotations

from datetime import datetime

from asp_graph_turbo.artifact_timeline import (
    TimelineParameters,
    evaluate_artifact_timeline,
)
from unit.asp_graph_turbo_timeline_support import write_timeline_json


def test_timeline_filters_since_and_recent_sessions(tmp_path) -> None:
    search_dir = tmp_path / "search"
    search_dir.mkdir()
    _write_filter_artifacts(search_dir)

    since_report = evaluate_artifact_timeline(
        tmp_path,
        parameters=TimelineParameters(session_gap_seconds=100, since_timestamp=1500),
    )
    recent_report = evaluate_artifact_timeline(
        tmp_path,
        parameters=TimelineParameters(session_gap_seconds=100, recent_sessions=1),
    )

    for report in (since_report, recent_report):
        assert report["eventCount"] == 2
        assert report["sessionCount"] == 1
        assert report["repeatSearches"] == 1
        assert report["promotableTypedFrontierSearches"] == 0
        assert report["collapsibleOwnerSearches"] == 1
        assert report["repeatGroups"][0]["method"] == "search/owner"
        assert report["repeatGroups"][0]["subject"] == "src/lib.rs"

    assert since_report["parameters"]["since"] == datetime.fromtimestamp(
        1500
    ).isoformat(timespec="seconds")
    assert recent_report["parameters"]["recentSessions"] == 1


def _write_filter_artifacts(search_dir) -> None:
    write_timeline_json(
        search_dir / "python-search-typed-frontier-a.json",
        _packet("python", "search/typed-frontier", query="semantic type"),
        mtime=1000,
    )
    write_timeline_json(
        search_dir / "python-search-typed-frontier-b.json",
        _packet("python", "search/typed-frontier", query="semantic type"),
        mtime=1010,
    )
    write_timeline_json(
        search_dir / "rust-search-owner-a.json",
        _packet("rust", "search/owner", owner="src/lib.rs"),
        mtime=2000,
    )
    write_timeline_json(
        search_dir / "rust-search-owner-b.json",
        _packet("rust", "search/owner", owner="src/lib.rs"),
        mtime=2010,
    )


def _packet(
    language: str, method: str, *, query: str = "", owner: str = ""
) -> dict[str, object]:
    packet = {
        "schemaId": "agent.semantic-protocols.semantic-search-packet",
        "languageId": language,
        "method": method,
    }
    if query:
        packet["query"] = query
    if owner:
        packet["ownerPath"] = owner
    return packet
