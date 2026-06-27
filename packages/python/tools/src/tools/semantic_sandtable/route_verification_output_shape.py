"""Detect route-verification risks from command output shape."""

from __future__ import annotations

import re
from typing import Any

from .utils import dict_value


GRAPH_NODE_RE = re.compile(r"^([A-Z][A-Z0-9]*)=([a-z][a-z0-9-]*):")
EVIDENCE_NODE_KIND_RE = re.compile(r"\bkind=([a-z][a-z0-9-]*)\b")
OWNER_TOPOLOGY_NODE_KINDS = {
    "owner",
    "workspace",
    "provider-root",
    "submodule",
}
NON_ACTIONABLE_NODE_KINDS = {"search", "query"}
FRONTIER_FIELD_PREFIXES = ("rank=", "frontier=")
FRONTIER_FIELD_TOKENS = ("rankedEvidence=", "evidenceFrontier=")


def has_owner_only_frontier_redundancy(command: dict[str, Any]) -> bool:
    stdout = command_stdout(command)
    if not stdout or not has_frontier_projection(stdout):
        return False
    graph_kinds = graph_node_kinds(stdout)
    if graph_kinds:
        return has_only_owner_topology_actionable_nodes(graph_kinds)
    evidence_kinds = evidence_node_kinds(stdout)
    if evidence_kinds:
        return has_only_owner_topology_actionable_nodes(evidence_kinds)
    return False


def command_stdout(command: dict[str, Any]) -> str:
    for value in stdout_candidates(command):
        if isinstance(value, str) and value.strip():
            return value
    return ""


def stdout_candidates(command: dict[str, Any]) -> list[Any]:
    candidates: list[Any] = [
        command.get("stdout"),
        command.get("stdoutText"),
        command.get("output"),
        command.get("outputText"),
        command.get("commandOutput"),
    ]
    for key in ("result", "payload", "event", "data"):
        nested = dict_value(command.get(key))
        candidates.extend(
            [
                nested.get("stdout"),
                nested.get("stdoutText"),
                nested.get("output"),
                nested.get("outputText"),
            ]
        )
    return candidates


def has_only_owner_topology_actionable_nodes(kinds: set[str]) -> bool:
    actionable = kinds - NON_ACTIONABLE_NODE_KINDS
    return bool(actionable) and actionable <= OWNER_TOPOLOGY_NODE_KINDS


def has_frontier_projection(stdout: str) -> bool:
    for line in stdout.splitlines():
        stripped = line.strip()
        if stripped.startswith(FRONTIER_FIELD_PREFIXES):
            return True
        if any(token in stripped for token in FRONTIER_FIELD_TOKENS):
            return True
    return False


def graph_node_kinds(stdout: str) -> set[str]:
    kinds: set[str] = set()
    for line in stdout.splitlines():
        match = GRAPH_NODE_RE.match(line.strip())
        if match:
            kinds.add(match.group(2))
    return kinds


def evidence_node_kinds(stdout: str) -> set[str]:
    kinds: set[str] = set()
    for line in stdout.splitlines():
        if "evidenceNodes=" not in line:
            continue
        kinds.update(EVIDENCE_NODE_KIND_RE.findall(line))
    return kinds
