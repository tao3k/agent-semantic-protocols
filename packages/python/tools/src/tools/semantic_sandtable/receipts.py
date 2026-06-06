"""Receipt validation and summarization helpers."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from .models import ReceiptLoadError, ReceiptResult
from .schemas import validate_receipt_schema
from .utils import dict_value, list_value, optional_int, require_str


SEARCH_COMMAND_KINDS = {"search", "external-ingest"}


def load_receipt(path: Path, repo_root: Path) -> dict[str, Any]:
    try:
        with path.open("r", encoding="utf-8") as handle:
            receipt = json.load(handle)
    except OSError as error:
        raise ReceiptLoadError(f"failed to read receipt: {error}") from error
    except json.JSONDecodeError as error:
        raise ReceiptLoadError(f"failed to parse receipt JSON: {error.msg}") from error
    validate_receipt_schema(repo_root, path, receipt)
    if not isinstance(receipt, dict):
        raise ReceiptLoadError("receipt must be an object")
    validate_receipt_consistency(receipt)
    return receipt


def validate_receipt_consistency(receipt: dict[str, Any]) -> None:
    commands = receipt.get("commands", [])
    summary = dict_value(receipt.get("summary"))
    expected_count = optional_int(summary.get("commandCount"))
    if expected_count is not None and isinstance(commands, list):
        if expected_count != len(commands):
            raise ReceiptLoadError(
                f"receipt summary.commandCount={expected_count} but commands={len(commands)}"
            )
    validate_receipt_output_mode_count(
        commands,
        optional_int(summary.get("jsonSearches")),
        "json",
        "jsonSearches",
    )
    validate_receipt_output_mode_count(
        commands,
        optional_int(summary.get("compactSearches")),
        "compact",
        "compactSearches",
    )
    validate_receipt_token_costs(commands, dict_value(summary.get("tokenCost")))


def validate_receipt_token_costs(
    commands: Any,
    summary_token_cost: dict[str, Any],
) -> None:
    if not summary_token_cost:
        return
    if not isinstance(commands, list):
        return
    expected_total = optional_int(summary_token_cost.get("totalTokens"))
    if expected_total is None:
        return

    actual_total = 0
    missing_ids: list[str] = []
    for index, command in enumerate(commands):
        if not isinstance(command, dict):
            continue
        command_id = require_str(command, "id", f"command-{index + 1}")
        metrics = dict_value(command.get("metrics"))
        token_cost = dict_value(metrics.get("tokenCost"))
        total_tokens = optional_int(token_cost.get("totalTokens"))
        if total_tokens is None:
            missing_ids.append(command_id)
            continue
        actual_total += total_tokens
    if missing_ids:
        raise ReceiptLoadError(
            "receipt summary.tokenCost requires command metrics.tokenCost for ids: "
            f"{','.join(missing_ids)}"
        )
    if actual_total != expected_total:
        raise ReceiptLoadError(
            f"receipt summary.tokenCost.totalTokens={expected_total} "
            f"but command tokenCost total={actual_total}"
        )


def validate_receipt_output_mode_count(
    commands: Any,
    expected_count: int | None,
    output_mode: str,
    summary_field: str,
) -> None:
    if expected_count is None:
        return
    if not isinstance(commands, list):
        return
    actual_count = 0
    for command in commands:
        if not isinstance(command, dict):
            continue
        if command.get("kind") not in SEARCH_COMMAND_KINDS:
            continue
        if receipt_command_output_mode(command) == output_mode:
            actual_count += 1
    if actual_count != expected_count:
        raise ReceiptLoadError(
            f"receipt summary.{summary_field}={expected_count} "
            f"but outputMode={output_mode} search commands={actual_count}"
        )


def receipt_command_output_mode(command: dict[str, Any]) -> str:
    output_mode = command.get("outputMode")
    if output_mode in {"compact", "json"}:
        return output_mode
    argv = command.get("argv", [])
    if isinstance(argv, list) and "--json" in argv:
        return "json"
    return "compact"


def validate_receipt_path(repo_root: Path, path: Path) -> ReceiptResult:
    receipt_path = path if path.is_absolute() else repo_root / path
    receipt_path = receipt_path.resolve()
    try:
        receipt = load_receipt(receipt_path, repo_root)
    except ReceiptLoadError as error:
        return ReceiptResult(path=receipt_path, status="fail", errors=[str(error)])

    summary = dict_value(receipt.get("summary"))
    return ReceiptResult(
        path=receipt_path,
        status="pass",
        scenario_id=require_str(receipt, "scenarioId", "unknown"),
        language=require_str(receipt, "language", "unknown"),
        command_count=optional_int(summary.get("commandCount")) or 0,
        stdout_bytes=optional_int(summary.get("stdoutBytes")) or 0,
        stderr_bytes=optional_int(summary.get("stderrBytes")) or 0,
        elapsed_ms=optional_int(summary.get("elapsedMs")) or 0,
        json_searches=optional_int(summary.get("jsonSearches")) or 0,
        compact_searches=optional_int(summary.get("compactSearches")) or 0,
        token_cost=dict_value(summary.get("tokenCost")),
        command_token_costs=receipt_command_token_costs(receipt),
        query_set_opportunities=[
            opportunity
            for opportunity in list_value(receipt.get("querySetOpportunities"))
            if isinstance(opportunity, dict)
        ],
        findings=[
            finding
            for finding in list_value(receipt.get("findings"))
            if isinstance(finding, dict)
        ],
    )


def receipt_command_token_costs(receipt: dict[str, Any]) -> list[dict[str, Any]]:
    costs: list[dict[str, Any]] = []
    for index, command in enumerate(list_value(receipt.get("commands"))):
        if not isinstance(command, dict):
            continue
        metrics = dict_value(command.get("metrics"))
        token_cost = dict_value(metrics.get("tokenCost"))
        if not token_cost:
            continue
        entry = dict(token_cost)
        entry["id"] = require_str(command, "id", f"command-{index + 1}")
        entry["kind"] = require_str(command, "kind", "unknown")
        stdout_bytes = optional_int(metrics.get("stdoutBytes"))
        if stdout_bytes is not None:
            entry["stdoutBytes"] = stdout_bytes
        elapsed_ms = optional_int(metrics.get("elapsedMs"))
        if elapsed_ms is not None:
            entry["elapsedMs"] = elapsed_ms
        costs.append(entry)
    return costs


def validate_linked_receipt(repo_root: Path, evidence: dict[str, Any]) -> str | None:
    receipt_path = evidence.get("receiptPath")
    if not isinstance(receipt_path, str):
        return None
    result = validate_receipt_path(repo_root, Path(receipt_path))
    if result.status == "pass":
        return None
    return "; ".join(result.errors) if result.errors else "receipt validation failed"
