# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""V1 live-corpus telemetry and artifact validation.

This module is deliberately non-serving.  It validates the shared
qualification plan and normalizes phase events produced by the ASP Server.
It does not discover resources, start providers, publish artifacts, or retry
missing data.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
from typing import Any, Iterable, Mapping

from blake3 import blake3


PLAN_SCHEMA_ID = "asp.live-corpus-search-query-qualification-plan"
TELEMETRY_SCHEMA_ID = "runtime-server-opentelemetry-performance-event"
RECEIPT_SCHEMA_ID = "asp.live-corpus-search-query-qualification-receipt"

CANONICAL_PHASES = (
    "launcher",
    "client-frame-encode",
    "ipc-connect",
    "server-admission-queue",
    "snapshot-resolve",
    "provider-dispatch",
    "parse-index-query",
    "projection-rank",
    "schema-validate-serialize",
    "terminal-egress",
)

LOW_CARDINALITY_METRIC_LABELS = frozenset(
    {"operation", "profile", "phase", "outcome", "error_class"}
)
IDENTITY_FIELDS = (
    "schemaRef",
    "requestId",
    "sessionId",
    "workspaceIdentity",
    "languageIds",
    "providerIds",
    "runtimeArtifactDigest",
    "runtimeBundleDigest",
    "executionPublicationDigest",
    "workspaceSnapshotDigest",
    "sourceGenerationDigest",
    "sourceIndexDigest",
    "providerCatalogDigest",
    "sourceSnapshotDigest",
)


class ObservabilityContractError(ValueError):
    """Raised when a plan, event, or artifact violates the V1 contract."""


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise ObservabilityContractError(message)


def _is_repo_relative(value: Any) -> bool:
    return isinstance(value, str) and value and not value.startswith(("/", "~"))


def _is_nonempty_string(value: Any) -> bool:
    return isinstance(value, str) and bool(value)


def validate_plan(plan: Mapping[str, Any]) -> None:
    """Validate the canonical machine-readable qualification plan."""

    schema = plan.get("schema")
    _require(isinstance(schema, Mapping), "missing top-level schema identity")
    _require(schema.get("id") == PLAN_SCHEMA_ID, "unexpected plan schema id")
    _require(schema.get("version") == 1, "unsupported plan schema version")
    _require(plan.get("schemaId") == PLAN_SCHEMA_ID, "schemaId mismatch")
    _require(plan.get("schemaVersion") == 1, "schemaVersion mismatch")

    phases = plan.get("phases")
    _require(tuple(phases or ()) == CANONICAL_PHASES, "canonical phase sequence mismatch")
    assertions = plan.get("assertions")
    _require(isinstance(assertions, Mapping), "missing deterministic assertions")
    _require(assertions.get("phaseCount") == len(CANONICAL_PHASES), "phase count mismatch")
    _require(assertions.get("profileCount") == len(plan.get("profiles", ())), "profile count mismatch")
    samples = plan.get("samples")
    _require(isinstance(samples, Mapping), "missing sample matrix")
    _require(assertions.get("sequentialSampleCount") == samples.get("sequential"), "sequential count mismatch")
    _require(assertions.get("concurrentSampleCount") == samples.get("concurrent"), "concurrent count mismatch")
    rounds = samples.get("failureInjectionRounds")
    _require(isinstance(rounds, list) and assertions.get("failureInjectionRoundCount") == len(rounds), "failure round mismatch")
    _require(assertions.get("repeatWorkloadCount") == samples.get("repeatWorkload"), "repeat count mismatch")

    policy = plan.get("artifactPolicy")
    _require(isinstance(policy, Mapping) and policy.get("pathsMustBeRepoRelative") is True, "artifact path policy missing")
    blocker = plan.get("currentBlocker")
    if isinstance(blocker, Mapping) and "pointer" in blocker:
        _require(_is_repo_relative(blocker["pointer"]), "absolute blocker pointer")
    planner_fields = {"next", "nextAction", "recommendedNext", "plannerDecision"}
    _require(not planner_fields.intersection(plan), "plan contains an Agent planner field")
    if isinstance(blocker, Mapping):
        _require(
            not planner_fields.intersection(blocker),
            "blocker contains an Agent planner field",
        )


def validate_identity(identity: Mapping[str, Any]) -> None:
    """Require the exact content/request identity carried by every event."""

    for field in IDENTITY_FIELDS:
        value = identity.get(field)
        if field in {"languageIds", "providerIds"}:
            _require(
                isinstance(value, list)
                and bool(value)
                and all(_is_nonempty_string(item) for item in value)
                and value == sorted(set(value)),
                f"non-canonical identity set: {field}",
            )
        else:
            _require(_is_nonempty_string(value), f"missing identity field: {field}")
    _require(identity.get("schemaRef") == TELEMETRY_SCHEMA_ID, "telemetry schema reference mismatch")


def validate_event(event: Mapping[str, Any]) -> None:
    """Validate one phase event without accepting unbounded metric labels."""

    _require(event.get("schemaId") == TELEMETRY_SCHEMA_ID, "event schema id mismatch")
    _require(event.get("schemaVersion") == 1, "event schema version mismatch")
    phase = event.get("phase")
    _require(phase in CANONICAL_PHASES, "unknown telemetry phase")
    _require(event.get("profile") in {"cold", "resident-warm"}, "unknown profile")
    _require(_is_nonempty_string(event.get("operation")), "missing operation")
    _require(_is_nonempty_string(event.get("outcome")), "missing outcome")
    validate_identity(event.get("identity", {}))
    metrics = event.get("metrics", {})
    _require(isinstance(metrics, Mapping), "metrics must be an object")
    labels = metrics.get("labels", {})
    _require(isinstance(labels, Mapping), "metric labels must be an object")
    _require(set(labels).issubset(LOW_CARDINALITY_METRIC_LABELS), "high-cardinality metric label")
    for key in ("monotonicStartNs", "monotonicEndNs"):
        _require(isinstance(metrics.get(key), int) and metrics[key] >= 0, f"invalid {key}")
    _require(metrics["monotonicEndNs"] >= metrics["monotonicStartNs"], "negative phase duration")


def validate_artifact(artifact: Mapping[str, Any], plan: Mapping[str, Any]) -> None:
    """Validate a qualification artifact before it can be reported."""

    validate_plan(plan)
    schema = artifact.get("schema")
    _require(isinstance(schema, Mapping), "missing artifact schema identity")
    _require(schema.get("id") == RECEIPT_SCHEMA_ID, "unexpected artifact schema id")
    _require(schema.get("version") == 1, "unsupported artifact schema version")
    _require(artifact.get("planSchema") == PLAN_SCHEMA_ID, "artifact plan schema mismatch")
    _require(artifact.get("phases") == list(CANONICAL_PHASES), "artifact phase list mismatch")
    _require(_is_repo_relative(artifact.get("artifactPath")), "artifact path must be repo-relative")
    events = artifact.get("events")
    _require(isinstance(events, list), "artifact events must be an array")
    for event in events:
        validate_event(event)
    request_ids = {event["identity"]["requestId"] for event in events}
    terminal_events = [event for event in events if event["phase"] == "terminal-egress"]
    _require(len(terminal_events) == len(request_ids), "exactly-one terminal per request violated")
    _require(not artifact.get("allowSync", False), "artifact permits corpus sync")
    _require(not artifact.get("allowFallback", False), "artifact permits fallback")


def normalized_events_digest(events: Iterable[Mapping[str, Any]]) -> str:
    """Hash replay-comparable events with monotonic clock values removed."""

    normalized = []
    for event in events:
        validate_event(event)
        copy = json.loads(json.dumps(event, sort_keys=True))
        metrics = copy.get("metrics", {})
        metrics.pop("monotonicStartNs", None)
        metrics.pop("monotonicEndNs", None)
        normalized.append(copy)
    payload = json.dumps(normalized, sort_keys=True, separators=(",", ":")).encode()
    return "blake3-256:" + blake3(payload).hexdigest()


@dataclass(frozen=True)
class PhaseEvent:
    """Small typed constructor for a server-emitted phase event."""

    phase: str
    profile: str
    operation: str
    outcome: str
    identity: Mapping[str, Any]
    start_ns: int
    end_ns: int
    labels: Mapping[str, str]

    def as_dict(self) -> dict[str, Any]:
        event = {
            "schemaId": TELEMETRY_SCHEMA_ID,
            "schemaVersion": 1,
            "phase": self.phase,
            "profile": self.profile,
            "operation": self.operation,
            "outcome": self.outcome,
            "identity": dict(self.identity),
            "metrics": {
                "monotonicStartNs": self.start_ns,
                "monotonicEndNs": self.end_ns,
                "labels": dict(self.labels),
            },
        }
        validate_event(event)
        return event


class TelemetryCollector:
    """Bounded server-side collector used by qualification adapters.

    Exporters may turn the returned events and counters into OpenTelemetry
    spans/metrics.  The collector itself owns ordering, terminal uniqueness,
    and label cardinality so an exporter cannot weaken the V1 contract.
    """

    def __init__(self, max_events: int = 4096) -> None:
        _require(isinstance(max_events, int) and max_events > 0, "invalid telemetry capacity")
        self._max_events = max_events
        self._events: list[dict[str, Any]] = []
        self._terminal_requests: set[str] = set()
        self._metric_counts: dict[tuple[str, str, str, str, str], int] = {}

    def record(self, event: Mapping[str, Any]) -> None:
        validate_event(event)
        if len(self._events) >= self._max_events:
            raise ObservabilityContractError("telemetry buffer capacity exhausted")
        request_id = event["identity"]["requestId"]
        if event["phase"] == "terminal-egress":
            if request_id in self._terminal_requests:
                raise ObservabilityContractError("duplicate terminal receipt")
            self._terminal_requests.add(request_id)
        labels = event["metrics"].get("labels", {})
        metric_key = (
            event["operation"],
            event["profile"],
            event["phase"],
            event["outcome"],
            labels.get("error_class", "none"),
        )
        self._metric_counts[metric_key] = self._metric_counts.get(metric_key, 0) + 1
        self._events.append(dict(event))

    def events(self) -> tuple[Mapping[str, Any], ...]:
        return tuple(self._events)

    def metrics(self) -> Mapping[tuple[str, str, str, str, str], int]:
        return dict(self._metric_counts)

    def to_artifact(self, plan: Mapping[str, Any], artifact_path: str) -> dict[str, Any]:
        validate_plan(plan)
        _require(_is_repo_relative(artifact_path), "artifact path must be repo-relative")
        artifact = {
            "schema": {"id": RECEIPT_SCHEMA_ID, "version": 1},
            "planSchema": PLAN_SCHEMA_ID,
            "phases": list(CANONICAL_PHASES),
            "artifactPath": artifact_path,
            "events": list(self._events),
            "allowSync": False,
            "allowFallback": False,
        }
        validate_artifact(artifact, plan)
        return artifact
