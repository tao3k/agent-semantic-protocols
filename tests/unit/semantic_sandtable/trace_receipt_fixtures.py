"""Fixtures for trace receipt tests."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[3]
TEST_BLOCK = (
    "crates/agent-semantic-client/tests/unit/cache_cli/writeback/search.rs:40-72"
)
WRITEBACK_BLOCK = "crates/agent-semantic-client/src/cache_cli/writeback.rs:220-260"
REPLAY_BLOCK = "crates/agent-semantic-client/src/cache_replay/artifact.rs:88-132"
FRESHNESS_BLOCK = "crates/agent-semantic-client/src/cache_cli/probe.rs:140-205"
HOT_BLOCKS = [
    TEST_BLOCK,
    WRITEBACK_BLOCK,
    REPLAY_BLOCK,
    FRESHNESS_BLOCK,
]


def trace_event(
    command_id: str,
    selector: str,
    *,
    stdout_bytes: int = 700,
) -> dict[str, Any]:
    return {
        "id": command_id,
        "argv": [
            "asp",
            "rust",
            "query",
            "--from-hook",
            "direct-source-read",
            "--selector",
            selector,
            "--code",
            ".",
        ],
        "metrics": {
            "elapsedMs": 3,
            "stdoutBytes": stdout_bytes,
            "stderrBytes": 0,
        },
    }


def window_selector(index: int) -> str:
    return f"src/cache.rs:{index * 10}-{index * 10 + 20}"


def write_failure_frontier_dev_log_root(
    trace_root: Path,
    *,
    baseline_session: str = "baseline",
    candidate_session: str = "candidate",
) -> None:
    command_dir = trace_root / "semantic_protocol" / "rust" / "rs-harness" / "commands"
    command_dir.mkdir(parents=True)
    command_lines = [
        *[
            _session_event(
                baseline_session,
                trace_event(f"window-{index}", window_selector(index)),
            )
            for index in range(1, 11)
        ],
        _frontier_event(candidate_session),
        _session_event(
            candidate_session,
            trace_event("test", TEST_BLOCK, stdout_bytes=120),
        ),
        _session_event(
            candidate_session,
            trace_event("writeback", WRITEBACK_BLOCK, stdout_bytes=120),
        ),
        _session_event(
            candidate_session,
            trace_event("replay", REPLAY_BLOCK, stdout_bytes=120),
        ),
        _session_event(
            candidate_session,
            trace_event("freshness", FRESHNESS_BLOCK, stdout_bytes=120),
        ),
    ]
    (command_dir / "commands.jsonl").write_text(
        "\n".join(json.dumps(line) for line in command_lines) + "\n",
        encoding="utf-8",
    )


def _frontier_event(session_id: str) -> dict[str, object]:
    return _session_event(
        session_id,
        {
            "id": "failure-frontier",
            "kind": "check",
            "argv": ["asp", "rust", "check", "changed", "--view", "seeds", "."],
            "next": HOT_BLOCKS,
            "metrics": {"elapsedMs": 5, "stdoutBytes": 180, "stderrBytes": 0},
        },
    )


def _session_event(session_id: str, event: dict[str, object]) -> dict[str, object]:
    event = dict(event)
    event["sessionId"] = session_id
    event["languageId"] = "rust"
    event["providerId"] = "rs-harness"
    return event
