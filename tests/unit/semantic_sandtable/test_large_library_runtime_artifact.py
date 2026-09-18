# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""State Home artifact resolution tests for provider live corpora."""

from __future__ import annotations

import json
from pathlib import Path

from tools.semantic_sandtable.large_library_runtime_artifact import resolve_corpora
from tools.semantic_sandtable.large_library_runtime_types import (
    Corpus,
    LanguageExtensionAdmission,
)


def corpus() -> Corpus:
    return Corpus(
        resource_id="rust.live",
        scenario_id="rust.live",
        provider_id="asp-rust",
        language="rust",
        repository="example/live",
        remote="https://github.com/example/live.git",
        revision="1" * 40,
        directory="ignored-directory",
        environment="IGNORED_CORPUS_ROOT",
        inputs={"owner": "src/lib.rs", "query": "live", "dependency": "serde"},
    )


def publish_receipts(
    state_home: Path,
    source: Path,
    *,
    revision: str,
) -> None:
    current = state_home / "artifacts/live-corpus/v1/by-resource/rust.live/current"
    current.mkdir(parents=True)
    manifest = {
        "schemaId": "agent.semantic-protocols.live-corpus-artifact",
        "schemaVersion": "1",
        "artifactDigest": "2" * 64,
        "resourceId": "rust.live",
        "providerId": "asp-rust",
        "languageId": "rust",
        "git": {
            "remote": "https://github.com/example/live.git",
            "revision": revision,
        },
        "sourceMerkleRoot": "3" * 64,
    }
    qualification = {
        "schemaId": "agent.semantic-protocols.live-corpus-artifact-qualification",
        "schemaVersion": "1",
        "artifactDigest": "2" * 64,
        "sourcePath": str(source),
        "headRevision": revision,
        "sourceMerkleRoot": "3" * 64,
        "languageExtensionEvidence": {
            "authority": "provider-project-resolution",
            "candidateSetAuthority": "provider-registry-extension-index",
            "sourceExtensions": [".rs"],
            "matchingFileCount": 400,
            "candidateLanguageFileCount": 500,
        },
        "clean": True,
        "status": "qualified",
    }
    (current / "manifest.json").write_text(json.dumps(manifest), encoding="utf-8")
    (current / "qualification.json").write_text(
        json.dumps(qualification), encoding="utf-8"
    )


def test_resolves_only_qualified_state_home_artifact(tmp_path: Path) -> None:
    state_home = tmp_path / "state"
    source = tmp_path / "immutable-source"
    (source / "src").mkdir(parents=True)
    (source / "src/lib.rs").write_text("pub fn live() {}\n", encoding="utf-8")
    publish_receipts(state_home, source, revision="1" * 40)

    resolved, missing = resolve_corpora([corpus()], state_home)

    assert missing == []
    assert resolved[0]["path"] == str(source)
    assert resolved[0]["artifactDigest"] == "2" * 64
    assert resolved[0]["sourceMerkleRoot"] == "3" * 64


def test_rejects_artifact_revision_drift(tmp_path: Path) -> None:
    state_home = tmp_path / "state"
    source = tmp_path / "immutable-source"
    source.mkdir()
    publish_receipts(state_home, source, revision="9" * 40)

    resolved, missing = resolve_corpora([corpus()], state_home)

    assert resolved == []
    assert missing[0]["reason"] == "artifact-identity-mismatch"


def test_rejects_artifact_when_owner_is_not_in_provider_extension_set(
    tmp_path: Path,
) -> None:
    state_home = tmp_path / "state"
    source = tmp_path / "immutable-source"
    (source / "src").mkdir(parents=True)
    (source / "src/lib.rs").write_text("pub fn live() {}\n", encoding="utf-8")
    publish_receipts(state_home, source, revision="1" * 40)
    qualification_path = (
        state_home
        / "artifacts/live-corpus/v1/by-resource/rust.live/current/qualification.json"
    )
    qualification = json.loads(qualification_path.read_text(encoding="utf-8"))
    qualification["languageExtensionEvidence"]["sourceExtensions"] = [".md"]
    qualification_path.write_text(json.dumps(qualification), encoding="utf-8")

    resolved, missing = resolve_corpora([corpus()], state_home)

    assert resolved == []
    assert missing[0]["reason"] == "artifact-identity-mismatch"


def test_rejects_document_corpus_below_locked_extension_ratio(tmp_path: Path) -> None:
    state_home = tmp_path / "state"
    source = tmp_path / "immutable-source"
    (source / "src").mkdir(parents=True)
    (source / "src/lib.rs").write_text("pub fn live() {}\n", encoding="utf-8")
    publish_receipts(state_home, source, revision="1" * 40)
    qualification_path = (
        state_home
        / "artifacts/live-corpus/v1/by-resource/rust.live/current/qualification.json"
    )
    qualification = json.loads(qualification_path.read_text(encoding="utf-8"))
    qualification["languageExtensionEvidence"]["matchingFileCount"] = 100
    qualification["languageExtensionEvidence"]["candidateLanguageFileCount"] = 500
    qualification_path.write_text(json.dumps(qualification), encoding="utf-8")
    locked = corpus()
    locked = Corpus(
        resource_id=locked.resource_id,
        scenario_id=locked.scenario_id,
        provider_id=locked.provider_id,
        language=locked.language,
        repository=locked.repository,
        remote=locked.remote,
        revision=locked.revision,
        directory=locked.directory,
        environment=locked.environment,
        inputs=locked.inputs,
        admission=LanguageExtensionAdmission(
            authority="provider-project-resolution",
            minimum_matching_files=100,
            minimum_matching_file_ratio=0.8,
        ),
    )

    resolved, missing = resolve_corpora([locked], state_home)

    assert resolved == []
    assert missing[0]["reason"] == "artifact-identity-mismatch"
