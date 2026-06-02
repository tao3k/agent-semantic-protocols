"""Line-protocol validation for compact sandtable output."""

from __future__ import annotations

import re

from .constants import PROJECT_PATH_PATTERN, RANK_PREFIXED_PATH_PATTERN
from .models import StepResult


def validate_line_protocol(result: StepResult, stdout: str) -> None:
    lines = [line for line in stdout.splitlines() if line.strip()]
    if not lines:
        result.errors.append("stdout has no line protocol lines")
        return
    if not lines[0].startswith("["):
        result.errors.append("first stdout line does not start with '['")
    for line in lines[1:]:
        if not (line.startswith("|") or line.startswith("[")):
            result.errors.append(f"line protocol stray line: {line[:80]!r}")
            return
    _validate_line_protocol_path_values(result, lines)


def _validate_line_protocol_path_values(result: StepResult, lines: list[str]) -> None:
    for line in lines:
        _validate_line_path_values(result, line)
        if result.errors:
            return


def _validate_line_path_values(result: StepResult, line: str) -> None:
    validators = (
        _validate_location_token,
        _validate_owner_token,
        _validate_edge_tokens,
        _validate_finding_tokens,
        _validate_named_path_fields,
        _validate_named_path_list_fields,
        _validate_next_action_path_fields,
        _validate_window_set_fields,
    )
    for validator in validators:
        validator(result, line)
        if result.errors:
            return


def _validate_location_token(result: StepResult, line: str) -> None:
    location_locator = re.match(r"^\|(?:hit|api)\s+([^\s=]+:\d+(?::\d+)?)", line)
    if location_locator is not None:
        result.errors.append(
            "line protocol location mixes path and line/column "
            f"{location_locator.group(1)!r}; use path=<projectPath> "
            "line=<line> column=<column>"
        )
        return
    location_field = re.match(r"^\|(?:hit|api)\s+path=([^,\s)]+)", line)
    if location_field is not None:
        _validate_project_path_field(result, location_field.group(1), line)


def _validate_owner_token(result: StepResult, line: str) -> None:
    owner_match = re.match(r"^\|owner\s+(\S+)", line)
    if owner_match is not None:
        _validate_project_path_token(result, owner_match.group(1), line)


def _validate_edge_tokens(result: StepResult, line: str) -> None:
    edge_match = re.match(r"^\|edge\s+(\S+)\s+-[^ ]+->\s+(\S+)", line)
    if edge_match is None:
        return
    for token in edge_match.groups():
        _validate_rank_prefixed_path_token(result, token, line)
        if result.errors:
            return


def _validate_finding_tokens(result: StepResult, line: str) -> None:
    for match in re.finditer(r"\bat=([^,\s)]+)", line):
        _validate_rank_prefixed_path_token(result, match.group(1), line)
        if result.errors:
            return


def _validate_named_path_fields(result: StepResult, line: str) -> None:
    for match in re.finditer(r"\b(?:ownerPath|owner|test)=([^,\s)]+)", line):
        value = match.group(1)
        if "/" not in value and "." not in value:
            continue
        _validate_project_path_field(result, value, line)
        if result.errors:
            return


def _validate_named_path_list_fields(result: StepResult, line: str) -> None:
    for match in re.finditer(
        (
            r"\b(?:highImpactOwners|frontierOwners|editFrontier|testFrontier|"
            r"findingOwners|high_impact_owners|frontier_owners|edit_frontier|"
            r"test_frontier|finding_owners)=([^\s]+)"
        ),
        line,
    ):
        for value in match.group(1).split(","):
            _validate_project_path_field(result, value, line)
            if result.errors:
                return


def _validate_next_action_path_fields(result: StepResult, line: str) -> None:
    for match in re.finditer(r"\b(?:next|seeds)=([^\s]+)", line):
        for action in match.group(1).split(","):
            kind, separator, target = action.partition(":")
            if separator != ":" or kind not in {"owner", "tests"}:
                continue
            target = target.split("(", maxsplit=1)[0]
            _validate_project_path_field(result, target, line)
            if result.errors:
                return


def _validate_window_set_fields(result: StepResult, line: str) -> None:
    for match in re.finditer(r"\b(?:windowSet|window_set)=([^\s]+)", line):
        for action in match.group(1).split(","):
            kind, separator, target = action.partition(":")
            if separator != ":" or kind not in {"owner", "tests", "read"}:
                result.errors.append(
                    "line protocol windowSet entry must be kind:path with "
                    f"kind owner/tests/read, got {action!r}"
                )
                return
            target = target.split("(", maxsplit=1)[0]
            _validate_project_path_field(result, target, line)
            if result.errors:
                return
            if kind == "owner" and _is_test_like_path(target):
                result.errors.append(
                    "line protocol windowSet owner target points at test path "
                    f"{target!r}; use tests:{target}"
                )
                return
            if kind == "tests" and not _is_test_like_path(target):
                result.errors.append(
                    "line protocol windowSet tests target is not test-like "
                    f"{target!r}; use owner:{target} or a concrete test path"
                )
                return


def _validate_project_path_field(result: StepResult, value: str, line: str) -> None:
    _validate_rank_prefixed_path_token(result, value, line)
    if result.errors:
        return
    _validate_project_path_token(result, value, line)


def _validate_rank_prefixed_path_token(result: StepResult, value: str, line: str) -> None:
    bad_locator = RANK_PREFIXED_PATH_PATTERN.fullmatch(value)
    if bad_locator is None:
        return
    result.errors.append(
        "line protocol path value includes non-path prefix "
        f"{bad_locator.group(0)!r}; use a project-root-relative path "
        "and put rank/slot metadata in a separate field"
    )


def _validate_project_path_token(result: StepResult, value: str, line: str) -> None:
    if PROJECT_PATH_PATTERN.fullmatch(value) is not None:
        return
    result.errors.append(
        f"line protocol invalid project path {value!r} in {line[:80]!r}"
    )


def _is_test_like_path(value: str) -> bool:
    path = value.replace("\\", "/")
    return (
        path.startswith("test/")
        or path.startswith("tests/")
        or "/test/" in f"/{path}"
        or "/tests/" in f"/{path}"
        or "/__tests__/" in f"/{path}"
        or path.endswith("_test.rs")
        or path.endswith("_test.py")
        or path.endswith(".test.ts")
        or path.endswith(".test.tsx")
        or path.endswith(".spec.ts")
        or path.endswith(".spec.tsx")
    )
