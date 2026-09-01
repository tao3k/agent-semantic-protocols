"""Shared fixtures for semantic AST patch schema tests."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator
from referencing import Registry, Resource


_ROOT = Path(__file__).resolve().parents[3]


def _load_schema(name: str) -> dict[str, Any]:
    return json.loads((_ROOT / "schemas" / name).read_text(encoding="utf-8"))


def _local_schema_registry() -> Registry[Any]:
    registry: Registry[Any] = Registry()
    for path in sorted((_ROOT / "schemas").glob("*.schema.json")):
        schema = json.loads(path.read_text(encoding="utf-8"))
        schema_id = schema.get("$id")
        if isinstance(schema_id, str):
            registry = registry.with_resource(schema_id, Resource.from_contents(schema))
    return registry


def minimal_ast_patch_request() -> dict[str, Any]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-ast-patch",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.ast-patch",
        "protocolVersion": "1",
        "languageId": "typescript",
        "providerId": "asp-typescript",
        "binary": "asp-typescript",
        "namespace": "agent.semantic-protocols.languages.typescript.asp-typescript",
        "target": {
            "ownerPath": "src/render.ts",
            "locator": "src/render.ts#fn:renderSemanticSearchSeedPacket",
            "read": "src/render.ts:10:43",
            "location": {"path": "src/render.ts", "lineRange": "10:43"},
        },
        "operation": {"op": "append_to_block", "snippet": "lines.push(value);"},
    }


def minimal_ast_patch_receipt() -> dict[str, Any]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-ast-patch-receipt",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.ast-patch",
        "protocolVersion": "1",
        "status": "verified",
        "mode": "verify",
        "capability": "codex-compact-safety-verifier",
        "mutationAvailable": False,
        "languageId": "typescript",
        "target": {
            "ownerPath": "src/render.ts",
            "locator": "src/render.ts#fn:renderSemanticSearchSeedPacket",
            "read": "src/render.ts:10:43",
        },
        "operation": "append_to_block",
        "verification": ["packet-parsed", "codex-mutation-disabled"],
        "failureKind": None,
        "failures": [],
        "supportedOperations": [
            "append_to_block",
            "insert_before_statement",
            "insert_after_statement",
            "replace_statement",
            "replace_expression",
            "replace_call_arg",
            "insert_import",
            "remove_import",
            "remove_statement",
            "remove_item",
            "replace_item",
            "split_owner_items",
        ],
        "mechanicalEditPlan": {
            "kind": "codex-dry-run",
            "operation": "append_to_block",
            "targetRead": "src/render.ts:10:43",
            "estimatedEdits": 1,
            "maxEdits": 1,
            "safeForLargeChange": False,
            "mutationAvailable": False,
            "requiresCodexApplyPatch": True,
            "changedRanges": ["src/render.ts:10:43"],
            "notes": ["single-target AST patch intent verified"],
        },
        "next": (
            "provider-dry-run: asp typescript ast-patch dry-run --packet "
            "semantic-ast-patch.json .; exact-read: asp typescript query "
            "--from-hook direct-source-read --selector src/render.ts:10:43 "
            "--code .; fallback: Codex apply_patch only when "
            "mutationSource=codex-text-fallback or receipt.requiresCodexApplyPatch=true; "
            "consume the dependency-owned TypeScript policy receipt"
        ),
    }


def schema_errors(schema_name: str, packet: dict[str, Any]) -> list[str]:
    validator = Draft202012Validator(
        _load_schema(schema_name),
        registry=_local_schema_registry(),
    )
    return [error.message for error in validator.iter_errors(packet)]
