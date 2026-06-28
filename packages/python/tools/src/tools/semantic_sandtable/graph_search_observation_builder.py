"""Build graph search observations from semantic sandtable reports."""

from __future__ import annotations

import json
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterable

from tools.semantic_sandtable.graph_search_observation_contract import (
    SCHEMA_ID,
    SCHEMA_VERSION,
    _bool_or_none,
    _drop_none,
    _int_or_zero,
    _number_or_none,
    _safe_optional,
    _safe_scalar,
    _string_or_none,
    assert_no_absolute_paths,
    path_ref,
)
from tools.semantic_sandtable.graph_search_observation_provider_health import (
    _first_provider_failure_kind,
    _provider_health,
    _scenario_status,
)
from tools.semantic_sandtable.graph_search_observation_values import (
    _command_text,
    _display_ranges,
    _first_string,
    _graph_edges,
    _list_of_dicts,
    _path_refs,
    _ranked_evidence,
    _string_list,
    _string_values,
    _symbol_refs,
)


def observations_from_report(report: dict[str, Any], source_ref: str | None = None) -> list[dict[str, Any]]:
    scenarios = report.get("scenarios")
    if not isinstance(scenarios, list):
        scenarios = [report]

    run_id = _string_or_none(report.get("runId") or report.get("id"))
    observations: list[dict[str, Any]] = []
    for index, scenario in enumerate(scenarios):
        if not isinstance(scenario, dict):
            continue
        observation = _observation_from_scenario(
            scenario,
            index=index,
            run_id=run_id,
            source_ref=source_ref,
        )
        assert_no_absolute_paths(observation)
        observations.append(observation)
    return observations


def write_jsonl(observations: Iterable[dict[str, Any]], output: Path) -> None:
    with output.open("w", encoding="utf-8") as handle:
        for observation in observations:
            assert_no_absolute_paths(observation)
            handle.write(json.dumps(observation, ensure_ascii=False, sort_keys=True))
            handle.write("\n")


def _observation_from_scenario(
    scenario: dict[str, Any],
    *,
    index: int,
    run_id: str | None,
    source_ref: str | None,
) -> dict[str, Any]:
    steps = _list_of_dicts(scenario.get("steps"))
    usage_records = _token_usage_records(steps)
    provider_health = _provider_health(scenario, steps, usage_records)
    cost = _cost_from_steps(steps, usage_records)
    failure_kind = _first_provider_failure_kind(provider_health)
    source = _source(source_ref=source_ref, run_id=run_id)

    observation = {
        "schemaId": SCHEMA_ID,
        "schemaVersion": SCHEMA_VERSION,
        "recordedAt": datetime.now(timezone.utc).isoformat(),
        "subject": _subject_from_scenario(scenario, usage_records, index),
        "evidenceState": _evidence_state(scenario),
        "routeDecision": _route_decision(scenario, steps),
        "graphEvidence": _graph_evidence(scenario),
        "providerHealth": provider_health,
        "cost": cost,
        "outcome": _outcome(scenario, provider_health, failure_kind, cost),
        "source": source,
    }
    return _drop_none(observation)


def _source(*, source_ref: str | None, run_id: str | None) -> dict[str, Any]:
    source: dict[str, Any] = {"kind": "sandtable-report"}
    if source_ref:
        source["pathRef"] = path_ref("artifact", source_ref)
    if run_id:
        source["runId"] = _safe_scalar(run_id)
    return source


def _subject_from_scenario(
    scenario: dict[str, Any],
    usage_records: list[dict[str, Any]],
    index: int,
) -> dict[str, Any]:
    first_usage = usage_records[0] if usage_records else {}
    subject = {
        "scenarioId": _safe_scalar(
            scenario.get("id")
            or scenario.get("scenarioId")
            or scenario.get("name")
            or f"scenario-{index + 1}"
        ),
        "language": _string_or_none(scenario.get("language")),
        "intent": _string_or_none(scenario.get("intent") or scenario.get("question")),
        "agent": _string_or_none(scenario.get("agent")),
        "provider": _string_or_none(first_usage.get("provider")),
        "model": _string_or_none(first_usage.get("model")),
        "tags": [_safe_scalar(tag) for tag in scenario.get("tags", []) if isinstance(tag, str)],
    }
    return _drop_none(subject)


def _evidence_state(scenario: dict[str, Any]) -> dict[str, Any]:
    evidence = scenario.get("evidenceState")
    if not isinstance(evidence, dict):
        evidence = {}
    result = {
        "knownSelector": _bool_or_none(evidence.get("knownSelector")),
        "knownOwner": _bool_or_none(evidence.get("knownOwner")),
        "queryQuality": _string_or_none(evidence.get("queryQuality")),
        "packageCohesion": _string_or_none(evidence.get("packageCohesion")),
        "missingTerms": _string_list(evidence.get("missingTerms")),
        "riskFactors": _string_list(evidence.get("riskFactors")),
    }
    return _drop_none(result)


def _route_decision(scenario: dict[str, Any], steps: list[dict[str, Any]]) -> dict[str, Any]:
    route = scenario.get("routeDecision")
    if not isinstance(route, dict):
        route = {}
    chosen = _string_or_none(route.get("chosen")) or _infer_chosen_route(steps)
    result = {
        "chosen": chosen or "unknown",
        "recommendedNext": _safe_optional(route.get("recommendedNext")),
        "nextCommandKind": _safe_optional(route.get("nextCommandKind")),
        "avoided": _string_list(route.get("avoided")),
        "denials": _infer_denials(scenario, steps),
    }
    return _drop_none(result)


def _graph_evidence(scenario: dict[str, Any]) -> dict[str, Any]:
    graph = scenario.get("graphEvidence")
    if not isinstance(graph, dict):
        graph = {}
    return {
        "owners": _path_refs(graph.get("owners"), "owner"),
        "items": _symbol_refs(graph.get("items")),
        "hotRanges": _display_ranges(graph.get("hotRanges")),
        "tests": _path_refs(graph.get("tests"), "owner"),
        "edges": _graph_edges(graph.get("edges")),
        "rankedEvidence": _ranked_evidence(graph.get("rankedEvidence")),
    }


def _outcome(
    scenario: dict[str, Any],
    provider_health: list[dict[str, Any]],
    failure_kind: str | None,
    cost: dict[str, Any],
) -> dict[str, Any]:
    status = _scenario_status(scenario, provider_health)
    return {
        "status": status,
        "answerQuality": "usable" if status == "pass" else "needs-reroute",
        "nextOptimization": _next_optimization(failure_kind, cost),
    }


def _next_optimization(failure_kind: str | None, cost: dict[str, Any]) -> str:
    if failure_kind in {"dynamic-library-rpath", "python-provider-module-missing"}:
        return "repair-provider-runtime-before-ranking"
    if cost.get("totalTokens", 0) == 0:
        return "connect-live-llm-token-receipt"
    if cost.get("commands", 0) > 6:
        return "reduce-route-turns-with-graph-frontier"
    return "calibrate-route-ranking"


def _infer_chosen_route(steps: list[dict[str, Any]]) -> str | None:
    for step in steps:
        text = _command_text(step.get("command"))
        if " search owner " in text:
            return "owner-items"
        if " search pipe " in text:
            return "search-pipe"
        if " search prime " in text:
            return "search-prime"
        if " query " in text:
            return "query"
    return None


def _infer_denials(scenario: dict[str, Any], steps: list[dict[str, Any]]) -> list[str]:
    text = "\n".join(_string_values({"scenario": scenario, "steps": steps})).lower()
    denials = []
    if "repeat" in text and "pipe" in text:
        denials.append("repeat-search-pipe")
    if "direct" in text and "source" in text:
        denials.append("direct-source-read")
    if "line" in text and "selector" in text:
        denials.append("line-selector-as-action")
    return denials


def _cost_from_steps(steps: list[dict[str, Any]], usage_records: list[dict[str, Any]]) -> dict[str, Any]:
    elapsed, has_elapsed = _elapsed_ms(steps)
    cost = {
        "commands": sum(1 for step in steps if step.get("command") is not None),
        "routeTurns": len(steps),
        "inputTokens": sum(int(record["inputTokens"]) for record in usage_records),
        "outputTokens": sum(int(record["outputTokens"]) for record in usage_records),
        "totalTokens": sum(int(record["totalTokens"]) for record in usage_records),
        "usageRecords": usage_records,
    }
    if has_elapsed:
        cost["elapsedMs"] = elapsed
    _add_optional_counts(cost, steps)
    return cost


def _elapsed_ms(steps: list[dict[str, Any]]) -> tuple[float, bool]:
    elapsed = 0.0
    has_elapsed = False
    for step in steps:
        value = _number_or_none(step.get("elapsedMs") or step.get("durationMs"))
        if value is not None:
            elapsed += value
            has_elapsed = True
    return elapsed, has_elapsed


def _add_optional_counts(cost: dict[str, Any], steps: list[dict[str, Any]]) -> None:
    candidate_count = _sum_observation_ints(steps, ("candidateCount", "candidates"))
    selected_count = _sum_observation_ints(steps, ("selectedCount", "selected"))
    if candidate_count is not None:
        cost["candidateCount"] = candidate_count
    if selected_count is not None:
        cost["selectedCount"] = selected_count


def _token_usage_records(steps: list[dict[str, Any]]) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    for step in steps:
        token_cost = _token_cost(step)
        if not isinstance(token_cost, dict):
            continue
        input_tokens = _int_or_zero(token_cost.get("inputTokens"))
        output_tokens = _int_or_zero(token_cost.get("outputTokens"))
        total_tokens = _int_or_zero(token_cost.get("totalTokens")) or input_tokens + output_tokens
        records.append(_token_usage_record(token_cost, input_tokens, output_tokens, total_tokens))
    return records


def _token_cost(step: dict[str, Any]) -> Any:
    observations = step.get("observations")
    if not isinstance(observations, dict):
        observations = step
    return observations.get("tokenCost") if isinstance(observations, dict) else None


def _token_usage_record(
    token_cost: dict[str, Any],
    input_tokens: int,
    output_tokens: int,
    total_tokens: int,
) -> dict[str, Any]:
    record = {
        "provider": _first_string(token_cost.get("providers")) or _string_or_none(token_cost.get("provider")),
        "model": _first_string(token_cost.get("models")) or _string_or_none(token_cost.get("model")),
        "source": _string_or_none(token_cost.get("source")),
        "inputTokens": input_tokens,
        "outputTokens": output_tokens,
        "totalTokens": total_tokens,
    }
    return _drop_none(record)


def _sum_observation_ints(steps: list[dict[str, Any]], keys: tuple[str, ...]) -> int | None:
    total = 0
    found = False
    for step in steps:
        for candidate in _step_observation_candidates(step):
            for key in keys:
                value = candidate.get(key)
                if isinstance(value, int):
                    total += value
                    found = True
    return total if found else None


def _step_observation_candidates(step: dict[str, Any]) -> list[dict[str, Any]]:
    candidates = [step]
    observations = step.get("observations")
    if isinstance(observations, dict):
        candidates.append(observations)
    return candidates
