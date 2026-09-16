# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Validate scalar fields in a Relationship Contract projection."""

from __future__ import annotations

import hashlib
import re
from collections.abc import Mapping
from pathlib import Path
from typing import Any

RELATIONSHIP_CONTRACT_CANONICALIZATION = "orgize-section-source-raw-lf-utf8-v1"
RELATIONSHIP_CONTRACT_RECEIPT_SCHEMA_ID = (
    "asp.relationship-contract-verification-receipt.v1"
)

_CLAUSE_ID = re.compile(r"^ASP-RFC-[A-Za-z0-9._-]+$")
_SHA256 = re.compile(r"^[0-9a-f]{64}$")
_RECEIPT_CHAIN_ID = re.compile(r"^sha256:[0-9a-f]{64}$")
_CLAUSE_FIELDS = {
    "clauseId",
    "sourcePath",
    "outlinePath",
    "sourceSha256",
    "dependsOn",
    "dependencySetSha256",
}


class RelationshipContractVerificationError(ValueError):
    """Raised when a relationship contract cannot be independently reproduced."""


def require_mapping(value: object, label: str) -> Mapping[str, Any]:
    """Return a JSON object or reject the typed boundary."""

    if not isinstance(value, dict):
        raise RelationshipContractVerificationError(f"{label} must be a JSON object")
    return value


def require_string(value: object, label: str) -> str:
    """Return a non-empty string or reject the typed boundary."""

    if not isinstance(value, str) or not value:
        raise RelationshipContractVerificationError(f"{label} must be a non-empty string")
    return value


def sha256_text(value: str) -> str:
    """Hash one normalized UTF-8 text value."""

    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def normalize_rfc_source(raw: str) -> str:
    """Normalize source line endings without changing semantic text."""

    return raw.replace("\r\n", "\n").replace("\r", "\n")


def require_receipt_chain_id(value: object) -> str:
    """Validate and return the packet receipt-chain identity."""

    receipt_chain_id = require_string(value, "receiptChainId")
    if _RECEIPT_CHAIN_ID.fullmatch(receipt_chain_id) is None:
        raise RelationshipContractVerificationError(
            "receiptChainId must be sha256: followed by a lowercase SHA-256 digest"
        )
    return receipt_chain_id


def _require_clause_fields(clause: dict[str, Any], index: int) -> None:
    if set(clause) == _CLAUSE_FIELDS:
        return
    missing = sorted(_CLAUSE_FIELDS - set(clause))
    unknown = sorted(set(clause) - _CLAUSE_FIELDS)
    details = []
    if missing:
        details.append("missing " + ", ".join(missing))
    if unknown:
        details.append("unknown " + ", ".join(unknown))
    raise RelationshipContractVerificationError(
        "relationshipContract.applicationProfiles.sectionCommitments."
        f"commitments[{index}] has invalid fields: " + "; ".join(details)
    )


def require_workspace_relative_path(value: object, label: str) -> str:
    """Return a workspace-relative path without allowing parent traversal."""

    source_path = require_string(value, label)
    relative_path = Path(source_path)
    if relative_path.is_absolute() or ".." in relative_path.parts:
        raise RelationshipContractVerificationError(
            f"RFC source path must be workspace-relative: {source_path}"
        )
    return source_path


def _require_outline_path(value: object, clause_id: str) -> list[str]:
    if (
        not isinstance(value, list)
        or not value
        or any(not isinstance(item, str) or not item for item in value)
    ):
        raise RelationshipContractVerificationError(
            f"{clause_id}.outlinePath must be a non-empty string array"
        )
    return value


def require_sha256(value: object, label: str) -> str:
    """Return one lowercase SHA-256 digest."""

    digest = require_string(value, label)
    if _SHA256.fullmatch(digest) is None:
        raise RelationshipContractVerificationError(
            f"{label} must be a lowercase SHA-256 digest"
        )
    return digest


def _require_dependencies(value: object, clause_id: str) -> list[str]:
    if not isinstance(value, list) or any(
        not isinstance(dependency, str) or _CLAUSE_ID.fullmatch(dependency) is None
        for dependency in value
    ):
        raise RelationshipContractVerificationError(
            f"{clause_id}.dependsOn must be an RFC clause id array"
        )
    if len(value) != len(set(value)):
        raise RelationshipContractVerificationError(
            f"{clause_id}.dependsOn contains duplicate clause ids"
        )
    return value


def validate_rfc_clause(raw_clause: object, index: int) -> dict[str, Any]:
    """Validate one exact clause record from the handoff packet."""

    clause = dict(
        require_mapping(
            raw_clause,
            "relationshipContract.applicationProfiles.sectionCommitments."
            f"commitments[{index}]",
        )
    )
    _require_clause_fields(clause, index)
    clause_id = require_string(clause["clauseId"], f"clause[{index}].clauseId")
    if _CLAUSE_ID.fullmatch(clause_id) is None:
        raise RelationshipContractVerificationError(
            f"invalid RFC clause id: {clause_id}"
        )
    clause["sourcePath"] = require_workspace_relative_path(
        clause["sourcePath"], f"{clause_id}.sourcePath"
    )
    clause["outlinePath"] = _require_outline_path(clause["outlinePath"], clause_id)
    clause["sourceSha256"] = require_sha256(
        clause["sourceSha256"], f"{clause_id}.sourceSha256"
    )
    clause["dependsOn"] = _require_dependencies(clause["dependsOn"], clause_id)
    clause["dependencySetSha256"] = require_sha256(
        clause["dependencySetSha256"], f"{clause_id}.dependencySetSha256"
    )
    return clause
