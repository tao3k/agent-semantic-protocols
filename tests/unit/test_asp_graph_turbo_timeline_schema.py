"""Schema validation tests for graph turbo timeline audit packets."""

from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

from asp_graph_turbo.artifact_events import (
    artifact_events_from_packet,
    artifact_events_packet,
    scan_artifact_events,
)
from asp_graph_turbo.artifact_timeline import (
    TimelineParameters,
    evaluate_artifact_events_timeline,
)
from unit.asp_graph_turbo_timeline_support import write_microburst_repeat_artifacts
from unit.schema_validation import schema_validator_for


_REPO_ROOT = Path(__file__).resolve().parents[2]
_EVENTS_SCHEMA = (
    _REPO_ROOT
    / "schemas"
    / "semantic-graph-turbo-artifact-events.v1.schema.json"
)
_TIMELINE_SCHEMA = (
    _REPO_ROOT
    / "schemas"
    / "semantic-graph-turbo-artifact-timeline.v1.schema.json"
)


def test_timeline_events_packet_is_schema_owned_sqlite_boundary(tmp_path) -> None:
    write_microburst_repeat_artifacts(tmp_path)
    events = scan_artifact_events(tmp_path)

    packet = artifact_events_packet(
        tmp_path,
        events,
        source_kind="db-engine",
        client_dir=str(tmp_path / "client"),
    )

    errors = list(schema_validator_for(_EVENTS_SCHEMA).iter_errors(packet))
    assert errors == []
    assert len(artifact_events_from_packet(packet)) == len(events)


def test_timeline_report_is_schema_owned_graph_turbo_output(tmp_path) -> None:
    write_microburst_repeat_artifacts(tmp_path)
    events = scan_artifact_events(tmp_path)

    report = evaluate_artifact_events_timeline(
        events,
        artifact_dir=tmp_path,
        parameters=TimelineParameters(examples=3),
    )

    errors = list(schema_validator_for(_TIMELINE_SCHEMA).iter_errors(report))
    assert errors == []
    assert report["schemaId"] == "agent.semantic-protocols.graph-turbo-artifact-timeline"
    assert report["efficiencyEstimate"]["policy"] == (
        "timeline-action-reduction-estimate"
    )


def test_timeline_cli_accepts_schema_owned_events_json(tmp_path) -> None:
    write_microburst_repeat_artifacts(tmp_path)
    events = scan_artifact_events(tmp_path)
    packet_path = tmp_path / "events.json"
    packet_path.write_text(
        json.dumps(
            artifact_events_packet(tmp_path, events, source_kind="artifact-scan"),
            sort_keys=True,
        ),
        encoding="utf-8",
    )
    env = os.environ.copy()
    env["PYTHONPATH"] = str(
        _REPO_ROOT / "packages" / "python" / "asp_graph_turbo" / "src"
    )

    completed = subprocess.run(
        [
            sys.executable,
            "-S",
            "-m",
            "asp_graph_turbo",
            "timeline",
            str(tmp_path),
            "--events-json",
            str(packet_path),
            "--format",
            "json",
        ],
        check=True,
        text=True,
        capture_output=True,
        env=env,
    )
    report = json.loads(completed.stdout)

    errors = list(schema_validator_for(_TIMELINE_SCHEMA).iter_errors(report))
    assert errors == []
    assert report["eventCount"] == len(events)
