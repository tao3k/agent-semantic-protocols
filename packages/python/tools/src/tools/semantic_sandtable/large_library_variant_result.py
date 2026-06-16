"""Build large-library optimization variant result packets."""

from __future__ import annotations

from typing import Any

from .utils import dict_value, list_value, optional_int, require_str

VARIANT_RESULT_SCHEMA_ID = (
    "agent.semantic-protocols.semantic-sandtable-large-library-optimization-variant-result"
)


def build_large_library_variant_result(
    report_chain: dict[str, Any],
    *,
    variant_run_id: str,
    receipt_metrics: dict[str, Any],
    answer_metrics: dict[str, Any],
    source_receipt_path: str | None = None,
) -> dict[str, Any]:
    run, variant = _find_variant_run(report_chain, variant_run_id)
    _require_metrics(
        "receiptMetrics",
        receipt_metrics,
        list_value(dict_value(report_chain.get("optimizationBatch")).get("requiredReceiptMetrics")),
    )
    _require_metrics(
        "answerMetrics",
        answer_metrics,
        list_value(dict_value(report_chain.get("optimizationBatch")).get("requiredAnswerMetrics")),
    )
    packet = {
        "schemaId": VARIANT_RESULT_SCHEMA_ID,
        "schemaVersion": "1",
        "packetKind": "large-library-optimization-variant-result",
        "variantRunId": variant_run_id,
        "runId": require_str(run, "runId", "unknown"),
        "language": require_str(run, "language", "unknown"),
        "scenarioId": require_str(run, "scenarioId", "unknown"),
        "scenarioPath": require_str(run, "scenarioPath", "unknown"),
        "package": require_str(run, "package", "unknown"),
        "questionId": require_str(run, "questionId", "unknown"),
        "depthBucket": require_str(run, "depthBucket", "unknown"),
        "ablationVariant": variant,
        "targetGraphPhase": require_str(run, "targetGraphPhase", "query-first-stage"),
        "receiptMetrics": dict(receipt_metrics),
        "answerMetrics": dict(answer_metrics),
    }
    if source_receipt_path:
        packet["sourceReceiptPath"] = source_receipt_path
    return packet


def _find_variant_run(
    report_chain: dict[str, Any], variant_run_id: str
) -> tuple[dict[str, Any], str]:
    for run in list_value(report_chain.get("optimizationMatrix")):
        if not isinstance(run, dict):
            continue
        for variant in list_value(run.get("ablationVariants")):
            if not isinstance(variant, str):
                continue
            candidate = f"{require_str(run, 'runId', 'unknown')}:{variant}"
            if candidate == variant_run_id:
                return run, variant
    raise ValueError(f"unknown variantRunId: {variant_run_id}")


def _require_metrics(
    section: str,
    metrics: dict[str, Any],
    required: list[object],
) -> None:
    missing = [name for name in required if isinstance(name, str) and name not in metrics]
    if missing:
        raise ValueError(f"{section} missing required metrics: {','.join(missing)}")


def parse_metric_values(values: list[str]) -> dict[str, Any]:
    metrics: dict[str, Any] = {}
    for value in values:
        key, separator, raw = value.partition("=")
        if not separator or not key:
            raise ValueError(f"metric must be KEY=VALUE: {value}")
        metrics[key] = _parse_metric_value(raw)
    return metrics


def _parse_metric_value(value: str) -> int | float | str | bool:
    if value in {"true", "false"}:
        return value == "true"
    parsed_int = optional_int(value)
    if parsed_int is not None:
        return parsed_int
    try:
        return float(value)
    except ValueError:
        return value
