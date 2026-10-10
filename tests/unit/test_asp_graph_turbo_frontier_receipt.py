# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Frontier receipt capture tests for graph-turbo."""

from __future__ import annotations

from unit._asp_graph_turbo_common import (
    Path,
    TypedGraph,
    json,
    rank_frontier,
    sample_request,
    schema_validator_for,
    subprocess,
    sys,
)

from asp_python_graphs.frontier_receipt import (
    FrontierCodeRead,
    FrontierTestCommand,
    FrontierTestResult,
    frontier_receipt_from_result,
)


_REPO_ROOT = Path(__file__).resolve().parents[2]
_RECEIPT_SCHEMA = (
    _REPO_ROOT / "schemas" / "semantic-fact-frontier-receipt.v1.schema.json"
)


def test_frontier_receipt_capture_from_result_is_schema_valid() -> None:
    graph = sample_request()["graph"]
    result = rank_frontier(
        TypedGraph.from_packet(graph),
        profile="owner-query",
        seeds=["q:parser", "owner:cli"],
        cache_enabled=False,
    )
    receipt = frontier_receipt_from_result(
        result,
        receipt_id="test.frontier-receipt",
        task_fingerprint="task:test-frontier",
        command_fingerprint="command:test-frontier",
        followed_node_ids=("item:collect",),
        code_reads=(FrontierCodeRead("src/cli.py:10:20"),),
        test_command=FrontierTestCommand(("pytest", "tests/test_cli.py"), workdir="."),
        test_result=FrontierTestResult("passed", "focused test passed", 0),
        commands_to_first_useful_locator=1,
        commands_to_validation=3,
    )

    assert receipt["frontierFollowed"][0]["nodeId"] == "item:collect"
    assert receipt["codeActuallyRead"][0]["fromFrontier"] is True
    assert receipt["metrics"]["frontierReturnedCount"] == len(receipt["frontierReturned"])
    assert list(schema_validator_for(_RECEIPT_SCHEMA).iter_errors(receipt)) == []


def test_frontier_receipt_cli_emits_schema_valid_receipt() -> None:
    command = [
        sys.executable,
        "-m",
        "asp_python_graphs",
        "receipt",
        "--receipt-id",
        "test.frontier-receipt-cli",
        "--task-fingerprint",
        "task:test-frontier-cli",
        "--command-fingerprint",
        "command:test-frontier-cli",
        "--follow-node",
        "item:collect",
        "--read-selector",
        "src/cli.py:10:20",
        "--test-argv-json",
        "[\"pytest\", \"tests/test_cli.py\"]",
        "--test-status",
        "passed",
        "--test-summary",
        "focused test passed",
        "--test-exit-code",
        "0",
        "--commands-to-first-useful-locator",
        "1",
        "--commands-to-validation",
        "3",
        "-",
    ]
    completed = subprocess.run(
        command,
        input=json.dumps(sample_request()),
        text=True,
        capture_output=True,
        check=True,
    )
    receipt = json.loads(completed.stdout)

    assert receipt["schemaId"] == "agent.semantic-protocols.semantic-fact-frontier-receipt"
    assert receipt["frontierFollowed"][0]["nodeId"] == "item:collect"
    assert receipt["metrics"]["commandsToValidation"] == 3
    assert list(schema_validator_for(_RECEIPT_SCHEMA).iter_errors(receipt)) == []
