"""Validate the shared content compaction schema."""

from __future__ import annotations

from pathlib import Path

from unit.schema_validation import schema_validator_for


_REPO_ROOT = Path(__file__).resolve().parents[2]


def _valid_packet() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-content-compaction",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "packetKind": "content-compaction-result",
        "source": {
            "path": "docs/effect-patterns.md",
            "fingerprint": "sha256:docs-effect-patterns",
        },
        "contentBlocks": [
            {
                "id": "block:doc:interruption-scope",
                "contentKind": "documentation-metadata",
                "mediaType": "text/markdown",
                "criticality": "metadata",
                "locator": {
                    "path": "docs/effect-patterns.md",
                    "lineRange": "40:90",
                },
                "sourceFingerprint": "sha256:docs-effect-patterns-40-90",
                "compaction": {
                    "mode": "markdown-heading-link-digest",
                    "lossiness": "aggressive",
                    "trustLevel": "metadata-backed",
                    "sourceOfTruth": "document-parser-facts",
                    "validFor": ["discovery", "routing", "evidence-selection"],
                    "notValidFor": ["quoting", "normative-proof"],
                    "preserved": ["headings", "anchors", "links", "keywords"],
                    "omitted": ["paragraph-body", "example-body"],
                },
                "metadata": {
                    "heading": "Interruption and Scope",
                    "keywords": ["effect", "interrupt", "scope"],
                },
            },
            {
                "id": "block:code:effect-interrupt",
                "contentKind": "source-code",
                "languageId": "typescript",
                "criticality": "exact-source-required-for-edit",
                "locator": {
                    "path": "packages/runtime/src/fiber.ts",
                    "lineRange": "120:155",
                },
                "sourceFingerprint": "sha256:fiber-120-155",
                "compaction": {
                    "mode": "source-code-call-skeleton",
                    "lossiness": "bounded",
                    "trustLevel": "parser-backed",
                    "sourceOfTruth": "parser-facts",
                    "validFor": ["navigation", "reasoning"],
                    "notValidFor": ["patch", "line-edit", "exact-source"],
                    "preserved": ["signature", "control-flow-shape", "called-symbols"],
                    "omitted": ["large-literals", "private-branch-body"],
                    "requiresExactSourceFor": ["patch", "compile-fix"],
                    "exactSourceRequired": True,
                },
                "content": "export function interrupt(...) { ... }",
            },
        ],
    }


def test_content_compaction_packet_is_valid() -> None:
    validator = schema_validator_for(
        _REPO_ROOT / "schemas" / "semantic-content-compaction.v1.schema.json"
    )

    assert list(validator.iter_errors(_valid_packet())) == []


def test_content_compaction_requires_lossiness() -> None:
    validator = schema_validator_for(
        _REPO_ROOT / "schemas" / "semantic-content-compaction.v1.schema.json"
    )
    packet = _valid_packet()
    del packet["contentBlocks"][0]["compaction"]["lossiness"]

    assert list(validator.iter_errors(packet)) != []
