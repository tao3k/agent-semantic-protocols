"""Line-protocol validation for compact sandtable output."""

from __future__ import annotations

import re

from .constants import PROJECT_PATH_PATTERN, RANK_PREFIXED_PATH_PATTERN
from .models import StepResult

COMPACT_GRAPH_MICRO_LEGEND = (
    "legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next"
)


def validate_line_protocol(result: StepResult, stdout: str) -> None:
    lines = [line for line in stdout.splitlines() if line.strip()]
    if not lines:
        result.errors.append("stdout has no line protocol lines")
        return
    if not lines[0].startswith("["):
        result.errors.append("first stdout line does not start with '['")
    for line in lines[1:]:
        if not (
            line.startswith("|") or line.startswith("[") or _is_compact_graph_line(line)
        ):
            result.errors.append(f"line protocol stray line: {line[:80]!r}")
            return
    _validate_compact_graph_contract(result, lines)
    _validate_line_protocol_path_values(result, lines)


def _validate_compact_graph_contract(result: StepResult, lines: list[str]) -> None:
    if not _requires_compact_graph_contract(result.command) and not any(
        _is_compact_graph_line(line) for line in lines
    ):
        return
    if COMPACT_GRAPH_MICRO_LEGEND not in lines:
        result.errors.append("compact graph missing micro-legend line")
    alias_lines = [line for line in lines if line.startswith("alias: graph:{")]
    if not alias_lines:
        result.errors.append("compact graph missing alias legend line")
    elif "G=search" not in alias_lines[0]:
        result.errors.append("compact graph alias legend missing G=search")
    if not any(line.startswith("G>{") for line in lines):
        result.errors.append("compact graph missing root edge line")
    if not any(line.startswith("rank=") and " frontier=" in line for line in lines):
        result.errors.append("compact graph missing rank/frontier line")
    if any(line.startswith("|seed") or line.startswith("|synthesis") for line in lines):
        result.errors.append(
            "compact graph output must not include legacy seed/synthesis rows"
        )


def _requires_compact_graph_contract(command: list[str]) -> bool:
    if "search" not in command:
        return False
    for index, arg in enumerate(command):
        if (
            arg == "--view"
            and index + 1 < len(command)
            and command[index + 1] == "seeds"
        ):
            return True
        if arg == "--view=seeds":
            return True
    return False


def _is_compact_graph_line(line: str) -> bool:
    if line == COMPACT_GRAPH_MICRO_LEGEND:
        return True
    if line.startswith("alias: graph:{"):
        return _looks_like_compact_graph_legend_line(line)
    if line.startswith("rank="):
        return " frontier=" in line
    if line.startswith("entries="):
        return _looks_like_compact_graph_entries_line(line)
    if line.startswith("omit=") or line.startswith("avoid="):
        return _looks_like_compact_graph_csv_metadata_line(line)
    if ">{" in line and line.endswith("}"):
        return _looks_like_compact_graph_edge_line(line)
    return _looks_like_compact_graph_alias_line(line)


def _looks_like_compact_graph_legend_line(line: str) -> bool:
    if not line.endswith("}"):
        return False
    entries = line.removeprefix("alias: graph:{")[:-1].split(",")
    if not entries:
        return False
    for entry in entries:
        alias, sep, node_type = entry.partition("=")
        if not sep or not _looks_like_compact_graph_alias_id(alias):
            return False
        if not node_type.replace("_", "").isalnum():
            return False
    return True


def _looks_like_compact_graph_edge_line(line: str) -> bool:
    source, _, edge_targets = line.partition(">{")
    if not _looks_like_compact_graph_alias_id(source):
        return False
    edge_targets = edge_targets[:-1]
    if not edge_targets:
        return source == "G"
    for edge in edge_targets.split(","):
        target, sep, relation = edge.partition(":")
        if not sep or not _looks_like_compact_graph_alias_id(target):
            return False
        if not relation.replace("_", "").isalnum():
            return False
    return True


def _looks_like_compact_graph_entries_line(line: str) -> bool:
    entries = _compact_graph_return_entries(line)
    return bool(entries) and all(
        _looks_like_compact_graph_return_entry(entry) for entry in entries
    )


def _compact_graph_return_entries(line: str) -> list[str]:
    entries = line.removeprefix("entries=").split("),")
    return [
        f"{entry})" if index + 1 < len(entries) else entry
        for index, entry in enumerate(entries)
    ]


def _looks_like_compact_graph_return_entry(entry: str) -> bool:
    profile, sep, selector_and_returns = entry.partition("(")
    if not sep or not profile or not selector_and_returns.endswith(")"):
        return False
    if not _looks_like_compact_graph_metadata_value(profile):
        return False
    selectors, sep, returns = selector_and_returns[:-1].partition("=>")
    if not sep:
        return False
    selector_values = selectors.split(",")
    return_values = returns.split("+")
    return (
        bool(selector_values)
        and bool(return_values)
        and all(
            _looks_like_compact_graph_alias_id(selector) for selector in selector_values
        )
        and all(
            _looks_like_compact_graph_metadata_value(value) for value in return_values
        )
    )


def _looks_like_compact_graph_csv_metadata_line(line: str) -> bool:
    _, sep, values = line.partition("=")
    return bool(sep and values) and all(
        _looks_like_compact_graph_metadata_value(value) for value in values.split(",")
    )


def _looks_like_compact_graph_metadata_value(value: str) -> bool:
    return value.replace("-", "").replace("_", "").isalnum()


def _looks_like_compact_graph_alias_line(line: str) -> bool:
    for entry in line.split(";"):
        alias, sep, fact = entry.partition("=")
        if not sep or not alias:
            return False
        if not _looks_like_compact_graph_alias_id(alias):
            return False
        if ":" not in fact or "!" not in fact:
            return False
    return True


def _looks_like_compact_graph_alias_id(value: str) -> bool:
    if not value or not value[0].isalpha():
        return False
    return value.replace("_", "").isalnum()


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
            _validate_window_set_action(result, action, line)
            if result.errors:
                return


def _validate_window_set_action(result: StepResult, action: str, line: str) -> None:
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

    _validate_window_set_target_kind(result, kind, target)


def _validate_window_set_target_kind(
    result: StepResult, kind: str, target: str
) -> None:
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


def _validate_project_path_field(result: StepResult, value: str, line: str) -> None:
    _validate_rank_prefixed_path_token(result, value, line)
    if result.errors:
        return
    _validate_project_path_token(result, value, line)


def _validate_rank_prefixed_path_token(
    result: StepResult, value: str, line: str
) -> None:
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
