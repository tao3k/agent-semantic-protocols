"""ASP command read-loop metrics for sandtable agent observations."""

from __future__ import annotations

from tools.semantic_sandtable.agent_observation_read_loop import (
    read_loop_memory,
    read_loop_stats,
)


def test_read_loop_counts_language_query_selector_code_duplicates() -> None:
    stats = read_loop_stats(
        [
            "asp rust query --selector tokio/src/io/blocking.rs:15:35 --code .",
            "asp rust query --selector tokio/src/io/blocking.rs:15:35 --code --context 30 .",
        ]
    )

    assert stats["readLoopDirectCodeCommands"] == 2
    assert stats["readLoopDuplicateSelectors"] == 1
    assert stats["readLoopAdjacentRangeWindows"] == 0
    assert stats["readLoopSameOwnerScans"] == 0


def test_read_loop_ignores_metadata_selector_queries() -> None:
    stats = read_loop_stats(
        [
            "asp org query --selector docs/plan.org:1-10 --view metadata .",
            "asp rust query --selector src/lib.rs:1:20 .",
        ]
    )

    assert stats["readLoopDirectCodeCommands"] == 0
    assert stats["readLoopDuplicateSelectors"] == 0


def test_read_loop_memory_records_selector_fingerprints_and_suppression() -> None:
    command = "asp rust query --selector src/lib.rs:10:20 --code ."
    memory = read_loop_memory(
        [
            command,
            "asp rust query --selector src/lib.rs:10:20 --code --context 30 .",
            "asp rust query --selector src/lib.rs:21:28 --code .",
        ],
        [
            {
                "command": command,
                "outputBytes": 42,
                "outputFingerprint": "sha256:abc",
            }
        ],
    )

    assert memory["schemaId"] == "agent.semantic-protocols.read-loop-memory"
    assert memory["entryCount"] == 2
    assert memory["suppressibleReadCount"] >= 2
    first = memory["entries"][0]
    assert first["selector"] == "src/lib.rs:10:20"
    assert first["readCount"] == 2
    assert first["repeatCount"] == 1
    assert "duplicate-read" in first["avoidReasons"]
    assert first["outputBytes"] == 42
    assert first["resultFingerprints"] == ["sha256:abc"]
