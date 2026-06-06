"""Schema-owned compact graph reasoning profile contracts."""

from __future__ import annotations

from dataclasses import dataclass
from functools import lru_cache
import json
from pathlib import Path
from typing import Any


@dataclass(frozen=True)
class CompactGraphEntryProfileSelector:
    kind: str
    required: bool


@dataclass(frozen=True)
class CompactGraphEntryProfileContract:
    profile: str
    selectors: tuple[CompactGraphEntryProfileSelector, ...]
    returns: tuple[str, ...]

    @property
    def required_selector_count(self) -> int:
        return sum(1 for selector in self.selectors if selector.required)


def compact_graph_entry_profile_contracts() -> dict[str, CompactGraphEntryProfileContract]:
    return dict(_load_compact_graph_entry_profile_contracts())


def compact_graph_entry_selector_summary(
    contract: CompactGraphEntryProfileContract,
) -> str:
    return ",".join(
        f"{selector.kind}{'?' if not selector.required else ''}"
        for selector in contract.selectors
    )


@lru_cache(maxsize=1)
def _load_compact_graph_entry_profile_contracts() -> dict[str, CompactGraphEntryProfileContract]:
    schema = _load_compact_graph_render_schema()
    contracts = schema["properties"]["reasoningProfileContracts"]["const"]
    if not isinstance(contracts, list):
        raise TypeError("reasoningProfileContracts const must be an array")
    return {
        contract.profile: contract
        for contract in (_parse_contract(value) for value in contracts)
    }


def _parse_contract(value: Any) -> CompactGraphEntryProfileContract:
    if not isinstance(value, dict):
        raise TypeError("reasoning profile contract must be an object")
    profile = value.get("profile")
    selectors = value.get("selectors")
    returns = value.get("returns")
    if not isinstance(profile, str):
        raise TypeError("reasoning profile contract profile must be a string")
    if not isinstance(selectors, list):
        raise TypeError("reasoning profile contract selectors must be an array")
    if not isinstance(returns, list) or not all(isinstance(item, str) for item in returns):
        raise TypeError("reasoning profile contract returns must be a string array")
    return CompactGraphEntryProfileContract(
        profile=profile,
        selectors=tuple(_parse_selector(selector) for selector in selectors),
        returns=tuple(returns),
    )


def _parse_selector(value: Any) -> CompactGraphEntryProfileSelector:
    if not isinstance(value, dict):
        raise TypeError("reasoning profile selector contract must be an object")
    kind = value.get("kind")
    required = value.get("required")
    if not isinstance(kind, str):
        raise TypeError("reasoning profile selector kind must be a string")
    if not isinstance(required, bool):
        raise TypeError("reasoning profile selector required must be a boolean")
    return CompactGraphEntryProfileSelector(kind=kind, required=required)


def _load_compact_graph_render_schema() -> dict[str, Any]:
    with _compact_graph_render_schema_path().open("r", encoding="utf-8") as handle:
        schema = json.load(handle)
    if not isinstance(schema, dict):
        raise TypeError("compact graph render schema must be an object")
    return schema


def _compact_graph_render_schema_path() -> Path:
    for parent in Path(__file__).resolve().parents:
        schema_path = parent / "schemas" / "semantic-compact-graph-render.v1.schema.json"
        if schema_path.exists():
            return schema_path
    raise FileNotFoundError("schemas/semantic-compact-graph-render.v1.schema.json")


COMPACT_GRAPH_ENTRY_PROFILE_CONTRACTS = compact_graph_entry_profile_contracts()
COMPACT_GRAPH_ENTRY_PROFILE_NODE_KINDS: dict[str, set[str]] = {
    profile: {selector.kind for selector in contract.selectors}
    for profile, contract in COMPACT_GRAPH_ENTRY_PROFILE_CONTRACTS.items()
}
