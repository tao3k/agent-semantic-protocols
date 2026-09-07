# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Receipt completeness for sub-millisecond live-corpus qualification."""

from tests.unit.live_corpus_search_query_qualification_support import (
    RECEIPT_SCHEMA_PATH,
    load_json,
)


def test_receipt_requires_complete_sub_millisecond_latency_distributions() -> None:
    receipt_schema = load_json(RECEIPT_SCHEMA_PATH)
    assert "clientProtocol" in receipt_schema["required"]
    client_receipt = receipt_schema["$defs"]["clientProtocolReceipt"]
    assert client_receipt["properties"]["cancelOutcome"]["const"] == "cancelled"
    assert client_receipt["properties"]["requestOutcome"]["const"] == "cancelled"
    assert client_receipt["properties"]["qualifiedCaseCount"]["const"] == 17
    assert client_receipt["properties"]["sessionPolicy"]["const"] == "one-initialize-per-session"
    assert client_receipt["properties"]["readyEffects"]["const"] == [
        "mpsc",
        "oneshot",
        "cancel",
        "response",
    ]
    assert client_receipt["properties"]["forbiddenReadyEffects"]["const"] == [
        "process",
        "filesystem",
        "dbWrite",
        "generationMutation",
        "providerActivation",
        "controlPoll",
    ]
    assert client_receipt["properties"]["nonReadyDispatchCount"]["const"] == 0
    assert client_receipt["properties"]["residualTaskCount"]["const"] == 0
    assert client_receipt["properties"]["p50MaximumMicros"]["const"] == 250
    assert client_receipt["properties"]["p99MaximumMicros"]["const"] == 700
    assert client_receipt["properties"]["maxMaximumMicros"]["const"] == 1000
    case_schema = receipt_schema["$defs"]["caseReceipt"]
    required = set(case_schema["required"])
    assert {
        "residentSampleCount",
        "searchResidentReadLatencyMicros",
        "searchServiceLatencyMicros",
        "searchTotalLatencyMicros",
        "exactSourceLatencyMicros",
        "callableSkeletonOperationId",
        "callableSkeletonElapsedMicros",
        "callableSkeletonLatencyMicros",
        "coldBuildSampleCount",
        "coldBuildSearchQueryLatencyMicros",
        "coldLoadSampleCount",
        "coldLoadPrepareLatencyMicros",
        "coldLoadSearchQueryLatencyMicros",
        "warmReadPrepareElapsedMicros",
        "cancellationProbeElapsedMicros",
        "backpressureCapacity",
        "backpressureHeldCallCount",
        "backpressureRejectedCallCount",
        "backpressureProbeElapsedMicros",
        "staleContentBindingRejected",
        "staleContentBindingProbeElapsedMicros",
        "sequentialSampleCount",
        "sequentialSearchQueryLatencyMicros",
        "concurrentSampleCount",
        "concurrentSearchQueryLatencyMicros",
        "route",
        "searchTerminal",
        "queryTerminal",
        "callableSkeletonTerminal",
        "zeroMatchTerminal",
    } <= required

    assert not {
        "sourceIndexTelemetryDigest",
        "sourceExactTelemetryDigest",
        "callableSkeletonTelemetryDigest",
        "merkleTelemetryDigest",
    } & required
    assert case_schema["properties"]["route"]["const"] == "public-typed-asp-client"
    assert case_schema["properties"]["backpressureCapacity"]["const"] == 32
    assert case_schema["properties"]["backpressureHeldCallCount"]["const"] == 31
    assert case_schema["properties"]["backpressureRejectedCallCount"]["const"] == 1
    assert case_schema["properties"]["staleContentBindingRejected"]["const"] is True

    assert case_schema["properties"]["sequentialSampleCount"]["const"] == 10
    assert case_schema["properties"]["concurrentSampleCount"]["const"] == 32
    executed_distribution = receipt_schema["$defs"]["executedLatencyDistribution"]
    assert executed_distribution["additionalProperties"] is False
    assert executed_distribution["properties"]["sampleCount"]["minimum"] == 1

    distribution = receipt_schema["$defs"]["latencyDistribution"]
    assert distribution["additionalProperties"] is False
    assert set(distribution["required"]) == {
        "sampleCount",
        "minMicros",
        "p50Micros",
        "p95Micros",
        "p99Micros",
        "maxMicros",
    }
    assert distribution["properties"]["sampleCount"]["minimum"] == 128
    for field in ("minMicros", "p50Micros", "p95Micros", "p99Micros", "maxMicros"):
        assert distribution["properties"][field]["maximum"] == 1_000
