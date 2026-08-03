from __future__ import annotations

import json

from tools import search_wall_gate


def wall_failure(*, surface: str = "query") -> bytes:
    return (
        json.dumps(
            {
                "schemaId": "agent.semantic-protocols.agent-facing-search-wall-failure",
                "schemaVersion": "1",
                "surface": surface,
                "state": "unavailable",
                "reasonKind": "agent-facing-search-wall-budget-exceeded",
                "stage": "runtime-server-client",
                "budgetMicros": 1_000_000,
                "executionBudgetMicros": 800_000,
                "elapsedMicros": 800_127,
                "retryAfterMs": 250,
            }
        )
        + "\n"
    ).encode()


def test_typed_wall_failure_is_recognized() -> None:
    assert search_wall_gate._is_typed_wall_failure(
        wall_failure(), surface="query"
    )


def test_typed_wall_failure_rejects_surface_mismatch() -> None:
    assert not search_wall_gate._is_typed_wall_failure(
        wall_failure(surface="search"), surface="query"
    )


def test_typed_wall_failure_rejects_generic_stderr() -> None:
    assert not search_wall_gate._is_typed_wall_failure(
        b"provider failed\n", surface="query"
    )


def test_typed_wall_failure_rejects_json_boolean_as_elapsed_integer() -> None:
    payload = json.loads(wall_failure())
    payload["elapsedMicros"] = True
    assert not search_wall_gate._is_typed_wall_failure(
        (json.dumps(payload) + "\n").encode(), surface="query"
    )


def test_one_second_is_already_over_budget() -> None:
    assert search_wall_gate.budget_status(999_999) == "within-budget"
    assert search_wall_gate.budget_status(1_000_000) == "budget-exceeded"


def test_run_gate_reserves_one_hundred_milliseconds_for_reply(
    monkeypatch,
    capsys,
) -> None:
    observed_timeouts: list[float | None] = []

    class CompletedEvidence:
        returncode = 0

        def communicate(self, timeout: float | None = None) -> tuple[bytes, bytes]:
            observed_timeouts.append(timeout)
            return b"evidence", b""

    monkeypatch.setattr(
        search_wall_gate.subprocess,
        "Popen",
        lambda *args, **kwargs: CompletedEvidence(),
    )

    assert search_wall_gate.run_gate(
        ["asp", "rust", "query"], "rust", "query"
    ) == 0
    assert observed_timeouts == [0.9]
    capsys.readouterr()


def test_run_gate_classifies_schema_owned_failure_as_unavailable(
    monkeypatch,
    capsys,
) -> None:
    class CompletedFailure:
        returncode = 2

        def communicate(self, timeout: float | None = None) -> tuple[bytes, bytes]:
            return b"", wall_failure()

    monkeypatch.setattr(
        search_wall_gate.subprocess,
        "Popen",
        lambda *args, **kwargs: CompletedFailure(),
    )

    assert search_wall_gate.run_gate(
        ["asp", "rust", "query"], "rust", "query"
    ) == 1
    receipt = json.loads(capsys.readouterr().out)
    assert receipt["replyKind"] == "unavailable"
    assert receipt["exitCode"] == 2
    assert receipt["budgetStatus"] == "within-budget"


def test_run_gate_keeps_untyped_failure_distinct(monkeypatch, capsys) -> None:
    class CompletedFailure:
        returncode = 2

        def communicate(self, timeout: float | None = None) -> tuple[bytes, bytes]:
            return b"", b"provider failed\n"

    monkeypatch.setattr(
        search_wall_gate.subprocess,
        "Popen",
        lambda *args, **kwargs: CompletedFailure(),
    )

    assert search_wall_gate.run_gate(
        ["asp", "rust", "search"], "rust", "search"
    ) == 1
    receipt = json.loads(capsys.readouterr().out)
    assert receipt["replyKind"] == "failure"
