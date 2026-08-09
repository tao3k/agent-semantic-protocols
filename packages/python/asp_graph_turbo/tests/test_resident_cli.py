from __future__ import annotations

import importlib.metadata
import io
import json
import sys

from asp_graph_turbo import resident_cli


def test_standalone_resident_entrypoint_owns_the_v1_ipc_lifecycle(monkeypatch) -> None:
    entrypoints = {
        entry.name: entry.value
        for entry in importlib.metadata.entry_points(group="console_scripts")
    }
    assert entrypoints["asp-graph-turbo-resident"] == "asp_graph_turbo.resident_cli:main"

    runtime_digest = f"blake3-256:{'a' * 64}"
    command_digest = f"blake3-256:{'b' * 64}"
    request = {
        "schemaId": "agent.semantic-protocols.graph-turbo-resident-server",
        "schemaVersion": "1",
        "messageKind": "hello",
        "requestId": 1,
        "runtimeArtifactDigest": runtime_digest,
        "executionCommandDigest": command_digest,
    }
    stdout = io.StringIO()
    monkeypatch.setattr(sys, "stdin", io.StringIO(json.dumps(request) + "\n"))
    monkeypatch.setattr(sys, "stdout", stdout)

    assert resident_cli.main() == 0
    receipt = json.loads(stdout.getvalue())
    assert receipt["messageKind"] == "receipt"
    assert receipt["state"] == "ready"
    assert receipt["processIdentity"]["runtimeArtifactDigest"] == runtime_digest
    assert receipt["processIdentity"]["executionCommandDigest"] == command_digest
