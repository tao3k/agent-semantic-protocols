"""Build semantic fact frontier receipts from graph-turbo results."""

from __future__ import annotations

import hashlib
from collections.abc import Iterable, Mapping, Sequence
from dataclasses import dataclass

from .model import FrontierEntry, GraphResult, Node
from .render import owner_query_frontier_action_items, render_compact
from .selector import (
    GraphTurboSelectorRange,
    graph_turbo_node_range,
    graph_turbo_owner_path_for_node,
    graph_turbo_parse_selector,
    graph_turbo_selector_for_node,
)


@dataclass(frozen=True)
class FrontierCodeRead:
    selector: str
    read_kind: str = "exact-selector"
    owner: str | None = None
    from_frontier: bool | None = None


@dataclass(frozen=True)
class FrontierTestCommand:
    argv: tuple[str, ...]
    workdir: str | None = None
    fingerprint: str | None = None


@dataclass(frozen=True)
class FrontierTestResult:
    status: str
    summary: str
    exit_code: int | None = None


def frontier_receipt_from_result(
    result: GraphResult,
    *,
    receipt_id: str,
    task_fingerprint: str,
    command_fingerprint: str,
    receipt_kind: str = "frontier",
    followed_node_ids: Iterable[str] = (),
    code_reads: Sequence[FrontierCodeRead] = (),
    test_command: FrontierTestCommand | None = None,
    test_result: FrontierTestResult | None = None,
    edit_touched_owner: Sequence[str] = (),
    output_fingerprint: str | None = None,
    commands_to_first_useful_locator: int | None = None,
    commands_to_validation: int | None = None,
    fields: Mapping[str, object] | None = None,
) -> dict[str, object]:
    """Return a schema-shaped semantic fact frontier receipt."""

    followed_ids = tuple(followed_node_ids)
    frontier_returned = [_frontier_item(entry) for entry in result.frontier]
    frontier_followed = [
        item for item in frontier_returned if item["nodeId"] in set(followed_ids)
    ]
    reads = [_code_read(read, frontier_followed) for read in code_reads]
    compact_output = render_compact(result)
    first_item = _first_followed_item(frontier_followed, frontier_returned)
    metrics = _receipt_metrics(
        result,
        frontier_returned=frontier_returned,
        frontier_followed=frontier_followed,
        code_reads=reads,
        compact_output=compact_output,
        commands_to_first_useful_locator=commands_to_first_useful_locator,
        commands_to_validation=commands_to_validation,
    )
    return {
        "schemaId": "agent.semantic-protocols.semantic-fact-frontier-receipt",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-fact-frontier-feedback",
        "protocolVersion": "1",
        "receiptId": receipt_id,
        "receiptKind": receipt_kind,
        "taskFingerprint": task_fingerprint,
        "commandFingerprint": command_fingerprint,
        "selector": first_item.get("selector"),
        "owner": first_item.get("owner"),
        "symbol": first_item.get("symbol"),
        "range": first_item.get("range"),
        "frontierReturned": frontier_returned,
        "frontierActions": owner_query_frontier_action_items(result),
        "frontierFollowed": frontier_followed,
        "codeActuallyRead": reads,
        "testCommand": None if test_command is None else _test_command(test_command),
        "testResult": None if test_result is None else _test_result(test_result),
        "editTouchedOwner": list(edit_touched_owner),
        "outputFingerprint": output_fingerprint or _sha256_fingerprint(compact_output),
        "metrics": metrics,
        "fields": dict(fields or {}),
    }


def _frontier_item(entry: FrontierEntry) -> dict[str, object]:
    node = entry.node
    selector = graph_turbo_selector_for_node(node)
    node_range = graph_turbo_node_range(node)
    item: dict[str, object] = {
        "nodeId": node.id,
        "action": entry.action,
        "selector": selector,
        "owner": graph_turbo_owner_path_for_node(node),
        "symbol": _node_symbol(node),
        "range": _source_range(node_range),
    }
    confidence = _node_field(node, "confidence")
    freshness = _node_field(node, "freshness")
    if confidence is not None:
        item["confidence"] = confidence
    if freshness is not None:
        item["freshness"] = freshness
    return item


def _node_field(node: Node, name: str) -> str | None:
    nested = node.fields.get("fields")
    if isinstance(nested, Mapping):
        value = nested.get(name)
        if isinstance(value, str) and value:
            return value
    value = node.fields.get(name)
    if isinstance(value, str) and value:
        return value
    return None


def _node_symbol(node: Node) -> str | None:
    symbol = node.fields.get("symbol")
    if isinstance(symbol, str) and symbol:
        return symbol
    return node.value if node.value else None


def _code_read(
    read: FrontierCodeRead, followed_items: Sequence[Mapping[str, object]]
) -> dict[str, object]:
    selector_range = graph_turbo_parse_selector(read.selector)
    owner = read.owner
    if owner is None and selector_range is not None:
        owner = selector_range.path
    from_frontier = read.from_frontier
    if from_frontier is None:
        followed_selectors = {
            item.get("selector")
            for item in followed_items
            if isinstance(item.get("selector"), str)
        }
        from_frontier = read.selector in followed_selectors
    return {
        "selector": read.selector,
        "owner": owner or read.selector,
        "range": _source_range(selector_range) or {
            "path": owner or read.selector,
            "startLine": 1,
            "endLine": 1,
        },
        "readKind": read.read_kind,
        "fromFrontier": from_frontier,
    }


def _test_command(command: FrontierTestCommand) -> dict[str, object]:
    packet: dict[str, object] = {"argv": list(command.argv)}
    if command.workdir is not None:
        packet["workdir"] = command.workdir
    if command.fingerprint is not None:
        packet["fingerprint"] = command.fingerprint
    return packet


def _test_result(result: FrontierTestResult) -> dict[str, object]:
    packet: dict[str, object] = {
        "status": result.status,
        "summary": result.summary,
    }
    if result.exit_code is not None:
        packet["exitCode"] = result.exit_code
    return packet


def _first_followed_item(
    followed: Sequence[Mapping[str, object]],
    returned: Sequence[Mapping[str, object]],
) -> Mapping[str, object]:
    if followed:
        return followed[0]
    if returned:
        return returned[0]
    return {"selector": None, "owner": None, "symbol": None, "range": None}


def _receipt_metrics(
    result: GraphResult,
    *,
    frontier_returned: Sequence[Mapping[str, object]],
    frontier_followed: Sequence[Mapping[str, object]],
    code_reads: Sequence[Mapping[str, object]],
    compact_output: str,
    commands_to_first_useful_locator: int | None,
    commands_to_validation: int | None,
) -> dict[str, object]:
    returned_count = len(frontier_returned)
    followed_count = len(frontier_followed)
    metrics = result.algorithm_metrics
    return {
        "frontierReturnedCount": returned_count,
        "frontierFollowedCount": followed_count,
        "frontierFollowRate": (
            0.0 if returned_count == 0 else round(followed_count / returned_count, 6)
        ),
        "codeActuallyReadCount": len(code_reads),
        "rawReadFallbackCount": sum(
            1 for read in code_reads if read.get("readKind") == "raw-read"
        ),
        "duplicateSelectorCount": metrics.read_loop_duplicate_selector_count,
        "sameOwnerScanCount": metrics.read_loop_same_owner_scan_count,
        "relationChannelCount": metrics.relation_channel_count,
        "pprIterations": metrics.ppr_iterations,
        "pprResidual": metrics.ppr_residual,
        "stdoutBytesToFrontier": len(compact_output.encode("utf-8")),
        "commandsToFirstUsefulLocator": commands_to_first_useful_locator,
        "commandsToValidation": commands_to_validation,
    }


def _source_range(
    selector_range: GraphTurboSelectorRange | None,
) -> dict[str, object] | None:
    if selector_range is None:
        return None
    return {
        "path": selector_range.path,
        "startLine": selector_range.start_line,
        "endLine": selector_range.end_line,
    }


def _sha256_fingerprint(text: str) -> str:
    return "sha256:" + hashlib.sha256(text.encode("utf-8")).hexdigest()
