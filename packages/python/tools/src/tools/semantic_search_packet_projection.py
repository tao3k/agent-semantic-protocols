"""Project semantic-search-packet schema facts into Lean proof inputs."""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any


@dataclass(frozen=True)
class SchemaProjection:
    source_schema: str
    facts: list[dict[str, str]]
    formal_lean: str
    candidate_lean: str


def build_semantic_search_packet_projection(schema_path: Path) -> SchemaProjection:
    schema = json.loads(schema_path.read_text(encoding="utf-8"))
    defs = schema["$defs"]
    facts = [
        _ref_fact(defs, "item", "structuralSelector", "executable-selector", "structuralSelector"),
        _ref_fact(defs, "item", "displayLineRange", "display-only", "lineRange"),
        _ref_fact(defs, "item", "sourceLocatorHint", "display-only", "sourceLocator"),
        _field_fact(defs, "routeAction", "selector", "executable-selector"),
        _description_fact(defs, "routeAction", "displayLineRange", "display-only", "display-only"),
        _description_fact(defs, "routeAction", "sourceLocatorHint", "display-only", "display-only"),
        _ref_fact(
            defs,
            "windowSetTarget",
            "structuralSelector",
            "executable-selector",
            "structuralSelector",
        ),
        _ref_fact(defs, "windowSetTarget", "displayLineRange", "display-only", "lineRange"),
        _ref_fact(defs, "windowSetTarget", "sourceLocatorHint", "display-only", "sourceLocator"),
    ]
    return SchemaProjection(
        source_schema=str(schema_path),
        facts=facts,
        formal_lean=_render_projection_lean(facts, proof_body="sorry"),
        candidate_lean=_render_projection_lean(facts, proof_body="by\n  simp [" + _lean_def_list(facts) + "]"),
    )


def _properties(defs: dict[str, Any], owner: str) -> dict[str, Any]:
    return defs[owner]["properties"]


def _ref_fact(
    defs: dict[str, Any],
    owner: str,
    field: str,
    role: str,
    ref_suffix: str,
) -> dict[str, str]:
    value = _properties(defs, owner)[field]
    ref = value.get("$ref", "")
    if not ref.endswith(ref_suffix):
        raise ValueError(f"{owner}.{field} expected $ref ending {ref_suffix!r}, got {ref!r}")
    return {"id": f"{owner}.{field}", "owner": owner, "field": field, "role": role}


def _field_fact(defs: dict[str, Any], owner: str, field: str, role: str) -> dict[str, str]:
    if field not in _properties(defs, owner):
        raise ValueError(f"{owner}.{field} is missing")
    return {"id": f"{owner}.{field}", "owner": owner, "field": field, "role": role}


def _description_fact(
    defs: dict[str, Any],
    owner: str,
    field: str,
    role: str,
    expected: str,
) -> dict[str, str]:
    value = _properties(defs, owner)[field]
    description = value.get("description", "").lower()
    if expected.lower() not in description:
        raise ValueError(f"{owner}.{field} description does not contain {expected!r}")
    return {"id": f"{owner}.{field}", "owner": owner, "field": field, "role": role}


def _lean_name(fact: dict[str, str]) -> str:
    owner, field = fact["owner"], fact["field"]
    return f"schema_{owner}_{field}".replace("-", "_")


def _lean_def_list(facts: list[dict[str, str]]) -> str:
    return ", ".join(_lean_name(fact) for fact in facts)


def _render_projection_lean(facts: list[dict[str, str]], proof_body: str) -> str:
    definitions = "\n".join(f"def {_lean_name(fact)} : Bool := true" for fact in facts)
    theorem_terms = " /\\\n    ".join(f"{_lean_name(fact)} = true" for fact in facts)
    return f"""\
{definitions}

theorem semantic_search_packet_schema_projects_selector_identity :
    {theorem_terms} := {proof_body}
"""
