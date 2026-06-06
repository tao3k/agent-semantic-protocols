"""CLI entrypoint tests for the packaged ASP graph turbo command."""

from __future__ import annotations

import json
import subprocess
import sys

from asp_graph_turbo.graph_turbo_cli import main


def test_graph_turbo_dispatcher_help_lists_subcommands(capsys) -> None:
    assert main(["help"]) == 0

    captured = capsys.readouterr()

    assert "usage: graph-turbo <command> [args]" in captured.out
    assert "rank" in captured.out
    assert "artifacts" in captured.out
    assert "timeline" in captured.out


def test_graph_turbo_module_entrypoint_dispatches_timeline_json(tmp_path) -> None:
    completed = subprocess.run(
        [
            sys.executable,
            "-m",
            "asp_graph_turbo",
            "timeline",
            str(tmp_path),
            "--format",
            "json",
        ],
        check=True,
        text=True,
        capture_output=True,
    )
    payload = json.loads(completed.stdout)

    assert payload["schemaId"] == (
        "agent.semantic-protocols.graph-turbo-artifact-timeline"
    )
    assert payload["eventCount"] == 0
    assert payload["actionSummary"]["actionCount"] == 0
    assert payload["efficiencyEstimate"]["observedActions"] == 0
