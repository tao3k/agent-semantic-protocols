# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Schema gates for content-addressed provider live-corpus artifacts."""

from __future__ import annotations

import json
from pathlib import Path

from jsonschema import Draft202012Validator


_ROOT = Path(__file__).resolve().parents[2]
_DIGEST = "a" * 64
_REVISION = "b" * 40
_TREE = "c" * 40


def validator(name: str) -> Draft202012Validator:
    schema = json.loads((_ROOT / "schemas" / name).read_text(encoding="utf-8"))
    Draft202012Validator.check_schema(schema)
    return Draft202012Validator(schema)


def test_live_corpus_artifact_identity_is_path_independent() -> None:
    manifest = {
        "schemaId": "agent.semantic-protocols.live-corpus-artifact",
        "schemaVersion": "1",
        "artifactDigest": _DIGEST,
        "lockDigest": "d" * 64,
        "resourceId": "rust.tokio",
        "providerId": "asp-rust",
        "languageId": "rust",
        "builderId": "asp-live-corpus",
        "git": {
            "remote": "git@github.com:tokio-rs/tokio.git",
            "canonicalRemoteIdentity": "github.com/tokio-rs/tokio",
            "remoteDigest": "e" * 64,
            "revision": _REVISION,
            "tree": _TREE,
        },
        "sourceMerkleRoot": "f" * 64,
    }

    validator("asp.live-corpus-artifact.v1.schema.json").validate(manifest)
    assert "sourcePath" not in manifest


def test_live_corpus_qualification_owns_materialized_source_path() -> None:
    receipt = {
        "schemaId": "agent.semantic-protocols.live-corpus-artifact-qualification",
        "schemaVersion": "1",
        "artifactDigest": _DIGEST,
        "materializationAuthority": "nix-store",
        "sourcePath": "/nix/store/example-tokio-source",
        "headRevision": _REVISION,
        "gitTree": _TREE,
        "sourceMerkleRoot": "f" * 64,
        "languageExtensionEvidence": {
            "authority": "provider-project-resolution",
            "candidateSetAuthority": "provider-registry-extension-index",
            "sourceExtensions": [".rs"],
            "matchingFileCount": 1200,
            "candidateLanguageFileCount": 1400,
        },
        "clean": True,
        "status": "qualified",
    }

    validator("asp.live-corpus-artifact-qualification.v1.schema.json").validate(receipt)


def test_live_corpus_qualification_rejects_unqualified_or_dirty_source() -> None:
    receipt = {
        "schemaId": "agent.semantic-protocols.live-corpus-artifact-qualification",
        "schemaVersion": "1",
        "artifactDigest": _DIGEST,
        "materializationAuthority": "developer-gix",
        "sourcePath": "/state/git/repo/blake3-256/repository/checkouts/revision",
        "headRevision": _REVISION,
        "gitTree": _TREE,
        "sourceMerkleRoot": "f" * 64,
        "languageExtensionEvidence": {
            "authority": "provider-project-resolution",
            "candidateSetAuthority": "provider-registry-extension-index",
            "sourceExtensions": [".rs"],
            "matchingFileCount": 1200,
            "candidateLanguageFileCount": 1400,
        },
        "clean": False,
        "status": "qualified",
    }

    errors = list(
        validator("asp.live-corpus-artifact-qualification.v1.schema.json").iter_errors(
            receipt
        )
    )
    assert errors


def test_document_corpus_lock_requires_language_extension_admission() -> None:
    manifest = {
        "schemaId": "agent.semantic-protocols.semantic-sandtable-large-library-corpora",
        "schemaVersion": "1",
        "corpora": [
            {
                "resourceId": "md.invalid",
                "scenarioId": "md.invalid",
                "providerId": "orgize",
                "language": "md",
                "repository": "example/javascript-processor",
                "git": {
                    "remote": "https://github.com/example/javascript-processor.git",
                    "revision": _REVISION,
                },
                "directory": "md-invalid",
                "environment": "MD_INVALID_ROOT",
                "inputs": {
                    "owner": "README.md",
                    "query": "markdown",
                    "dependency": "processor",
                },
            }
        ],
    }

    errors = list(
        validator(
            "semantic-sandtable-large-library-corpora.v1.schema.json"
        ).iter_errors(manifest)
    )

    assert errors


def test_live_corpus_materialize_receipt_binds_extension_counts() -> None:
    receipt = {
        "schemaId": "agent.semantic-protocols.live-corpus-materialize-receipt",
        "schemaVersion": "1",
        "resourceId": "org.worg",
        "providerId": "orgize",
        "languageId": "org",
        "artifactDigest": _DIGEST,
        "artifactPath": "/state/artifacts/live-corpus/v1/blake3-256/digest",
        "currentPointer": "/state/artifacts/live-corpus/v1/by-resource/org.worg/current",
        "matchingFileCount": 293,
        "candidateLanguageFileCount": 297,
        "status": "qualified",
    }

    validator("asp.live-corpus-materialize-receipt.v1.schema.json").validate(receipt)


def test_live_corpus_path_receipt_exposes_gix_derived_checkout() -> None:
    receipt = {
        "schemaId": "agent.semantic-protocols.live-corpus-path-receipt",
        "schemaVersion": "1",
        "resourceId": "md.mdn-content",
        "remote": "https://github.com/mdn/content.git",
        "revision": _REVISION,
        "repositoryPath": "/state/git/repo/blake3-256/digest",
        "checkoutPath": f"/state/git/repo/blake3-256/digest/checkouts/{_REVISION}",
    }

    validator("asp.live-corpus-path-receipt.v1.schema.json").validate(receipt)


def test_live_corpus_sync_receipt_binds_pinned_checkout() -> None:
    receipt = {
        "schemaId": "agent.semantic-protocols.live-corpus-sync-receipt",
        "schemaVersion": "1",
        "resourceId": "org.worg",
        "remote": "https://git.sr.ht/~bzg/worg",
        "revision": _REVISION,
        "checkoutPath": f"/state/git/repo/blake3-256/digest/checkouts/{_REVISION}",
        "status": "materialized",
    }

    validator("asp.live-corpus-sync-receipt.v1.schema.json").validate(receipt)
