"""Validate the shared semantic AST patch request and receipt schemas."""

import json
from pathlib import Path

from jsonschema import Draft202012Validator


_ROOT = Path(__file__).resolve().parents[2]


def _load_schema(name: str) -> dict:
    return json.loads((_ROOT / "schemas" / name).read_text(encoding="utf-8"))


def minimal_ast_patch_request() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-ast-patch",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.ast-patch",
        "protocolVersion": "1",
        "languageId": "typescript",
        "providerId": "ts-harness",
        "binary": "ts-harness",
        "namespace": "agent.semantic-protocols.languages.typescript.ts-harness",
        "target": {
            "ownerPath": "src/render.ts",
            "locator": "src/render.ts#fn:renderSemanticSearchSeedPacket",
            "read": "src/render.ts:10:43",
            "location": {"path": "src/render.ts", "lineRange": "10:43"},
        },
        "operation": {"op": "append_to_block", "snippet": "lines.push(value);"},
    }


def minimal_ast_patch_receipt() -> dict[str, object]:
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
        "next": "Codex adapter: apply_patch remains the mutation path.",
    }


def _errors(schema_name: str, packet: dict[str, object]) -> list[str]:
    validator = Draft202012Validator(_load_schema(schema_name))
    return [error.message for error in validator.iter_errors(packet)]


def test_ast_patch_request_schema_accepts_compact_locator_packet() -> None:
    assert (
        _errors("semantic-ast-patch.v1.schema.json", minimal_ast_patch_request()) == []
    )


def test_ast_patch_request_schema_rejects_start_line_end_line_fields() -> None:
    packet = minimal_ast_patch_request()
    target = packet["target"]
    assert isinstance(target, dict)
    target["startLine"] = 10
    target["endLine"] = 43

    messages = _errors("semantic-ast-patch.v1.schema.json", packet)

    assert any(
        "Additional properties are not allowed" in message for message in messages
    )


def test_ast_patch_request_schema_requires_source_locator_read() -> None:
    packet = minimal_ast_patch_request()
    target = packet["target"]
    assert isinstance(target, dict)
    target["read"] = "src/render.ts:10"

    messages = _errors("semantic-ast-patch.v1.schema.json", packet)

    assert any("does not match" in message for message in messages)


def test_ast_patch_request_schema_accepts_bounded_mechanical_delete() -> None:
    packet = minimal_ast_patch_request()
    packet["operation"] = {
        "op": "remove_statement",
        "expectedSnippet": "console.log(value);",
        "mechanicalKind": "bounded-multi-node",
        "maxEdits": 20,
        "allowLargeMechanicalEdit": True,
    }

    assert _errors("semantic-ast-patch.v1.schema.json", packet) == []


def test_ast_patch_request_schema_rejects_unknown_operation() -> None:
    packet = minimal_ast_patch_request()
    packet["operation"] = {"op": "regex_replace_everything", "snippet": "changed"}

    messages = _errors("semantic-ast-patch.v1.schema.json", packet)

    assert any("is not one of" in message for message in messages)


def test_ast_patch_receipt_schema_accepts_codex_verifier_receipt() -> None:
    assert (
        _errors(
            "semantic-ast-patch-receipt.v1.schema.json",
            minimal_ast_patch_receipt(),
        )
        == []
    )


def test_ast_patch_receipt_schema_rejects_mutation_available() -> None:
    receipt = minimal_ast_patch_receipt()
    receipt["mutationAvailable"] = True

    messages = _errors("semantic-ast-patch-receipt.v1.schema.json", receipt)

    assert any("False was expected" in message for message in messages)
