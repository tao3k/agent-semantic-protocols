# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Validate the generic V1 Relationship Contract packet projection."""

from __future__ import annotations

from collections.abc import Mapping
from typing import Any

from ._relationship_contract_model import (
    RelationshipContractVerificationError,
    require_mapping,
    require_sha256,
    require_string,
    require_workspace_relative_path,
    validate_rfc_clause,
)

ARTIFACT_KINDS = {
    "org-document",
    "org-section",
    "source-file",
    "proof-module",
    "schema",
    "receipt",
    "org-contract",
}
PREDICATES = {
    "governed-by",
    "depends-on",
    "references",
    "realizes",
    "proves",
    "audits",
    "conforms-to",
    "renders",
}
IMPACTS = {"informational", "invalidates-subject", "invalidates-object"}


def _exact(record: Mapping[str, Any], fields: set[str], label: str) -> None:
    if set(record) != fields:
        raise RelationshipContractVerificationError(
            f"{label} must contain exactly " + ", ".join(sorted(fields))
        )


def _strings(value: object, label: str) -> list[str]:
    if not isinstance(value, list) or not value:
        raise RelationshipContractVerificationError(
            f"{label} must be a non-empty array"
        )
    values = [require_string(item, label) for item in value]
    if len(values) != len(set(values)):
        raise RelationshipContractVerificationError(
            f"{label} must contain unique values"
        )
    return values


def validate_authority(raw: object) -> dict[str, Any]:
    authority = dict(require_mapping(raw, "relationshipContract.authority"))
    _exact(
        authority,
        {
            "sourcePath",
            "contractPath",
            "contractId",
            "relationshipContractId",
            "canonicalization",
            "sourceSha256",
        },
        "relationshipContract.authority",
    )
    authority["sourcePath"] = require_workspace_relative_path(
        authority["sourcePath"], "authority.sourcePath"
    )
    authority["contractPath"] = require_workspace_relative_path(
        authority["contractPath"], "authority.contractPath"
    )
    authority["contractId"] = require_string(
        authority["contractId"], "authority.contractId"
    )
    authority["relationshipContractId"] = require_string(
        authority["relationshipContractId"], "authority.relationshipContractId"
    )
    if authority["canonicalization"] != "org-contract-source-raw-lf-utf8-v1":
        raise RelationshipContractVerificationError(
            "unsupported authority canonicalization"
        )
    authority["sourceSha256"] = require_sha256(
        authority["sourceSha256"], "authority.sourceSha256"
    )
    return authority


def validate_artifacts(raw: object) -> list[dict[str, Any]]:
    if not isinstance(raw, list) or not raw:
        raise RelationshipContractVerificationError(
            "relationshipContract.artifacts must be non-empty"
        )
    artifacts = []
    for index, value in enumerate(raw):
        artifact = dict(require_mapping(value, f"artifacts[{index}]"))
        required = {
            "artifactId",
            "artifactKind",
            "artifactPath",
            "canonicalization",
            "expectedSha256",
        }
        allowed = required | {"artifactSelector"}
        if not required <= set(artifact) or not set(artifact) <= allowed:
            raise RelationshipContractVerificationError(
                f"artifacts[{index}] has invalid fields"
            )
        artifact["artifactId"] = require_string(
            artifact["artifactId"], f"artifacts[{index}].artifactId"
        )
        artifact["artifactKind"] = require_string(
            artifact["artifactKind"], f"artifacts[{index}].artifactKind"
        )
        if artifact["artifactKind"] not in ARTIFACT_KINDS:
            raise RelationshipContractVerificationError(
                f"unsupported artifact kind: {artifact['artifactKind']}"
            )
        artifact["artifactPath"] = require_workspace_relative_path(
            artifact["artifactPath"], f"artifacts[{index}].artifactPath"
        )
        if "artifactSelector" in artifact:
            artifact["artifactSelector"] = require_string(
                artifact["artifactSelector"], f"artifacts[{index}].artifactSelector"
            )
        artifact["canonicalization"] = require_string(
            artifact["canonicalization"], f"artifacts[{index}].canonicalization"
        )
        artifact["expectedSha256"] = require_sha256(
            artifact["expectedSha256"], f"artifacts[{index}].expectedSha256"
        )
        artifacts.append(artifact)
    ids = [artifact["artifactId"] for artifact in artifacts]
    if len(ids) != len(set(ids)):
        raise RelationshipContractVerificationError("artifactId values must be unique")
    return artifacts


def validate_relationships(raw: object) -> list[dict[str, Any]]:
    if not isinstance(raw, list) or not raw:
        raise RelationshipContractVerificationError(
            "relationshipContract.relationships must be non-empty"
        )
    relationships = []
    fields = {
        "relationshipId",
        "subject",
        "predicate",
        "object",
        "impact",
        "evidenceGates",
    }
    for index, value in enumerate(raw):
        relationship = dict(require_mapping(value, f"relationships[{index}]"))
        _exact(relationship, fields, f"relationships[{index}]")
        for field in fields - {"evidenceGates"}:
            relationship[field] = require_string(
                relationship[field], f"relationships[{index}].{field}"
            )
        if relationship["predicate"] not in PREDICATES:
            raise RelationshipContractVerificationError(
                f"unsupported relationship predicate: {relationship['predicate']}"
            )
        if relationship["impact"] not in IMPACTS:
            raise RelationshipContractVerificationError(
                f"unsupported relationship impact: {relationship['impact']}"
            )
        relationship["evidenceGates"] = _strings(
            relationship["evidenceGates"], f"relationships[{index}].evidenceGates"
        )
        relationships.append(relationship)
    ids = [relationship["relationshipId"] for relationship in relationships]
    if len(ids) != len(set(ids)):
        raise RelationshipContractVerificationError(
            "relationshipId values must be unique"
        )
    return relationships


def validate_section_profile(raw: object) -> dict[str, Any]:
    profile = dict(require_mapping(raw, "applicationProfiles.sectionCommitments"))
    _exact(
        profile,
        {"canonicalization", "orgContract", "commitments"},
        "sectionCommitments",
    )
    if profile["canonicalization"] != "orgize-section-source-raw-lf-utf8-v1":
        raise RelationshipContractVerificationError(
            "unsupported section commitment canonicalization"
        )
    commitments = profile["commitments"]
    if not isinstance(commitments, list) or not commitments:
        raise RelationshipContractVerificationError(
            "sectionCommitments.commitments must be non-empty"
        )
    profile["commitments"] = [
        validate_rfc_clause(value, index) for index, value in enumerate(commitments)
    ]
    return profile


def validate_relationship_contract(raw: object) -> dict[str, Any]:
    contract = dict(require_mapping(raw, "relationshipContract"))
    fields = {
        "authority",
        "artifacts",
        "relationships",
        "evidenceGates",
        "applicationProfiles",
    }
    _exact(contract, fields, "relationshipContract")
    contract["authority"] = validate_authority(contract["authority"])
    contract["artifacts"] = validate_artifacts(contract["artifacts"])
    contract["relationships"] = validate_relationships(contract["relationships"])
    contract["evidenceGates"] = _strings(
        contract["evidenceGates"], "relationshipContract.evidenceGates"
    )
    profiles = dict(
        require_mapping(contract["applicationProfiles"], "applicationProfiles")
    )
    _exact(profiles, {"sectionCommitments"}, "applicationProfiles")
    profiles["sectionCommitments"] = validate_section_profile(
        profiles["sectionCommitments"]
    )
    contract["applicationProfiles"] = profiles
    return contract
