"""Precision metrics for ASP search command output."""

from __future__ import annotations

import re

from .agent_observation_asp import asp_args


def search_pipe_precision_facts(command: str, output: str) -> dict[str, int]:
    args = asp_args(command)
    if len(args) < 3 or args[1:3] != ["search", "pipe"]:
        return {}
    lines = output.splitlines()
    return {
        "fieldFacts": _count_matching_lines(lines, r"^[A-Z][0-9]*=field:"),
        "typeFacts": _count_matching_lines(lines, r"^[A-Z][0-9]*=type:"),
        "collectionFacts": _count_matching_lines(lines, r"^[A-Z][0-9]*=collection:"),
        "collectionOfEdges": sum("collection_of" in line for line in lines),
        "hasTypeEdges": sum("has_type" in line for line in lines),
        "s1Selectors": sum(
            line.startswith("frontierActions=S1.selector(") for line in lines
        ),
        "nextCommands": sum(
            line.startswith("nextCommand=asp ") and " query --selector " in line
            for line in lines
        ),
        "exactQueryCoverage": sum(
            line.startswith("queryCoverage=")
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
            line.startswith("frontierActions=") and " query --selector " in line
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
