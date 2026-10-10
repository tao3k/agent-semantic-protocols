# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Resolve qualified provider live-corpus artifacts from ASP State Home."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from .large_library_runtime_types import Corpus
from .utils import dict_value


def resolve_corpora(
    corpora: list[Corpus], state_home: Path
) -> tuple[list[dict[str, str]], list[dict[str, str]]]:
    resolved: list[dict[str, str]] = []
    missing: list[dict[str, str]] = []
    for corpus in corpora:
        artifact, artifact_error = live_corpus_artifact(corpus, state_home)
        path = Path(
            str(
                artifact.get("sourcePath", live_corpus_current_path(corpus, state_home))
            )
        )
        record = locked_corpus_record(corpus, path)
        if artifact_error is not None:
            missing.append({**record, "reason": artifact_error})
            continue
        owner = path / corpus.inputs["owner"]
        if not path.is_dir():
            missing.append({**record, "reason": "artifact-source-missing"})
        elif not owner.is_file():
            missing.append({**record, "reason": "owner-missing"})
        else:
            resolved.append(
                {
                    **record,
                    "revision": corpus.revision,
                    "artifactDigest": str(artifact["artifactDigest"]),
                    "sourceMerkleRoot": str(artifact["sourceMerkleRoot"]),
                }
            )
    return resolved, missing


def locked_corpus_record(corpus: Corpus, path: Path) -> dict[str, str]:
    return {
        "resourceId": corpus.resource_id,
        "scenarioId": corpus.scenario_id,
        "providerId": corpus.provider_id,
        "language": corpus.language,
        "repository": corpus.repository,
        "remote": corpus.remote,
        "path": str(path),
        "expectedRevision": corpus.revision,
    }


def live_corpus_artifact(
    corpus: Corpus, state_home: Path
) -> tuple[dict[str, Any], str | None]:
    current = live_corpus_current_path(corpus, state_home)
    manifest_path = current / "manifest.json"
    qualification_path = current / "qualification.json"
    if not manifest_path.is_file() or not qualification_path.is_file():
        return {}, "artifact-current-missing"
    try:
        manifest = dict_value(json.loads(manifest_path.read_text(encoding="utf-8")))
        qualification = dict_value(
            json.loads(qualification_path.read_text(encoding="utf-8"))
        )
    except (OSError, json.JSONDecodeError):
        return {}, "artifact-receipt-invalid"
    if not live_corpus_artifact_matches_lock(corpus, manifest, qualification):
        return {}, "artifact-identity-mismatch"
    source_path = qualification.get("sourcePath")
    if not isinstance(source_path, str) or not source_path:
        return {}, "artifact-source-path-missing"
    return {
        "artifactDigest": manifest["artifactDigest"],
        "sourceMerkleRoot": manifest["sourceMerkleRoot"],
        "sourcePath": str(Path(source_path).expanduser().resolve()),
    }, None


def live_corpus_current_path(corpus: Corpus, state_home: Path) -> Path:
    return (
        state_home
        / "artifacts"
        / "live-corpus"
        / "v1"
        / "by-resource"
        / corpus.resource_id
        / "current"
    )


def live_corpus_artifact_matches_lock(
    corpus: Corpus, manifest: dict[str, Any], qualification: dict[str, Any]
) -> bool:
    git = dict_value(manifest.get("git"))
    artifact_digest = manifest.get("artifactDigest")
    source_merkle_root = manifest.get("sourceMerkleRoot")
    return (
        manifest.get("schemaId") == "agent.semantic-protocols.live-corpus-artifact"
        and manifest.get("schemaVersion") == "1"
        and manifest.get("resourceId") == corpus.resource_id
        and manifest.get("providerId") == corpus.provider_id
        and manifest.get("languageId") == corpus.language
        and git.get("remote") == corpus.remote
        and git.get("revision") == corpus.revision
        and isinstance(artifact_digest, str)
        and len(artifact_digest) == 64
        and isinstance(source_merkle_root, str)
        and len(source_merkle_root) == 64
        and qualification.get("schemaId")
        == "agent.semantic-protocols.live-corpus-artifact-qualification"
        and qualification.get("schemaVersion") == "1"
        and qualification.get("artifactDigest") == artifact_digest
        and qualification.get("headRevision") == corpus.revision
        and qualification.get("sourceMerkleRoot") == source_merkle_root
        and language_extension_evidence_matches_lock(corpus, qualification)
        and qualification.get("clean") is True
        and qualification.get("status") == "qualified"
    )


def language_extension_evidence_matches_lock(
    corpus: Corpus, qualification: dict[str, Any]
) -> bool:
    evidence = dict_value(qualification.get("languageExtensionEvidence"))
    extensions = evidence.get("sourceExtensions")
    matching_files = evidence.get("matchingFileCount")
    candidate_files = evidence.get("candidateLanguageFileCount")
    if (
        evidence.get("authority") != "provider-project-resolution"
        or evidence.get("candidateSetAuthority") != "provider-registry-extension-index"
        or not isinstance(extensions, list)
        or not extensions
        or not all(
            isinstance(extension, str) and extension.startswith(".")
            for extension in extensions
        )
        or not any(
            corpus.inputs["owner"].lower().endswith(extension.lower())
            for extension in extensions
        )
        or not isinstance(matching_files, int)
        or isinstance(matching_files, bool)
        or not isinstance(candidate_files, int)
        or isinstance(candidate_files, bool)
        or matching_files < 1
        or candidate_files < matching_files
    ):
        return False
    admission = corpus.admission
    return admission is None or (
        evidence.get("authority") == admission.authority
        and matching_files >= admission.minimum_matching_files
        and matching_files / candidate_files >= admission.minimum_matching_file_ratio
    )
