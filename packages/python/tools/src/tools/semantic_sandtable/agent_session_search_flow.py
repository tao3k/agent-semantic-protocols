"""Detect search-flow drift from recorded agent-session command output."""

from __future__ import annotations

import re
from pathlib import Path
from typing import Any

from .utils import dict_value, list_value, require_str


def search_flow_findings_from_events(
    receipt: dict[str, Any],
    events: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    artifact_root = receipt.get("artifactRoot")
    root = Path(artifact_root) if isinstance(artifact_root, str) and artifact_root else None
    path_intents = _path_intents_from_events(events)
    findings = []
    seen: set[str] = set()
    for event in events:
        if event.get("kind") != "command.result":
            continue
        output = "\n".join(_event_stdout_texts(event, root))
        event_id = require_str(event, "eventId", require_str(event, "commandId", ""))
        if _output_has_package_drift(output):
            _append_finding(
                findings,
                seen,
                "search.package-drift",
                "Search output drifted across packages after a low-cohesion query.",
                "Split the query into cohesive clauses or preserve explicit path seeds.",
                event_id,
                graph_turbo_feedback=(
                    "Penalize broad query expansion when package drift is reported."
                ),
            )
        missing_intents = [
            path for path in path_intents if path not in output and "package-drift" in output
        ]
        if missing_intents:
            _append_finding(
                findings,
                seen,
                "search.path-intent-lost",
                "A user-specified path did not survive into the search frontier.",
                "Promote explicit path-like input to a path seed before graph ranking.",
                event_id,
                graph_turbo_feedback=(
                    "Boost exact path seeds over concept-only query expansion."
                ),
                matched_selectors=missing_intents[:3],
            )
        ignored_paths = _ignored_finder_paths(output, path_intents)
        if ignored_paths:
            _append_finding(
                findings,
                seen,
                "search.finder-path-ignored",
                "Finder recovered an exact target path, but nextCommand selected another owner.",
                "When finder returns an exact path candidate, route the next step to that owner.",
                event_id,
                graph_turbo_feedback=(
                    "Boost finder-confirmed exact paths over parser-index fallback owners."
                ),
                matched_selectors=ignored_paths[:3],
            )
    return findings


def _append_finding(
    findings: list[dict[str, Any]],
    seen: set[str],
    finding_id: str,
    message: str,
    recommended_action: str,
    event_id: str,
    *,
    graph_turbo_feedback: str,
    matched_selectors: list[str] | None = None,
) -> None:
    if finding_id in seen:
        return
    finding = {
        "id": finding_id,
        "kind": "search-flow",
        "severity": "warning",
        "message": message,
        "recommendedAction": recommended_action,
        "graphTurboFeedback": graph_turbo_feedback,
        "evidenceRefs": [event_id],
    }
    if matched_selectors:
        finding["matchedSelectors"] = matched_selectors
    findings.append(finding)
    seen.add(finding_id)


def _path_intents_from_events(events: list[dict[str, Any]]) -> list[str]:
    paths: list[str] = []
    for event in events:
        if event.get("kind") == "tool.result":
            continue
        fields = dict_value(event.get("fields"))
        texts = [str(event.get("preview", "")), str(fields.get("command", ""))]
        for text in texts:
            paths.extend(_path_intents_from_text(text))
    return _dedupe_strings(paths)


def _path_intents_from_text(text: str) -> list[str]:
    tokens = [
        token.strip("'\"")
        for token in re.findall(r"[A-Za-z0-9_./-]+", text)
        if token.strip("'\"")
    ]
    paths = [
        token
        for token in tokens
        if "/" in token and (token.startswith(".") or "." in Path(token).name)
    ]
    scope = next((token for token in tokens if token.startswith(".") and "/" not in token), "")
    file_name = next((token for token in tokens if token.endswith(".ss")), "")
    directory = next(
        (token for token in tokens if "-" in token and "." not in token and not token.startswith("-")),
        "",
    )
    if scope and scope != "." and directory and file_name:
        paths.append(f"{scope}/{directory}/{file_name}")
    return paths


def _event_stdout_texts(
    event: dict[str, Any],
    artifact_root: Path | None,
) -> list[str]:
    texts = []
    if isinstance(event.get("preview"), str):
        texts.append(str(event["preview"]))
    if artifact_root is None:
        return texts
    for ref in list_value(event.get("artifactRefs")):
        if not isinstance(ref, dict) or ref.get("kind") != "stdout":
            continue
        path = ref.get("path")
        if not isinstance(path, str) or not path:
            continue
        output_path = artifact_root / path
        if output_path.is_file():
            texts.append(output_path.read_text(encoding="utf-8"))
    return texts


def _output_has_package_drift(output: str) -> bool:
    return "package-drift" in output or "packageCohesion=low" in output


def _ignored_finder_paths(output: str, path_intents: list[str]) -> list[str]:
    candidates = _owner_candidates(output)
    if not candidates:
        return []
    next_command = _line_value(output, "nextCommand")
    ignored = []
    for candidate in candidates:
        if not _is_path_like_candidate(candidate):
            continue
        if path_intents and not any(
            _same_or_related_path(candidate, path) for path in path_intents
        ):
            continue
        if next_command and candidate not in next_command:
            ignored.append(candidate)
    return _dedupe_strings(ignored)


def _owner_candidates(output: str) -> list[str]:
    value = _line_value(output, "ownerCandidates")
    if not value:
        return []
    return [item.strip() for item in value.split(",") if item.strip()]


def _line_value(output: str, name: str) -> str:
    prefix = f"{name}="
    for line in output.splitlines():
        if line.startswith(prefix):
            return line[len(prefix) :].strip()
    return ""


def _is_path_like_candidate(value: str) -> bool:
    return "/" in value and "." in Path(value).name


def _same_or_related_path(candidate: str, target: str) -> bool:
    return candidate == target or candidate.endswith("/" + target.strip("./"))


def _dedupe_strings(values: list[str]) -> list[str]:
    seen = set()
    result = []
    for value in values:
        if value and value not in seen:
            result.append(value)
            seen.add(value)
    return result
