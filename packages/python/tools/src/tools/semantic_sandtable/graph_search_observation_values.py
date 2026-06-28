"""Value normalization helpers for graph search observation packets."""

from __future__ import annotations

from typing import Any, Iterable

from tools.semantic_sandtable.graph_search_observation_contract import (
    _drop_none,
    _int_or_none,
    _number_or_none,
    _safe_optional,
    _safe_scalar,
    is_absolute_path,
)


def _list_of_dicts(value: Any) -> list[dict[str, Any]]:
    return [item for item in _list(value) if isinstance(item, dict)]


def _list(value: Any) -> list[Any]:
    if isinstance(value, list):
        return value
    if value is None:
        return []
    return [value]


def _string_list(value: Any) -> list[str]:
    return [_safe_scalar(item) for item in _list(value) if isinstance(item, str)]


def _string_values(value: Any) -> Iterable[str]:
    if isinstance(value, str):
        yield value
    elif isinstance(value, dict):
        for item in value.values():
            yield from _string_values(item)
    elif isinstance(value, list):
        for item in value:
            yield from _string_values(item)


def _command_text(command: Any) -> str:
    if isinstance(command, list):
        return " ".join(str(item) for item in command)
    if command is None:
        return ""
    return str(command)


def _first_string(value: Any) -> str | None:
    if isinstance(value, list):
        for item in value:
            if isinstance(item, str):
                return _safe_scalar(item)
    if isinstance(value, str):
        return _safe_scalar(value)
    return None


def _path_refs(values: Any, kind: str) -> list[dict[str, str]]:
    refs = []
    for value in _list(values):
        if isinstance(value, dict):
            raw = value.get("value") or value.get("path") or value.get("owner")
            ref_kind = value.get("kind") if isinstance(value.get("kind"), str) else kind
        else:
            raw = value
            ref_kind = kind
        if isinstance(raw, str) and not is_absolute_path(raw):
            refs.append({"kind": ref_kind, "value": raw})
    return refs


def _symbol_refs(values: Any) -> list[dict[str, Any]]:
    refs = []
    for value in _list(values):
        if isinstance(value, str):
            refs.append({"name": _safe_scalar(value)})
        elif isinstance(value, dict):
            refs.extend(_symbol_ref_from_dict(value))
    return refs


def _symbol_ref_from_dict(value: dict[str, Any]) -> list[dict[str, Any]]:
    item = {
        "name": _safe_optional(value.get("name")),
        "kind": _safe_optional(value.get("kind")),
    }
    owner = value.get("owner")
    if isinstance(owner, str) and not is_absolute_path(owner):
        item["owner"] = {"kind": "owner", "value": owner}
    item = _drop_none(item)
    return [item] if "name" in item else []


def _display_ranges(values: Any) -> list[dict[str, Any]]:
    ranges = []
    for value in _list(values):
        if not isinstance(value, dict):
            continue
        owner = value.get("owner") or value.get("path")
        if not isinstance(owner, str) or is_absolute_path(owner):
            continue
        item = {
            "owner": {"kind": "owner", "value": owner},
            "displayLineStart": _int_or_none(value.get("displayLineStart") or value.get("lineStart")),
            "displayLineEnd": _int_or_none(value.get("displayLineEnd") or value.get("lineEnd")),
            "selectorHint": _safe_optional(value.get("selectorHint")),
        }
        ranges.append(_drop_none(item))
    return ranges


def _graph_edges(values: Any) -> list[dict[str, Any]]:
    edges = []
    for value in _list(values):
        if not isinstance(value, dict):
            continue
        edge = {
            "from": _safe_optional(value.get("from")),
            "to": _safe_optional(value.get("to")),
            "relation": _safe_optional(value.get("relation")),
            "weight": _number_or_none(value.get("weight")),
        }
        edge = _drop_none(edge)
        if {"from", "to", "relation"}.issubset(edge):
            edges.append(edge)
    return edges


def _ranked_evidence(values: Any) -> list[dict[str, Any]]:
    items = []
    for value in _list(values):
        if not isinstance(value, dict):
            continue
        item = {
            "id": _safe_optional(value.get("id")),
            "score": _number_or_none(value.get("score")),
            "reason": _safe_optional(value.get("reason")),
        }
        item = _drop_none(item)
        if {"id", "score"}.issubset(item):
            items.append(item)
    return items
