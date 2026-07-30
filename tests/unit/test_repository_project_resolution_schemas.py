import json
from pathlib import Path

import pytest
from jsonschema.validators import validator_for
from referencing import Registry, Resource


ROOT = Path(__file__).resolve().parents[2]
SCHEMA_NAMES = (
    "repository-candidate-snapshot.v1.schema.json",
    "language-package-graph.v1.schema.json",
    "project-resolution.v1.schema.json",
    "provider-project-resolution-descriptor.v1.schema.json",
)


def load_schemas() -> dict[str, dict]:
    schemas = {}
    for name in SCHEMA_NAMES:
        schema = json.loads((ROOT / "schemas" / name).read_text())
        validator_for(schema).check_schema(schema)
        schemas[schema["$id"]] = schema
    return schemas


def git_candidates() -> dict:
    return {
        "schemaId": "agent.semantic-protocols.repository-candidate-snapshot",
        "schemaVersion": "1",
        "mode": "git",
        "repositoryIdentity": {
            "kind": "git",
            "repositoryId": "repo-1",
            "root": "/workspace",
            "gitCommonDir": "/workspace/.git",
        },
        "worktreeIdentity": {
            "workspaceId": "workspace-1",
            "checkoutRoot": "/workspace",
            "gitDir": "/workspace/.git",
            "headOid": "abc",
            "indexDigest": "index-1",
            "sparseCheckout": False,
        },
        "candidateGeneration": "candidate-1",
        "candidates": [
            {
                "path": "Cargo.toml",
                "state": "indexed",
                "authority": "git-index",
            },
            {
                "path": "src/lib.rs",
                "state": "modified",
                "authority": "git-worktree",
            },
        ],
        "metrics": {
            "candidateCount": 2,
            "trackedCount": 2,
            "changedCount": 1,
            "untrackedCount": 0,
            "deletedCount": 0,
            "fullWorkspaceReads": 0,
            "elapsedMicros": 400,
        },
    }


def rust_package_graph() -> dict:
    return {
        "schemaId": "agent.semantic-protocols.language-package-graph",
        "schemaVersion": "1",
        "languageId": "rust",
        "providerId": "rs-harness",
        "projectEntry": "Cargo.toml",
        "parserId": "cargo-project-resolution",
        "manifests": [
            {
                "path": "Cargo.toml",
                "kind": "cargo-manifest",
                "digest": "manifest-1",
            }
        ],
        "lockfiles": [
            {
                "path": "Cargo.lock",
                "kind": "cargo-lock",
                "digest": "lock-1",
            }
        ],
        "packages": [
            {
                "packageId": "package-root",
                "name": "root",
                "version": "0.1.0",
                "manifestPath": "Cargo.toml",
                "root": ".",
                "workspaceMember": True,
                "targets": [
                    {
                        "targetId": "target-lib",
                        "kind": "lib",
                        "name": "root",
                        "explicit": False,
                        "sourceRoots": ["src"],
                        "entrypoints": ["src/lib.rs"],
                        "generatedRoots": [],
                    }
                ],
            }
        ],
        "internalDependencyEdges": [],
        "externalDependencies": [],
        "unresolved": [],
    }


def resolved_project() -> dict:
    return {
        "schemaId": "agent.semantic-protocols.project-resolution",
        "schemaVersion": "1",
        "state": "resolved",
        "completeness": "exact",
        "projectIdentity": {
            "projectId": "project-rust-root",
            "projectInstanceId": "project-rust-root@workspace-1",
            "projectEntry": "Cargo.toml",
            "languageId": "rust",
            "providerId": "rs-harness",
            "parserIdentityDigest": "parser-1",
        },
        "repositoryCandidates": git_candidates(),
        "packageGraph": rust_package_graph(),
        "resolutionGeneration": "resolution-1",
        "resolvedSourceScopes": [
            {
                "scopeId": "scope-target-lib",
                "packageId": "package-root",
                "targetId": "target-lib",
                "roots": ["src"],
                "extensions": ["rs"],
                "includeAuthority": "package-manager",
                "exclusions": [
                    {
                        "prefix": "target",
                        "authority": "asp-infrastructure",
                    }
                ],
            }
        ],
        "conflicts": [],
        "metrics": {
            "parsedManifestCount": 1,
            "parsedLockfileCount": 1,
            "affectedPackageCount": 1,
            "fullWorkspaceReads": 0,
            "fullManifestReparses": 0,
            "dbOpens": 0,
            "elapsedMicros": 800,
        },
    }


def validator(schema_id: str):
    schemas = load_schemas()
    schema = next(
        schema
        for absolute_id, schema in schemas.items()
        if absolute_id.endswith(f"/{schema_id}")
    )
    registry = Registry().with_resources(
        (absolute_id, Resource.from_contents(document))
        for absolute_id, document in schemas.items()
    )
    return validator_for(schema)(schema, registry=registry)


def test_project_resolution_schema_family_accepts_resolved_project() -> None:
    schemas = load_schemas()
    validator("repository-candidate-snapshot.v1.schema.json").validate(git_candidates())
    validator("language-package-graph.v1.schema.json").validate(rust_package_graph())
    validator("project-resolution.v1.schema.json").validate(resolved_project())

    descriptor = {
        "schemaId": "agent.semantic-protocols.provider-project-resolution-descriptor",
        "schemaVersion": "1",
        "capabilityId": "project-resolution",
        "entryMarkers": ["Cargo.toml"],
        "manifestKinds": ["cargo-manifest"],
        "lockfileKinds": ["cargo-lock"],
        "supportsGitCandidates": True,
        "supportsProviderOnly": True,
        "parserId": "rust.cargo-toml",
            "commandBinding": "project-resolution-stdin",
            "candidateSnapshotSchema": "https://schemas.agent-semantic-protocols.dev/repository-candidate-snapshot.v1.schema.json",
            "packageGraphSchema": "https://schemas.agent-semantic-protocols.dev/language-package-graph.v1.schema.json",
            "resolvedSourceScopeSchema": "https://schemas.agent-semantic-protocols.dev/resolved-source-scope.v1.schema.json",
            "projectResolutionSchema": "https://schemas.agent-semantic-protocols.dev/project-resolution.v1.schema.json",
    }
    validator("provider-project-resolution-descriptor.v1.schema.json").validate(
        descriptor
    )
    assert len(schemas) == 4


def test_project_resolution_schema_family_rejects_root_walk_and_db_open() -> None:
    project = resolved_project()
    project["repositoryCandidates"]["metrics"]["fullWorkspaceReads"] = 1
    project["metrics"]["dbOpens"] = 1

    errors = list(validator("project-resolution.v1.schema.json").iter_errors(project))
    assert len(errors) == 2
    assert {error.validator for error in errors} == {"const"}


def test_project_resolution_failure_is_typed_and_actionable() -> None:
    project = resolved_project()
    project.pop("packageGraph")
    project["state"] = "project-entry-missing"
    project["completeness"] = "partial"
    project["reasonKind"] = "provider-project-entry-required"
    project["recommendedNext"] = {
        "command": "asp rust search project-entry --workspace ."
    }
    project["resolvedSourceScopes"] = []

    validator("project-resolution.v1.schema.json").validate(project)

    project.pop("recommendedNext")
    with pytest.raises(Exception):
        validator("project-resolution.v1.schema.json").validate(project)
