"""Precision metrics for ASP search command output."""

from __future__ import annotations

import re

from .agent_observation_asp import asp_args


def search_pipe_precision_facts(command: str, output: str) -> dict[str, int]:
    args = asp_args(command)
    if len(args) < 3 or args[1:3] != ["search", "pipe"]:
        return {}
    lines = output.splitlines()
    collection_facts = _count_fact_kind(lines, "collection")
    collection_edges = sum("collection_of" in line for line in lines)
    return {
        "fieldFacts": _count_fact_kind(lines, "field"),
        "typeFacts": _count_fact_kind(lines, "type"),
        "collectionFacts": collection_facts,
        "collectionOfEdges": max(collection_edges, collection_facts),
        "hasTypeEdges": sum("has_type" in line for line in lines),
        "s1Selectors": sum(
            line.startswith("frontierActions=S1.selector(") for line in lines
        )
        + _count_evidence_frontier_locator_rows(lines),
        "nextCommands": sum(line.startswith("nextCommand=asp ") for line in lines),
        "exactQueryCoverage": sum(
            line.startswith(("queryCoverage=", "globalCoverage="))
            and "matched=" in line
            and " missing=" in line
            for line in lines
        ),
        "debugRows": sum(
            line.startswith(("scores=", "paths=", "trace=", "explain="))
            for line in lines
        ),
    }


def failure_frontier_precision_facts(command: str, output: str) -> dict[str, int]:
    args = asp_args(command)
    if len(args) < 3 or args[1:3] != ["search", "failure"]:
        return {}
    lines = output.splitlines()
    return {
        "failureFacts": _count_matching_lines(lines, r"^[A-Z][0-9]*=failure:"),
        "assertFacts": _count_matching_lines(lines, r"^[A-Z][0-9]*=assert:"),
        "ownerFacts": _count_matching_lines(lines, r"^[A-Z][0-9]*=owner:"),
        "hotFacts": _count_matching_lines(lines, r"^[A-Z][0-9]*=hot:"),
        "keyFacts": _count_matching_lines(lines, r"^[A-Z][0-9]*=key:"),
        "evidenceFacts": _count_matching_lines(lines, r"^[A-Z][0-9]*=evidence:"),
        "frontierActions": sum(
            line.startswith("frontierActions=") and ".query-code(" in line
            for line in lines
        ),
        "queryProfiles": sum(line.startswith("queryProfiles=") for line in lines),
        "omitRows": sum(line.startswith("omit=") for line in lines),
        "avoidRows": sum(line.startswith("avoid=") for line in lines),
        "debugRows": sum(
            line.startswith(("scores=", "paths=", "trace=", "explain="))
            for line in lines
        ),
    }


def _count_matching_lines(lines: list[str], pattern: str) -> int:
    regex = re.compile(pattern)
    return sum(bool(regex.search(line)) for line in lines)


def _count_fact_kind(lines: list[str], kind: str) -> int:
    prefix = f"{kind}:"
    return sum(
        bool(re.match(r"^[A-Z][0-9]*=", segment)) and prefix in segment
        for segment in _fact_segments(lines)
    )


def _fact_segments(lines: list[str]) -> list[str]:
    segments: list[str] = []
    for line in lines:
        body = line.removeprefix("evidenceNodes=")
        segments.extend(segment for segment in body.split(";") if segment)
    return segments


def _count_evidence_frontier_locator_rows(lines: list[str]) -> int:
    return sum(
        line.startswith("evidenceFrontier=")
        and any(marker in line for marker in (".syntax", ".hot", ".code"))
        for line in lines
    )
