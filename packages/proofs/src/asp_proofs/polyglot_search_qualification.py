from __future__ import annotations

import argparse
import hashlib
import json
from dataclasses import asdict, dataclass
from typing import Any, Sequence
from asp_proofs._cli_output import write_stdout


SCHEMA_ID = "agent.semantic-protocols.polyglot-search-qualification.v1"
GENERATOR_ID = "asp-proofs-polyglot-search-qualification"
GENERATOR_VERSION = "1"


def canonical_json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"), sort_keys=True)


def digest(value: Any) -> str:
    return "sha256:" + hashlib.sha256(canonical_json(value).encode()).hexdigest()


def named_digest(name: str) -> str:
    return digest({"name": name})


@dataclass(frozen=True)
class Thresholds:
    maxTokenRatioBps: int = 7000
    maxGraphHopRatioBps: int = 7500
    maxTurnRatioBps: int = 7500
    maxLatencyRatioBps: int = 10000
    maxPrecisionRegressionBps: int = 200
    maxRecallRegressionBps: int = 200
    maxClosureRegressionBps: int = 100
    maxAnswerQualityRegressionBps: int = 100


@dataclass(frozen=True)
class Observation:
    totalTokens: int
    graphHops: int
    interactionTurns: int
    latencyMicros: int
    cacheHit: bool
    precisionBps: int
    recallBps: int
    closureBps: int
    answerQualityBps: int
    stateDigestBefore: str
    stateDigestAfter: str
    resultDigest: str


@dataclass(frozen=True)
class ScenarioInput:
    scenarioId: str
    scenarioClass: str
    cacheMode: str
    taskDigest: str
    queryDigest: str
    expectedEvidenceDigest: str
    legacy: Observation
    candidate: Observation


def _within_ratio(candidate: int, legacy: int, ratio_bps: int) -> bool:
    return candidate * 10000 <= legacy * ratio_bps


def _quality_noninferior(candidate: int, legacy: int, allowance_bps: int) -> bool:
    return candidate + allowance_bps >= legacy


def evaluate_scenario(
    scenario: ScenarioInput, thresholds: Thresholds
) -> dict[str, Any]:
    checks = (
        ("token-ratio", _within_ratio(scenario.candidate.totalTokens, scenario.legacy.totalTokens, thresholds.maxTokenRatioBps)),
        ("graph-hop-ratio", _within_ratio(scenario.candidate.graphHops, scenario.legacy.graphHops, thresholds.maxGraphHopRatioBps)),
        ("turn-ratio", _within_ratio(scenario.candidate.interactionTurns, scenario.legacy.interactionTurns, thresholds.maxTurnRatioBps)),
        ("latency-ratio", _within_ratio(scenario.candidate.latencyMicros, scenario.legacy.latencyMicros, thresholds.maxLatencyRatioBps)),
        ("precision", _quality_noninferior(scenario.candidate.precisionBps, scenario.legacy.precisionBps, thresholds.maxPrecisionRegressionBps)),
        ("recall", _quality_noninferior(scenario.candidate.recallBps, scenario.legacy.recallBps, thresholds.maxRecallRegressionBps)),
        ("closure", _quality_noninferior(scenario.candidate.closureBps, scenario.legacy.closureBps, thresholds.maxClosureRegressionBps)),
        ("answer-quality", _quality_noninferior(scenario.candidate.answerQualityBps, scenario.legacy.answerQualityBps, thresholds.maxAnswerQualityRegressionBps)),
        ("legacy-read-only", scenario.legacy.stateDigestBefore == scenario.legacy.stateDigestAfter),
        ("candidate-read-only", scenario.candidate.stateDigestBefore == scenario.candidate.stateDigestAfter),
        ("cache-mode", scenario.candidate.cacheHit == (scenario.cacheMode == "warm")),
    )
    reasons = [name for name, admitted in checks if not admitted]
    return {
        **asdict(scenario),
        "admitted": not reasons,
        "reasons": reasons,
    }


def _p95(values: Sequence[int]) -> int:
    ordered = sorted(values)
    index = max(0, (95 * len(ordered) + 99) // 100 - 1)
    return ordered[index]


def qualify(
    scenarios: Sequence[ScenarioInput], thresholds: Thresholds | None = None
) -> dict[str, Any]:
    active_thresholds = thresholds or Thresholds()
    evaluated = [evaluate_scenario(item, active_thresholds) for item in scenarios]
    scenario_ids = [item.scenarioId for item in scenarios]
    cold_count = sum(item.cacheMode == "cold" for item in scenarios)
    warm_count = sum(item.cacheMode == "warm" for item in scenarios)
    aggregate = {
        "scenarioCount": len(scenarios),
        "admittedScenarioCount": sum(item["admitted"] for item in evaluated),
        "coldScenarioCount": cold_count,
        "warmScenarioCount": warm_count,
        "legacyTotalTokens": sum(item.legacy.totalTokens for item in scenarios),
        "candidateTotalTokens": sum(item.candidate.totalTokens for item in scenarios),
        "legacyTotalGraphHops": sum(item.legacy.graphHops for item in scenarios),
        "candidateTotalGraphHops": sum(item.candidate.graphHops for item in scenarios),
        "legacyTotalTurns": sum(item.legacy.interactionTurns for item in scenarios),
        "candidateTotalTurns": sum(item.candidate.interactionTurns for item in scenarios),
        "legacyP95LatencyMicros": _p95([item.legacy.latencyMicros for item in scenarios]),
        "candidateP95LatencyMicros": _p95([item.candidate.latencyMicros for item in scenarios]),
    }
    global_checks = (
        ("duplicate-scenario-id", len(set(scenario_ids)) == len(scenario_ids)),
        ("cold-scenario-missing", cold_count > 0),
        ("warm-scenario-missing", warm_count > 0),
        ("scenario-rejected", aggregate["admittedScenarioCount"] == len(scenarios)),
        ("aggregate-token-regression", aggregate["candidateTotalTokens"] < aggregate["legacyTotalTokens"]),
        ("aggregate-hop-regression", aggregate["candidateTotalGraphHops"] < aggregate["legacyTotalGraphHops"]),
        ("aggregate-turn-regression", aggregate["candidateTotalTurns"] < aggregate["legacyTotalTurns"]),
        ("p95-latency-regression", aggregate["candidateP95LatencyMicros"] <= aggregate["legacyP95LatencyMicros"]),
    )
    reasons = [name for name, admitted in global_checks if not admitted]
    return {
        "thresholds": asdict(active_thresholds),
        "scenarios": evaluated,
        "aggregate": aggregate,
        "decision": {"state": "accepted" if not reasons else "rejected", "reasons": reasons},
    }


def _observation(
    name: str,
    *,
    tokens: int,
    hops: int,
    turns: int,
    latency: int,
    cache_hit: bool,
    precision: int,
    recall: int,
    closure: int,
    quality: int,
) -> Observation:
    state = named_digest(f"state:{name.split(':')[0]}")
    return Observation(
        totalTokens=tokens,
        graphHops=hops,
        interactionTurns=turns,
        latencyMicros=latency,
        cacheHit=cache_hit,
        precisionBps=precision,
        recallBps=recall,
        closureBps=closure,
        answerQualityBps=quality,
        stateDigestBefore=state,
        stateDigestAfter=state,
        resultDigest=named_digest(f"result:{name}"),
    )


def reference_scenarios() -> tuple[ScenarioInput, ...]:
    specifications = (
        ("one-hop-cold", "gql-path", "cold", (1000, 5, 4, 120000, False), (650, 3, 3, 100000, False), (10000, 10000, 10000, 10000)),
        ("filter-alias-warm", "gql-filter-alias", "warm", (900, 6, 4, 90000, True), (550, 3, 2, 60000, True), (9900, 9900, 10000, 9900)),
        ("logic-join-warm", "logic-join", "warm", (1200, 8, 5, 150000, True), (700, 5, 3, 110000, True), (9800, 10000, 9900, 9900)),
        ("parallel-witness-warm", "parallel-witness-bag", "warm", (800, 5, 3, 80000, True), (500, 3, 2, 70000, True), (10000, 10000, 10000, 10000)),
    )
    result = []
    for scenario_id, scenario_class, cache_mode, legacy, candidate, quality in specifications:
        precision, recall, closure, answer_quality = quality
        result.append(
            ScenarioInput(
                scenarioId=scenario_id,
                scenarioClass=scenario_class,
                cacheMode=cache_mode,
                taskDigest=named_digest(f"task:{scenario_id}"),
                queryDigest=named_digest(f"query:{scenario_id}"),
                expectedEvidenceDigest=named_digest(f"evidence:{scenario_id}"),
                legacy=_observation(
                    f"{scenario_id}:legacy", tokens=legacy[0], hops=legacy[1],
                    turns=legacy[2], latency=legacy[3], cache_hit=legacy[4],
                    precision=precision, recall=recall, closure=closure, quality=answer_quality,
                ),
                candidate=_observation(
                    f"{scenario_id}:candidate", tokens=candidate[0], hops=candidate[1],
                    turns=candidate[2], latency=candidate[3], cache_hit=candidate[4],
                    precision=precision, recall=recall, closure=closure, quality=answer_quality,
                ),
            )
        )
    return tuple(result)


def qualification_payload(
    scenarios: Sequence[ScenarioInput] | None = None,
) -> dict[str, Any]:
    qualification = qualify(scenarios or reference_scenarios())
    payload: dict[str, Any] = {
        "schemaId": SCHEMA_ID,
        "schemaVersion": "1",
        "qualificationId": "polyglot-search-reference-qualification",
        "generator": {"id": GENERATOR_ID, "version": GENERATOR_VERSION},
        "referenceTracePayloadDigest": "sha256:ce1c58d026a0740fedf87c08f4b6d8c67ed986ab29d9a86b87f00e2a8860b2ac",
        "sourceSnapshotDigest": named_digest("source-snapshot:reference"),
        "environmentDigest": named_digest("environment:deterministic-reference"),
        "legacyArtifact": {
            "providerDigest": named_digest("legacy-provider"),
            "runtimeDigest": named_digest("legacy-runtime"),
        },
        "candidateArtifact": {
            "providerDigest": named_digest("candidate-provider"),
            "runtimeDigest": named_digest("candidate-runtime"),
        },
        **qualification,
    }
    payload["qualificationDigest"] = digest(payload)
    payload["payloadDigest"] = digest(payload)
    return payload


def render_json_fixture() -> str:
    return canonical_json(qualification_payload()) + "\n"


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser()
    parser.add_argument("--format", choices=("json",), default="json")
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    build_parser().parse_args(argv)
    write_stdout(render_json_fixture(), end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
