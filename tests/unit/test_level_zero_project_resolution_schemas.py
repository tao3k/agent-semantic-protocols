from __future__ import annotations

import json
from copy import deepcopy
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator, ValidationError
from referencing import Registry, Resource


ROOT = Path(__file__).resolve().parents[2]
SCHEMA_ROOT = ROOT / "schemas"
SCHEMA_NAMES = (
    "language-package-graph.v1.schema.json",
    "repository-candidate-snapshot.v1.schema.json",
    "resolved-source-scope.v1.schema.json",
    "project-resolution.v1.schema.json",
)
SCHEMAS = {
    name: json.loads((SCHEMA_ROOT / name).read_text())
    for name in SCHEMA_NAMES
}
REGISTRY = Registry().with_resources(
    (schema["$id"], Resource.from_contents(schema))
    for schema in SCHEMAS.values()
)


def validator(name: str) -> Draft202012Validator:
    return Draft202012Validator(SCHEMAS[name], registry=REGISTRY)


def git_candidates() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.repository-candidate-snapshot",
        "schemaVersion": "1",
        "mode": "git",
        "repositoryIdentity": {
            "repositoryId": "repo-1",
            "identityBasis": "git-common-dir:/workspace/.git",
            "gitCommonDir": "/workspace/.git",
        },
        "worktreeIdentity": {
            "worktreeId": "workspace-1",
            "worktreeRoot": "/workspace",
            "gitDir": "/workspace/.git",
            "headId": "abc",
        },
        "candidateGeneration": {
            "algorithm": "blake3-path-set-v1",
            "digest": "blake3:" + ("0" * 64),
            "authorities": ["git-index"],
        },
        "candidates": [
            {
                "path": "Cargo.toml",
                "state": "tracked",
                "authority": "git-index",
            }
        ],
        "policyOverlayDigest": "blake3:" + ("1" * 64),
        "policyExclusions": [],
        "metrics": {
            "indexEntryCount": 1,
            "worktreeAdditionCount": 0,
            "candidateCount": 1,
            "policyExclusionCount": 0,
            "fullWorkspaceReads": 0,
            "fullMerkleRebuilds": 0,
            "directDbOpens": 0,
        },
    }


def resolved_scope() -> dict[str, object]:
    return {
        "scopeId": "scope-runtime-lib",
        "packageId": "runtime",
        "targetId": "runtime:lib",
        "roots": ["crates/runtime/src"],
        "explicitPaths": ["crates/runtime/src/lib.rs"],
        "extensions": [".rs"],
        "includeAuthority": "manifest-explicit",
        "classifications": ["production"],
        "exclusions": [],
        "providerFacts": {
            "cargoTargetKind": "lib",
        },
        "conflicts": [],
        "resolutionState": "resolved",
        "scopeDigest": "scope-1",
    }


def missing_entry() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.project-resolution",
        "schemaVersion": "1",
        "state": "project-entry-missing",
        "completeness": "partial",
        "projectIdentity": {
            "projectId": "project-rust-root",
            "projectInstanceId": "project-rust-root@workspace-1",
            "projectEntry": "Cargo.toml",
            "languageId": "rust",
            "providerId": "asp-rust",
            "parserIdentityDigest": "parser-1",
        },
        "repositoryCandidates": git_candidates(),
        "resolutionGeneration": "resolution-1",
        "resolvedSourceScopes": [],
        "conflicts": [],
        "metrics": {
            "parsedManifestCount": 0,
            "parsedLockfileCount": 0,
            "affectedPackageCount": 0,
            "fullWorkspaceReads": 0,
            "fullManifestReparses": 0,
            "dbOpens": 0,
            "elapsedMicros": 100,
        },
        "reasonKind": "provider-project-entry-required",
        "recommendedNext": {
            "command": "asp rust search project-entry --workspace .",
        },
    }


@pytest.mark.parametrize("schema_name", SCHEMA_NAMES)
def test_level_zero_schemas_are_valid_draft_2020_12(schema_name: str) -> None:
    Draft202012Validator.check_schema(SCHEMAS[schema_name])


def test_candidate_snapshot_forbids_root_walk_and_direct_db_open() -> None:
    snapshot = git_candidates()
    validator("repository-candidate-snapshot.v1.schema.json").validate(snapshot)

    for metric in ("fullWorkspaceReads", "fullMerkleRebuilds", "directDbOpens"):
        invalid = deepcopy(snapshot)
        invalid["metrics"][metric] = 1
        with pytest.raises(ValidationError):
            validator("repository-candidate-snapshot.v1.schema.json").validate(invalid)


def test_source_scope_preserves_package_target_and_manifest_authority() -> None:
    validator("resolved-source-scope.v1.schema.json").validate(resolved_scope())

    invalid = resolved_scope()
    invalid["resolutionState"] = "conflicted"
    invalid["conflicts"] = []
    with pytest.raises(ValidationError):
        validator("resolved-source-scope.v1.schema.json").validate(invalid)


def test_non_git_or_markerless_resolution_fails_closed() -> None:
    receipt = missing_entry()
    validator("project-resolution.v1.schema.json").validate(receipt)

    invalid = deepcopy(receipt)
    invalid["reasonKind"] = "manifest-parse-failed"
    with pytest.raises(ValidationError):
        validator("project-resolution.v1.schema.json").validate(invalid)


def test_project_resolution_forbids_root_walk_manifest_reparse_and_db_open() -> None:
    receipt = missing_entry()
    for metric in ("fullWorkspaceReads", "fullManifestReparses", "dbOpens"):
        invalid = deepcopy(receipt)
        invalid["metrics"][metric] = 1
        with pytest.raises(ValidationError):
            validator("project-resolution.v1.schema.json").validate(invalid)
