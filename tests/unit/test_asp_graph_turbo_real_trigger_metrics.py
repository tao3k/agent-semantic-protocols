"""Real-trigger metric record tests for graph turbo."""

from __future__ import annotations

import json
from pathlib import Path

from asp_graph_turbo.metrics_cli import main
from asp_graph_turbo.real_trigger_metrics import (
    REAL_TRIGGER_METRICS_SCHEMA_ID,
    build_real_trigger_metrics,
    render_real_trigger_metrics,
)

_REPO_ROOT = Path(__file__).resolve().parents[2]
_REAL_TRIGGER_FIXTURE = (
    _REPO_ROOT / "sandtables" / "fixtures" / "asp" / "graph-turbo-real-trigger-metrics.json"
)


def sample_record_args() -> dict[str, object]:
    return {
        "scenario": "rust-lexical-request-rank",
        "source": "live-cli",
        "measured_at": "2026-06-07T00:47:40Z",
        "profile": "owner-query",
        "commands": (
            "asp rust search lexical --view graph-turbo-request",
            "graph-turbo rank --format compact",
        ),
        "command_count": 2,
        "packet_bytes": 4960,
        "result_bytes": 1850,
        "latency_ms": 364,
        "repeated_trigger_patterns": 0,
        "missing_facts": 0,
        "confusing_next_actions": 0,
    }


def test_real_trigger_record_renders_all_rfc_metrics() -> None:
    record = build_real_trigger_metrics(**sample_record_args())

    text = render_real_trigger_metrics(record)

    assert text.startswith(
        "[graph-turbo-real-trigger] scenario=rust-lexical-request-rank"
    )
    assert "commandCount=2" in text
    assert "packetBytes=4960" in text
    assert "latencyMs=364" in text
    assert "repeatedTriggerPatterns=0" in text
    assert "missingFacts=0" in text
    assert "confusingNextActions=0" in text
    assert "profile=owner-query algorithm=typed-ppr-diverse" in text


def test_real_trigger_record_packet_is_stable() -> None:
    record = build_real_trigger_metrics(**sample_record_args())

    packet = record.to_packet()

    assert packet["schemaId"] == REAL_TRIGGER_METRICS_SCHEMA_ID
    assert packet["packetKind"] == "graph-turbo-real-trigger-metrics"
    assert packet["metrics"]["commandCount"] == 2
    assert packet["metrics"]["packetBytes"] == 4960
    assert packet["observations"] == {
        "repeatedTriggerPatterns": 0,
        "missingFacts": 0,
        "confusingNextActions": 0,
    }


def test_real_trigger_fixture_records_live_graph_turbo_metrics() -> None:
    packet = json.loads(_REAL_TRIGGER_FIXTURE.read_text(encoding="utf-8"))

    assert packet["schemaId"] == REAL_TRIGGER_METRICS_SCHEMA_ID
    assert packet["scenario"] == "rust-lexical-default-rank"
    assert packet["source"] == "live-cli"
    assert packet["profile"] == "owner-query"
    assert packet["algorithm"] == "typed-ppr-diverse"
    assert packet["commands"] == ["asp rust search lexical graph_turbo owner tests ."]
    assert packet["metrics"]["commandCount"] == len(packet["commands"]) == 1
    assert packet["metrics"]["packetBytes"] >= 0
    assert packet["metrics"]["resultBytes"] > 0
    assert packet["metrics"]["latencyMs"] > 0
    assert packet["observations"] == {
        "confusingNextActions": 0,
        "missingFacts": 0,
        "repeatedTriggerPatterns": 0,
    }


def test_metrics_cli_emits_json(capsys) -> None:
    assert (
        main(
            [
                "--scenario",
                "rust-lexical-request-rank",
                "--source",
                "live-cli",
                "--measured-at",
                "2026-06-07T00:47:40Z",
                "--profile",
                "owner-query",
                "--command",
                "asp rust search lexical --view graph-turbo-request",
                "--command",
                "graph-turbo rank --format compact",
                "--command-count",
                "2",
                "--packet-bytes",
                "4960",
                "--result-bytes",
                "1850",
                "--latency-ms",
                "364",
                "--repeated-trigger-patterns",
                "0",
                "--missing-facts",
                "0",
                "--confusing-next-actions",
                "0",
                "--format",
                "json",
            ]
        )
        == 0
    )

    packet = json.loads(capsys.readouterr().out)

    assert packet["schemaId"] == REAL_TRIGGER_METRICS_SCHEMA_ID
    assert packet["commands"] == [
        "asp rust search lexical --view graph-turbo-request",
        "graph-turbo rank --format compact",
    ]


def test_metrics_cli_rejects_command_count_drift(capsys) -> None:
    assert (
        main(
            [
                "--scenario",
                "rust-lexical-request-rank",
                "--measured-at",
                "2026-06-07T00:47:40Z",
                "--profile",
                "owner-query",
                "--command",
                "asp rust search lexical --view graph-turbo-request",
                "--command-count",
                "2",
                "--packet-bytes",
                "4960",
                "--result-bytes",
                "1850",
                "--latency-ms",
                "364",
                "--repeated-trigger-patterns",
                "0",
                "--missing-facts",
                "0",
                "--confusing-next-actions",
                "0",
            ]
        )
        == 2
    )

    assert "command_count must match" in capsys.readouterr().err
