from __future__ import annotations

import json

from asp_graph_turbo.artifact_event_model import ArtifactEvent
from asp_graph_turbo.artifact_timeline import (
    TimelineParameters,
    evaluate_artifact_events_timeline,
)
from asp_graph_turbo.artifact_timeline_text import timeline_text_lines


def test_timeline_hydrates_first_stage_topology_from_search_packet(tmp_path):
    search_dir = tmp_path / "search"
    search_dir.mkdir()
    packet_path = search_dir / "rust-pipe.json"
    packet_path.write_text(
        json.dumps(
            {
                "languageId": "rust",
                "method": "search/pipe",
                "query": "graph turbo seed topology",
                "queryQuality": "low",
                "scopeQuality": "low",
                "packageCohesion": "low",
                "risk": "single-flat-or-recall,broad-scope,package-drift",
                "ownerCandidates": [],
                "evidenceFrontier": [],
                "recommendedNext": "owner-items",
                "rankedEvidence": ["O.owner"],
            }
        ),
        encoding="utf-8",
    )
    event = ArtifactEvent(
        timestamp=1.0,
        kind="search",
        language="rust",
        method="search/pipe",
        target="",
        query="graph turbo seed topology",
        project_root=str(tmp_path),
        project_root_arg=".",
        path="search/rust-pipe.json",
        bytes=packet_path.stat().st_size,
    )

    report = evaluate_artifact_events_timeline(
        (event,),
        artifact_dir=tmp_path,
        parameters=TimelineParameters(examples=3),
        event_source="unit",
    )

    topology = report["topology"]
    assert topology["eventsWithTopology"] == 1
    assert topology["weakQueryPackEvents"] == 1
    assert topology["weakPackageCohesionEvents"] == 1
    assert topology["weakScopeEvents"] == 1
    assert topology["routeCounts"] == {"owner-items": 1}
    assert topology["missingAxisCounts"]["owner-candidates"] == 1
    assert topology["missingAxisCounts"]["query-pack"] == 1

    lines = timeline_text_lines(report)
    assert any(line.startswith("[graph-turbo-topology]") for line in lines)
    assert any(line.startswith("[graph-turbo-topology-state]") for line in lines)
