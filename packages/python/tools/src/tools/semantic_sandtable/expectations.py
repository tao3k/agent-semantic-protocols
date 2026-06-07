"""Step expectation validation for semantic sandtable runs."""

from __future__ import annotations

import re
from pathlib import Path
from typing import Any

from .budgets import warn_if_over
from .guide_quality import validate_guide_quality
from .json_expectations import validate_stdout_json
from .line_protocol import validate_line_protocol
from .models import StepResult
from .utils import dict_value, optional_float, optional_int, string_list


def validate_step(
    step: dict[str, Any],
    result: StepResult,
    stdout: str,
    stderr: str,
    repo_root: Path,
) -> None:
    expect = step.get("expect", {})
    if not isinstance(expect, dict):
        result.errors.append("step.expect must be an object")
        return

    _validate_exit_code(expect, result)
    _validate_stdout_expectations(expect, result, stdout)
    _validate_stderr_expectations(expect, result, stderr)
    validate_stdout_json(expect, result, stdout, repo_root)
    validate_guide_quality(expect, result, stdout)
    _validate_pipe_flow_expectation(expect, result)
    _validate_line_protocol_expectation(expect, result, stdout)
    _validate_budget_warnings(expect, result)


def _validate_exit_code(expect: dict[str, Any], result: StepResult) -> None:
    if bool(expect.get("allowNonZeroExit", False)):
        return
    expected_exit = int(expect.get("exitCode", 0))
    if result.exit_code != expected_exit:
        result.errors.append(f"exitCode={result.exit_code} expected={expected_exit}")


def _validate_stdout_expectations(
    expect: dict[str, Any],
    result: StepResult,
    stdout: str,
) -> None:
    for needle in string_list(expect.get("stdoutContains", [])):
        if needle not in stdout:
            result.errors.append(f"stdout missing {needle!r}")
    for needle in string_list(expect.get("stdoutNotContains", [])):
        if needle in stdout:
            result.errors.append(f"stdout unexpectedly contains {needle!r}")
    for pattern in string_list(expect.get("stdoutMatches", [])):
        if re.search(pattern, stdout, flags=re.MULTILINE) is None:
            result.errors.append(f"stdout regex missed {pattern!r}")
    if bool(expect.get("stdoutEmpty", False)) and stdout:
        result.errors.append("stdout expected empty")


def _validate_stderr_expectations(
    expect: dict[str, Any],
    result: StepResult,
    stderr: str,
) -> None:
    for needle in string_list(expect.get("stderrContains", [])):
        if needle not in stderr:
            result.errors.append(f"stderr missing {needle!r}")


def _validate_line_protocol_expectation(
    expect: dict[str, Any],
    result: StepResult,
    stdout: str,
) -> None:
    if bool(expect.get("lineProtocol", False)):
        validate_line_protocol(result, stdout)


def _validate_pipe_flow_expectation(expect: dict[str, Any], result: StepResult) -> None:
    pipe_expect = dict_value(expect.get("pipeFlow"))
    if not pipe_expect:
        return
    pipe_flow = dict_value(result.observations.get("pipeFlow"))
    token_cost = dict_value(result.observations.get("tokenCost"))
    if not pipe_flow:
        result.errors.append("pipeFlow missing from agent observations")
        return

    _validate_pipe_flow_max(pipe_expect, pipe_flow, result)
    _validate_pipe_flow_min(pipe_expect, pipe_flow, result)
    _validate_frontier_context_metrics(pipe_expect, pipe_flow, result)
    if bool(pipe_expect.get("requireComplexPipeFlow")) and not bool(
        pipe_flow.get("complexPipeFlow")
    ):
        missing = pipe_flow.get("missingComplexPipeStages", [])
        result.errors.append(f"pipeFlow complex=false missing={missing}")
    if bool(pipe_expect.get("requireTokenCost")) and not token_cost:
        result.errors.append("tokenCost missing from agent observations")
    if bool(pipe_expect.get("requireSearchPipePrecision")):
        _validate_search_pipe_precision(pipe_flow, result)
    if bool(pipe_expect.get("requireReadLoopMemory")):
        _validate_read_loop_memory(pipe_flow, result)
    if bool(pipe_expect.get("requireFailureFrontierPrecision")):
        _validate_failure_frontier_precision(pipe_flow, result)
    if bool(pipe_expect.get("requireFailureLoopMemory")):
        _validate_failure_loop_memory(pipe_flow, result)
    for stage in string_list(pipe_expect.get("requiredStages")):
        if not _pipe_flow_stage_present(stage, pipe_flow):
            result.errors.append(f"pipeFlow missing required stage {stage!r}")
    for stage in string_list(pipe_expect.get("forbiddenStages")):
        if _pipe_flow_stage_present(stage, pipe_flow):
            result.errors.append(f"pipeFlow contains forbidden stage {stage!r}")


def _validate_pipe_flow_max(
    pipe_expect: dict[str, Any],
    pipe_flow: dict[str, Any],
    result: StepResult,
) -> None:
    fields = {
        "maxAspCommands": "aspCommands",
        "maxSearchCommands": "searchCommands",
        "maxQueryCommands": "queryCommands",
        "maxGuideCommands": "guideCommands",
        "maxDirectReadCommands": "directReadCommands",
        "maxRepeatedCommands": "repeatedCommands",
        "maxSearchPipeCommands": "searchPipeCommands",
        "maxSearchPrimeCommands": "searchPrimeCommands",
        "maxSearchFailureCommands": "searchFailureCommands",
        "maxReadLoopDirectCodeCommands": "readLoopDirectCodeCommands",
        "maxReadLoopDuplicateSelectors": "readLoopDuplicateSelectors",
        "maxReadLoopAdjacentRangeWindows": "readLoopAdjacentRangeWindows",
        "maxReadLoopSameOwnerScans": "readLoopSameOwnerScans",
        "maxReadLoopMemorySuppressibleReads": "readLoopMemorySuppressibleReads",
        "maxAspCommandOutputBytes": "aspCommandOutputBytes",
    }
    for expect_key, flow_key in fields.items():
        maximum = optional_int(pipe_expect.get(expect_key))
        observed = optional_int(pipe_flow.get(flow_key))
        if maximum is not None and observed is None and flow_key == "aspCommandOutputBytes":
            result.errors.append(f"pipeFlow {flow_key} missing for {expect_key}")
            continue
        value = observed or 0
        if maximum is not None and value > maximum:
            result.errors.append(
                f"pipeFlow {flow_key}={value} exceeds {expect_key}={maximum}"
            )


def _validate_pipe_flow_min(
    pipe_expect: dict[str, Any],
    pipe_flow: dict[str, Any],
    result: StepResult,
) -> None:
    minimum = optional_int(pipe_expect.get("minQuerySelectorCommands"))
    value = optional_int(pipe_flow.get("querySelectorCommands")) or 0
    if minimum is not None and value < minimum:
        result.errors.append(
            f"pipeFlow querySelectorCommands={value} below minQuerySelectorCommands={minimum}"
        )


def _validate_frontier_context_metrics(
    pipe_expect: dict[str, Any],
    pipe_flow: dict[str, Any],
    result: StepResult,
) -> None:
    fields = {
        "minFrontierFollowRate": "frontierFollowRate",
        "minContextPrecision": "contextPrecision",
        "minContextUtilization": "contextUtilization",
    }
    for expect_key, flow_key in fields.items():
        minimum = optional_float(pipe_expect.get(expect_key))
        if minimum is None:
            continue
        observed = optional_float(pipe_flow.get(flow_key))
        if observed is None:
            result.errors.append(f"pipeFlow {flow_key} missing for {expect_key}")
            continue
        if observed < minimum:
            result.errors.append(
                f"pipeFlow {flow_key}={observed:.4f} below {expect_key}={minimum:.4f}"
            )


def _validate_search_pipe_precision(
    pipe_flow: dict[str, Any],
    result: StepResult,
) -> None:
    precision = dict_value(pipe_flow.get("searchPipeOutputPrecision"))
    if not precision:
        result.errors.append("pipeFlow searchPipeOutputPrecision missing")
        return
    minimums = {
        "fieldFacts": 1,
        "typeFacts": 1,
        "collectionFacts": 1,
        "collectionOfEdges": 1,
        "s1Selectors": 1,
        "nextCommands": 1,
        "exactQueryCoverage": 1,
    }
    for key, minimum in minimums.items():
        value = optional_int(precision.get(key)) or 0
        if value < minimum:
            result.errors.append(
                f"pipeFlow searchPipeOutputPrecision {key}={value} below {minimum}"
            )
    debug_rows = optional_int(precision.get("debugRows")) or 0
    if debug_rows > 0:
        result.errors.append(
            f"pipeFlow searchPipeOutputPrecision debugRows={debug_rows} expected=0"
        )


def _validate_read_loop_memory(pipe_flow: dict[str, Any], result: StepResult) -> None:
    memory = dict_value(pipe_flow.get("readLoopMemory"))
    if not memory:
        result.errors.append("pipeFlow readLoopMemory missing")
        return
    entries = memory.get("entries", [])
    if not isinstance(entries, list) or not entries:
        result.errors.append("pipeFlow readLoopMemory entries missing")
    entry_count = optional_int(memory.get("entryCount")) or 0
    if entry_count < 1:
        result.errors.append("pipeFlow readLoopMemory entryCount below 1")


def _validate_failure_frontier_precision(
    pipe_flow: dict[str, Any], result: StepResult
) -> None:
    precision = dict_value(pipe_flow.get("failureFrontierOutputPrecision"))
    if not precision:
        result.errors.append("pipeFlow failureFrontierOutputPrecision missing")
        return
    minimums = {
        "failureFacts": 1,
        "assertFacts": 1,
        "hotFacts": 1,
        "frontierActions": 1,
        "queryProfiles": 1,
        "omitRows": 1,
        "avoidRows": 1,
    }
    for key, minimum in minimums.items():
        value = optional_int(precision.get(key)) or 0
        if value < minimum:
            result.errors.append(
                f"pipeFlow failureFrontierOutputPrecision {key}={value} below {minimum}"
            )
    debug_rows = optional_int(precision.get("debugRows")) or 0
    if debug_rows > 0:
        result.errors.append(
            f"pipeFlow failureFrontierOutputPrecision debugRows={debug_rows} expected=0"
        )


def _validate_failure_loop_memory(
    pipe_flow: dict[str, Any], result: StepResult
) -> None:
    memory = dict_value(pipe_flow.get("failureLoopMemory"))
    if not memory:
        result.errors.append("pipeFlow failureLoopMemory missing")
        return
    entries = memory.get("entries", [])
    if not isinstance(entries, list) or not entries:
        result.errors.append("pipeFlow failureLoopMemory entries missing")
    entry_count = optional_int(memory.get("entryCount")) or 0
    if entry_count < 1:
        result.errors.append("pipeFlow failureLoopMemory entryCount below 1")


def _pipe_flow_stage_present(stage: str, pipe_flow: dict[str, Any]) -> bool:
    if stage == "search-pipe":
        return (optional_int(pipe_flow.get("searchPipeCommands")) or 0) > 0
    if stage == "search-prime":
        return (optional_int(pipe_flow.get("searchPrimeCommands")) or 0) > 0
    if stage == "search-fzf":
        return (optional_int(pipe_flow.get("searchFzfCommands")) or 0) > 0
    if stage == "search-reasoning":
        return (optional_int(pipe_flow.get("searchReasoningCommands")) or 0) > 0
    if stage == "search-failure":
        return (optional_int(pipe_flow.get("searchFailureCommands")) or 0) > 0
    if stage == "search-fzf-or-reasoning":
        fzf = optional_int(pipe_flow.get("searchFzfCommands")) or 0
        reasoning = optional_int(pipe_flow.get("searchReasoningCommands")) or 0
        return fzf + reasoning > 0
    if stage == "query-selector":
        return (optional_int(pipe_flow.get("querySelectorCommands")) or 0) > 0
    if stage == "treesitter-query":
        return (optional_int(pipe_flow.get("treesitterQueryCommands")) or 0) > 0
    if stage == "direct-read":
        return (optional_int(pipe_flow.get("directReadCommands")) or 0) > 0
    if stage == "repeated-commands":
        return (optional_int(pipe_flow.get("repeatedCommands")) or 0) > 0
    if stage == "repeated-prime":
        return (optional_int(pipe_flow.get("searchPrimeCommands")) or 0) > 1
    if stage == "read-loop-risk":
        duplicate_selectors = (
            optional_int(pipe_flow.get("readLoopDuplicateSelectors")) or 0
        )
        adjacent_windows = (
            optional_int(pipe_flow.get("readLoopAdjacentRangeWindows")) or 0
        )
        same_owner_scans = optional_int(pipe_flow.get("readLoopSameOwnerScans")) or 0
        return duplicate_selectors + adjacent_windows + same_owner_scans > 0
    if stage == "read-loop-memory-risk":
        return (
            optional_int(pipe_flow.get("readLoopMemorySuppressibleReads")) or 0
        ) > 0
    if stage == "failure-loop-memory":
        return (optional_int(pipe_flow.get("failureLoopMemoryEntryCount")) or 0) > 0
    return False


def _validate_budget_warnings(expect: dict[str, Any], result: StepResult) -> None:
    warn_if_over(
        result,
        "stdoutLines",
        result.stdout_lines,
        "maxStdoutLinesWarn",
        expect.get("maxStdoutLinesWarn"),
    )
    warn_if_over(
        result,
        "stderrLines",
        result.stderr_lines,
        "maxStderrLinesWarn",
        expect.get("maxStderrLinesWarn"),
    )
    warn_if_over(
        result,
        "stdoutBytes",
        result.stdout_bytes,
        "maxStdoutBytesWarn",
        expect.get("maxStdoutBytesWarn"),
    )
    warn_if_over(
        result,
        "elapsedMs",
        result.elapsed_ms,
        "maxElapsedMsWarn",
        expect.get("maxElapsedMsWarn"),
    )


def capture_values(
    step: dict[str, Any],
    result: StepResult,
    stdout: str,
    captures: dict[str, str],
) -> None:
    capture_spec = step.get("capture", {})
    if not isinstance(capture_spec, dict):
        return
    for name, pattern in capture_spec.items():
        if not isinstance(name, str) or not isinstance(pattern, str):
            result.errors.append("capture entries must be string to regex")
            continue
        match = re.search(pattern, stdout, flags=re.MULTILINE)
        if match is None:
            result.errors.append(f"capture {name!r} missed {pattern!r}")
            continue
        captures[name] = match.group(1) if match.groups() else match.group(0)
