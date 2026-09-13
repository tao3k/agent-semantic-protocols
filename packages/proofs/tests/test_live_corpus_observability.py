# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import json
from pathlib import Path

from asp_proofs.live_corpus_observability import CANONICAL_PHASES
from asp_proofs.live_corpus_observability import ObservabilityContractError
from asp_proofs.live_corpus_observability import PhaseEvent
from asp_proofs.live_corpus_observability import TelemetryCollector
from asp_proofs.live_corpus_observability import normalized_events_digest
from asp_proofs.live_corpus_observability import validate_event
from asp_proofs.live_corpus_observability import validate_plan


PLAN = Path(__file__).resolve().parents[1] / "receipts/live-corpus-observability-validation-plan.json"


def identity():
    return {
        "schemaRef": "runtime-server-opentelemetry-performance-event",
        "requestId": "request-1",
        "sessionId": "session-1",
        "workspaceIdentity": "workspace-23cc5ba784c605ae",
        "languageIds": ["rust"],
        "providerIds": ["asp-rust"],
        "runtimeArtifactDigest": "blake3-256:" + "a" * 64,
        "runtimeBundleDigest": "blake3-256:" + "1" * 64,
        "executionPublicationDigest": "blake3-256:" + "2" * 64,
        "workspaceSnapshotDigest": "blake3-256:" + "b" * 64,
        "sourceGenerationDigest": "blake3-256:" + "c" * 64,
        "sourceIndexDigest": "blake3-256:" + "d" * 64,
        "providerCatalogDigest": "blake3-256:" + "e" * 64,
        "sourceSnapshotDigest": "blake3-256:" + "f" * 64,
    }


def test_canonical_phase_order_is_stable():
    assert len(CANONICAL_PHASES) == 10
    assert CANONICAL_PHASES[0] == "launcher"
    assert CANONICAL_PHASES[-1] == "terminal-egress"


def test_identity_is_preserved_but_metric_labels_are_bounded():
    event = PhaseEvent("launcher", "cold", "search", "ok", identity(), 10, 20, {"profile": "cold"}).as_dict()
    validate_event(event)
    event["metrics"]["labels"]["requestId"] = "request-1"
    try:
        validate_event(event)
    except ObservabilityContractError:
        return
    raise AssertionError("high-cardinality request label was accepted")


def test_multilanguage_identity_sets_must_be_sorted_unique_and_nonempty():
    event = PhaseEvent("launcher", "cold", "search", "ok", identity(), 10, 20, {}).as_dict()
    event["identity"]["languageIds"] = ["rust", "python", "rust"]
    try:
        validate_event(event)
    except ObservabilityContractError:
        return
    raise AssertionError("non-canonical language identity set was accepted")


def test_replay_digest_ignores_clock_values_only():
    first = PhaseEvent("launcher", "cold", "search", "ok", identity(), 10, 20, {}).as_dict()
    second = PhaseEvent("launcher", "cold", "search", "ok", identity(), 100, 200, {}).as_dict()
    assert normalized_events_digest([first]) == normalized_events_digest([second])


def test_unknown_phase_fails_closed():
    event = PhaseEvent("launcher", "cold", "search", "ok", identity(), 10, 20, {}).as_dict()
    event["phase"] = "unknown"
    try:
        validate_event(event)
    except ObservabilityContractError:
        return
    raise AssertionError("unknown phase was accepted")


def test_collector_bounds_events_and_rejects_duplicate_terminal():
    collector = TelemetryCollector(max_events=2)
    first = PhaseEvent("launcher", "cold", "search", "ok", identity(), 10, 20, {}).as_dict()
    terminal = PhaseEvent("terminal-egress", "cold", "search", "ok", identity(), 30, 40, {}).as_dict()
    collector.record(first)
    collector.record(terminal)
    try:
        collector.record(terminal)
    except ObservabilityContractError:
        pass
    else:
        raise AssertionError("duplicate terminal was accepted")

    other = dict(first)
    other["identity"] = dict(identity(), requestId="request-2")
    try:
        collector.record(other)
    except ObservabilityContractError:
        return
    raise AssertionError("telemetry capacity was not bounded")


def test_plan_rejects_embedded_agent_planner_fields():
    plan = json.loads(PLAN.read_text(encoding="utf-8"))
    plan["currentBlocker"]["nextAction"] = "publish-or-retry"
    try:
        validate_plan(plan)
    except ObservabilityContractError:
        return
    raise AssertionError("Agent planner field was accepted")
