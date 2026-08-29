"""Real-trigger metric records for graph turbo RFC validation."""

from __future__ import annotations

from collections.abc import Sequence
from dataclasses import dataclass
from typing import Any

from .constants import ALGORITHM_ID

REAL_TRIGGER_METRICS_SCHEMA_ID = (
    "agent.semantic-protocols.graph-turbo-real-trigger-metrics"
)
REAL_TRIGGER_METRICS_SCHEMA_VERSION = "1"
REAL_TRIGGER_METRICS_PROTOCOL_ID = "agent.semantic-protocols.semantic-language"
REAL_TRIGGER_METRICS_PROTOCOL_VERSION = "1"


@dataclass(frozen=True)
class RealTriggerMetrics:
    scenario: str
    source: str
    measured_at: str
    profile: str
    algorithm: str
    commands: tuple[str, ...]
    command_count: int
    packet_bytes: int
    result_bytes: int
    latency_ms: int
    repeated_trigger_patterns: int
    missing_facts: int
    confusing_next_actions: int

    def to_packet(self) -> dict[str, Any]:
        return {
            "schemaId": REAL_TRIGGER_METRICS_SCHEMA_ID,
            "schemaVersion": REAL_TRIGGER_METRICS_SCHEMA_VERSION,
            "protocolId": REAL_TRIGGER_METRICS_PROTOCOL_ID,
            "protocolVersion": REAL_TRIGGER_METRICS_PROTOCOL_VERSION,
            "packetKind": "graph-turbo-real-trigger-metrics",
            "scenario": self.scenario,
            "source": self.source,
            "measuredAt": self.measured_at,
            "profile": self.profile,
            "algorithm": self.algorithm,
            "commands": list(self.commands),
            "metrics": {
                "commandCount": self.command_count,
                "packetBytes": self.packet_bytes,
                "resultBytes": self.result_bytes,
                "latencyMs": self.latency_ms,
            },
            "observations": {
                "repeatedTriggerPatterns": self.repeated_trigger_patterns,
                "missingFacts": self.missing_facts,
                "confusingNextActions": self.confusing_next_actions,
            },
        }


def build_real_trigger_metrics(
    *,
    scenario: str,
    source: str,
    measured_at: str,
    profile: str,
    commands: Sequence[str],
    command_count: int,
    packet_bytes: int,
    result_bytes: int,
    latency_ms: int,
    repeated_trigger_patterns: int,
    missing_facts: int,
    confusing_next_actions: int,
    algorithm: str = ALGORITHM_ID,
) -> RealTriggerMetrics:
    _validate_identity_fields(scenario, source, measured_at, profile, algorithm)
    _validate_metric_counts(
        command_count,
        packet_bytes,
        result_bytes,
        latency_ms,
        repeated_trigger_patterns,
        missing_facts,
        confusing_next_actions,
    )
    command_tuple = tuple(commands)
    _validate_commands(command_tuple, command_count)

    return RealTriggerMetrics(
        scenario=scenario,
        source=source,
        measured_at=measured_at,
        profile=profile,
        algorithm=algorithm,
        commands=command_tuple,
        command_count=command_count,
        packet_bytes=packet_bytes,
        result_bytes=result_bytes,
        latency_ms=latency_ms,
        repeated_trigger_patterns=repeated_trigger_patterns,
        missing_facts=missing_facts,
        confusing_next_actions=confusing_next_actions,
    )


def render_real_trigger_metrics(record: RealTriggerMetrics) -> str:
    return "\n".join(
        [
            (
                "[graph-turbo-real-trigger] "
                f"scenario={record.scenario} source={record.source} "
                f"measuredAt={record.measured_at}"
            ),
            f"commands={','.join(record.commands)}",
            (
                "metrics="
                f"commandCount={record.command_count},"
                f"packetBytes={record.packet_bytes},"
                f"resultBytes={record.result_bytes},"
                f"latencyMs={record.latency_ms}"
            ),
            (
                "observations="
                f"repeatedTriggerPatterns={record.repeated_trigger_patterns},"
                f"missingFacts={record.missing_facts},"
                f"confusingNextActions={record.confusing_next_actions}"
            ),
            f"profile={record.profile} algorithm={record.algorithm}",
        ]
    )


def _require_non_empty(name: str, value: str) -> None:
    if not value:
        raise ValueError(f"{name} must not be empty")


def _validate_identity_fields(
    scenario: str, source: str, measured_at: str, profile: str, algorithm: str
) -> None:
    _require_non_empty("scenario", scenario)
    _require_non_empty("source", source)
    _require_non_empty("measured_at", measured_at)
    _require_non_empty("profile", profile)
    _require_non_empty("algorithm", algorithm)


def _validate_metric_counts(
    command_count: int,
    packet_bytes: int,
    result_bytes: int,
    latency_ms: int,
    repeated_trigger_patterns: int,
    missing_facts: int,
    confusing_next_actions: int,
) -> None:
    _require_positive("command_count", command_count)
    _require_non_negative("packet_bytes", packet_bytes)
    _require_non_negative("result_bytes", result_bytes)
    _require_non_negative("latency_ms", latency_ms)
    _require_non_negative("repeated_trigger_patterns", repeated_trigger_patterns)
    _require_non_negative("missing_facts", missing_facts)
    _require_non_negative("confusing_next_actions", confusing_next_actions)


def _validate_commands(commands: tuple[str, ...], command_count: int) -> None:
    if len(commands) != command_count:
        raise ValueError(
            "command_count must match the number of command labels "
            f"({command_count} != {len(commands)})"
        )
    if any(not command for command in commands):
        raise ValueError("command labels must not be empty")


def _require_positive(name: str, value: int) -> None:
    if value < 1:
        raise ValueError(f"{name} must be positive")


def _require_non_negative(name: str, value: int) -> None:
    if value < 0:
        raise ValueError(f"{name} must be non-negative")
