"""Receipt report rendering."""

from __future__ import annotations

from pathlib import Path
from typing import Any

from .models import ReceiptResult
from .output import emit
from .report_format import quote_value
from .utils import dict_value, optional_int, require_str, string_list


def print_receipt_report(repo_root: Path, results: list[ReceiptResult]) -> None:
    pass_count = sum(1 for result in results if result.status == "pass")
    fail_count = sum(1 for result in results if result.status == "fail")
    emit(
        "[receipt] "
        f"receipts={len(results)} pass={pass_count} fail={fail_count}"
    )
    for result in results:
        path = result.path
        try:
            path = result.path.relative_to(repo_root)
        except ValueError:
            pass
        emit(
            f"|receipt scenario={result.scenario_id} lang={result.language} "
            f"status={result.status} path={path} commands={result.command_count} "
            f"stdoutBytes={result.stdout_bytes} stderrBytes={result.stderr_bytes} "
            f"elapsedMs={result.elapsed_ms} jsonSearches={result.json_searches} "
            f"compactSearches={result.compact_searches}"
        )
        if result.token_cost:
            _print_receipt_token_cost(result.token_cost)
        for command_token_cost in result.command_token_costs:
            _print_receipt_command_token_cost(command_token_cost)
        for opportunity in result.query_set_opportunities:
            _print_receipt_query_set_opportunity(opportunity)
        for finding in result.findings:
            kind = require_str(finding, "kind", "unknown")
            severity = require_str(finding, "severity", "info")
            message = require_str(finding, "message", "")
            emit(
                f"|finding kind={kind} severity={severity} "
                f"message={quote_value(message)}"
            )
        for error in result.errors:
            emit(f"|error {error}")


def _print_receipt_token_cost(token_cost: dict[str, Any]) -> None:
    total_tokens = optional_int(token_cost.get("totalTokens"))
    if total_tokens is None:
        return
    line = f"|tokenCost totalTokens={total_tokens}"
    input_tokens = optional_int(token_cost.get("inputTokens"))
    output_tokens = optional_int(token_cost.get("outputTokens"))
    unit = token_cost.get("unit")
    basis = token_cost.get("basis")
    if input_tokens is not None:
        line = f"{line} inputTokens={input_tokens}"
    if output_tokens is not None:
        line = f"{line} outputTokens={output_tokens}"
    if isinstance(unit, str):
        line = f"{line} unit={quote_value(unit)}"
    if isinstance(basis, str):
        line = f"{line} basis={quote_value(basis)}"
    emit(line)


def _print_receipt_command_token_cost(token_cost: dict[str, Any]) -> None:
    command_id = token_cost.get("id")
    total_tokens = optional_int(token_cost.get("totalTokens"))
    if not isinstance(command_id, str) or total_tokens is None:
        return
    line = f"|commandTokenCost id={quote_value(command_id)} totalTokens={total_tokens}"
    emit(_append_token_cost_fields(line, token_cost))


def _append_token_cost_fields(line: str, token_cost: dict[str, Any]) -> str:
    for field in ("inputTokens", "outputTokens", "stdoutBytes", "elapsedMs"):
        value = optional_int(token_cost.get(field))
        if value is not None:
            line = f"{line} {field[0].lower()}{field[1:]}={value}"
    for field in ("unit", "basis"):
        value = token_cost.get(field)
        if isinstance(value, str):
            line = f"{line} {field}={quote_value(value)}"
    return line


def _print_receipt_query_set_opportunity(opportunity: dict[str, Any]) -> None:
    view = require_str(opportunity, "view", "unknown")
    queries = optional_int(opportunity.get("queries")) or 0
    save_commands = optional_int(opportunity.get("saveCommands")) or 0
    selector = require_str(opportunity, "selector", "-")
    line = (
        f"|merge view={view} queries={queries} "
        f"saveCommands={save_commands} selector={selector}"
    )
    scope = dict_value(opportunity.get("scope"))
    owner_path = scope.get("ownerPath")
    if isinstance(owner_path, str):
        line = f"{line} owner={quote_value(owner_path)}"
    terms = string_list(opportunity.get("terms"))
    if terms:
        line = f"{line} terms={len(terms)}"
    reason = opportunity.get("reason")
    if isinstance(reason, str):
        line = f"{line} reason={quote_value(reason)}"
    emit(line)
