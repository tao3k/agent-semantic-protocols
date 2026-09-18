# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Executable reference contract for compact Org graph data fragments.

Authority records and current source identities are trusted caller inputs from
admission, never data accepted from the display packet. This module checks their
consistency; it neither authenticates them, runs jq, nor implements standard GQL.
"""

from __future__ import annotations

import json
from collections import defaultdict

from jsonschema import Draft202012Validator
from jsonschema.exceptions import ValidationError


class ProjectionError(ValueError):
    """A failed projection must produce no partial display."""


def _require(condition: bool, reason: str) -> None:
    if not condition:
        raise ProjectionError(reason)


def _surface(node: dict) -> dict:
    return {key: node[key] for key in ("selector", "path", "jq", "lines") if key in node}


def _encoded(value: object) -> str:
    # JSON string escaping prevents raw Org delimiters/newlines in a property.
    return json.dumps(value, ensure_ascii=False, separators=(",", ":")).replace(
        "\u2028", "\\u2028"
    ).replace("\u2029", "\\u2029").replace("\u0085", "\\u0085")


def _line_count(windows: list[tuple[int, int]]) -> int:
    total, start, end = 0, 0, -1
    for left, right in sorted(windows):
        if left > end + 1:
            total += max(0, end - start + 1)
            start, end = left, right
        else:
            end = max(end, right)
    return total + max(0, end - start + 1)


def validate_projection(packet: dict, authority: dict, sources: dict, schema: dict) -> None:
    """Validate shape, bound read surfaces, graph references and source budget.

    `sources` belongs to the same admitted snapshot, not an opportunistic live
    filesystem scan. Read witnesses bind kind, path, digest and exact surface.
    A selector is admitted by its witness, not by the schema's URI pattern.
    """
    try:
        Draft202012Validator(schema).validate(packet)
    except ValidationError as exc:
        raise ProjectionError("schema-invalid") from exc

    aliases: dict[str, dict] = {}
    windows: dict[str, list[tuple[int, int]]] = defaultdict(list)
    for node in packet["nodes"]:
        _require(node["id"] not in aliases, "duplicate-alias")
        aliases[node["id"]] = node
        record = authority.get("nodes", {}).get(node["witness"])
        _require(isinstance(record, dict), "witness-unavailable")
        _require(record.get("binding") == packet["binding"], "binding-mismatch")
        _require(record.get("surface") == _surface(node), "surface-mismatch")
        _require(record.get("kind") == node["kind"], "kind-mismatch")
        path = record.get("path")
        _require(isinstance(path, str) and path in sources, "source-unavailable")
        source = sources[path]
        _require(source.get("contentDigest") is not None and
                 source["contentDigest"] == record.get("contentDigest"), "content-mismatch")
        if "path" in node:
            _require(path == node["path"], "surface-mismatch")
        if "jq" in node:
            _require(bool(node["jq"].strip()), "extraction-empty")
            _require(record.get("selectionVerified") is True and
                     record.get("resultKind") in {"value", "explicit-null"},
                     "extraction-unverified")
        if "lines" in node:
            previous = -1
            for start, end in node["lines"]:
                _require(start <= end and start > previous + 1, "line-order")
                _require(isinstance(source.get("lineCount"), int) and
                         end <= source["lineCount"], "line-bounds")
                windows[path].append((start, end))
                previous = end

    _require(sum(_line_count(value) for value in windows.values()) <=
             packet["contextBudgetLines"], "context-budget")

    seen_edges: set[tuple[str, str, str, str]] = set()
    for edge in packet["edges"]:
        _require(edge["from"] in aliases and edge["to"] in aliases, "dangling-edge")
        key = (edge["from"], edge["to"], edge["relation"], edge["witness"])
        _require(key not in seen_edges, "duplicate-edge")
        seen_edges.add(key)
        record = authority.get("edges", {}).get(edge["witness"])
        _require(isinstance(record, dict), "witness-unavailable")
        _require(record.get("binding") == packet["binding"], "binding-mismatch")
        _require(record.get("fromWitness") == aliases[edge["from"]]["witness"] and
                 record.get("toWitness") == aliases[edge["to"]]["witness"] and
                 record.get("relation") == edge["relation"], "relation-mismatch")


def render_projection(packet: dict, authority: dict, sources: dict, schema: dict) -> str:
    """Return one complete validated fragment, preserving supplied node order."""
    validate_projection(packet, authority, sources, schema)
    lines = [f'#+begin_src gql :profile search-evidence.v1 :binding "{packet["binding"]}"'
             f' :coverage partial :context-budget-lines {packet["contextBudgetLines"]} :eval never']
    for node in packet["nodes"]:
        fields = _surface(node) | {"witness": node["witness"]}
        properties = ", ".join(f"{key}:{_encoded(value)}" for key, value in fields.items())
        lines.append(f'({node["id"]}:{node["kind"]} {{{properties}}})')
    for edge in packet["edges"]:
        lines.append(f'({edge["from"]})-[:{edge["relation"]} '
                     f'{{witness:{_encoded(edge["witness"])}}}]->({edge["to"]})')
    lines.append("#+end_src")
    return "\n".join(lines) + "\n"
