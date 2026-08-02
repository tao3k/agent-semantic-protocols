"""Verify relationship authorities through parser-owned Org contracts."""

from __future__ import annotations

import json
import subprocess
from collections.abc import Mapping
from pathlib import Path
from typing import Any

from ._relationship_contract_model import (
    RelationshipContractVerificationError,
    normalize_rfc_source,
    require_mapping,
    require_sha256,
    require_string,
    require_workspace_relative_path,
    sha256_text,
)
from ._relationship_contract_projection import resolve_rfc_source_path

ORG_CONTRACT_CANONICALIZATION = "org-contract-source-raw-lf-utf8-v1"
_CONTRACT_FIELDS = {
    "targetPath",
    "contractId",
    "contractPath",
    "canonicalization",
    "contractSha256",
    "assertionCount",
}


def _validate_contract(raw: object, index: int) -> dict[str, Any]:
    contract = dict(
        require_mapping(
            raw,
            "relationshipContract.applicationProfiles.sectionCommitments."
            f"orgContracts[{index}]",
        )
    )
    if set(contract) != _CONTRACT_FIELDS:
        raise RelationshipContractVerificationError(
            "relationshipContract.applicationProfiles.sectionCommitments."
            f"orgContracts[{index}] must contain exactly "
            + ", ".join(sorted(_CONTRACT_FIELDS))
        )
    contract["targetPath"] = require_workspace_relative_path(
        contract["targetPath"], f"orgContracts[{index}].targetPath"
    )
    contract["contractPath"] = require_workspace_relative_path(
        contract["contractPath"], f"orgContracts[{index}].contractPath"
    )
    contract["contractId"] = require_string(
        contract["contractId"], f"orgContracts[{index}].contractId"
    )
    if contract["canonicalization"] != ORG_CONTRACT_CANONICALIZATION:
        raise RelationshipContractVerificationError(
            f"unsupported Org contract canonicalization: {contract['canonicalization']!r}"
        )
    contract["contractSha256"] = require_sha256(
        contract["contractSha256"], f"orgContracts[{index}].contractSha256"
    )
    if (
        isinstance(contract["assertionCount"], bool)
        or not isinstance(contract["assertionCount"], int)
        or contract["assertionCount"] < 1
    ):
        raise RelationshipContractVerificationError(
            f"orgContracts[{index}].assertionCount must be a positive integer"
        )
    return contract


def _run_contract_trace(
    orgize: str | Path, contract_path: Path, target_path: Path, contract_id: str
) -> Mapping[str, Any]:
    command = [
        str(orgize),
        "contract",
        "trace",
        "--json",
        "--org-contract-registry",
        str(contract_path),
        str(target_path),
    ]
    try:
        result = subprocess.run(
            command,
            check=False,
            capture_output=True,
            encoding="utf-8",
            timeout=30.0,
        )
    except subprocess.TimeoutExpired as error:
        raise RelationshipContractVerificationError(
            f"Org contract trace timed out for {contract_id} after 30 seconds"
        ) from error
    except (OSError, UnicodeError) as error:
        raise RelationshipContractVerificationError(
            f"failed to execute Org contract trace for {contract_id}: {error}"
        ) from error
    if result.returncode != 0:
        detail = result.stderr.strip() or f"exit status {result.returncode}"
        raise RelationshipContractVerificationError(
            f"Org contract trace failed for {contract_id}: {detail}"
        )
    try:
        return require_mapping(json.loads(result.stdout), "Org contract trace")
    except json.JSONDecodeError as error:
        raise RelationshipContractVerificationError(
            f"Org contract trace returned invalid JSON for {contract_id}: {error}"
        ) from error


def _decode_evaluation(
    trace: Mapping[str, Any], contract: Mapping[str, Any]
) -> dict[str, Any]:
    if trace.get("schemaVersion") != 1:
        raise RelationshipContractVerificationError(
            "Org contract trace schemaVersion must be 1"
        )
    files = trace.get("files")
    if not isinstance(files, list) or len(files) != 1:
        raise RelationshipContractVerificationError(
            "Org contract trace must contain one file"
        )
    file_record = require_mapping(files[0], "Org contract trace file")
    evaluations = file_record.get("evaluations")
    if not isinstance(evaluations, list) or len(evaluations) != 1:
        raise RelationshipContractVerificationError(
            "Org contract trace must contain exactly one evaluation"
        )
    evaluation = require_mapping(evaluations[0], "Org contract evaluation")
    if evaluation.get("contractId") != contract["contractId"]:
        raise RelationshipContractVerificationError("Org contract id mismatch")
    scope = require_mapping(evaluation.get("scope"), "Org contract scope")
    if scope.get("kind") != "document":
        raise RelationshipContractVerificationError(
            "section commitment Org contract scope must be document"
        )
    assertions = evaluation.get("assertions")
    if not isinstance(assertions, list) or len(assertions) != contract["assertionCount"]:
        observed = len(assertions) if isinstance(assertions, list) else "non-array"
        raise RelationshipContractVerificationError(
            f"Org contract assertion count mismatch: expected "
            f"{contract['assertionCount']}, observed {observed}"
        )
    assertion_ids = []
    failed = []
    for raw_assertion in assertions:
        assertion = require_mapping(raw_assertion, "Org contract assertion")
        assertion_id = require_string(assertion.get("assertionId"), "assertionId")
        assertion_ids.append(assertion_id)
        if assertion.get("status") != "passed":
            failed.append(assertion_id)
    if len(assertion_ids) != len(set(assertion_ids)):
        raise RelationshipContractVerificationError(
            "Org contract assertion ids must be unique"
        )
    if failed:
        raise RelationshipContractVerificationError(
            "Org contract assertions failed: " + ", ".join(sorted(failed))
        )
    return {
        "targetPath": contract["targetPath"],
        "contractId": contract["contractId"],
        "contractPath": contract["contractPath"],
        "contractSha256": contract["contractSha256"],
        "assertionCount": len(assertions),
        "passedAssertionCount": len(assertions),
    }


def verify_org_contracts(
    raw_contracts: object,
    clauses: Mapping[str, Mapping[str, Any]],
    repository_root: Path,
    orgize: str | Path,
) -> list[dict[str, Any]]:
    """Require exact contract coverage and passed Orgize trace receipts."""

    if not isinstance(raw_contracts, list) or not raw_contracts:
        raise RelationshipContractVerificationError(
            "relationshipContract.applicationProfiles.sectionCommitments."
            "orgContracts must be a non-empty array"
        )
    contracts = [_validate_contract(raw, index) for index, raw in enumerate(raw_contracts)]
    targets = [contract["targetPath"] for contract in contracts]
    if len(targets) != len(set(targets)):
        raise RelationshipContractVerificationError(
            "duplicate Org contract targetPath"
        )
    clause_targets = {str(clause["sourcePath"]) for clause in clauses.values()}
    if set(targets) != clause_targets:
        raise RelationshipContractVerificationError(
            "Org contract targets must exactly cover RFC clause source paths"
        )

    receipts = []
    for contract in sorted(contracts, key=lambda item: item["targetPath"]):
        contract_path = resolve_rfc_source_path(
            repository_root, contract["contractPath"]
        )
        target_path = resolve_rfc_source_path(repository_root, contract["targetPath"])
        contract_source = normalize_rfc_source(contract_path.read_text(encoding="utf-8"))
        observed_digest = sha256_text(contract_source)
        if observed_digest != contract["contractSha256"]:
            raise RelationshipContractVerificationError(
                f"{contract['contractId']} contractSha256 mismatch: expected "
                f"{contract['contractSha256']}, observed {observed_digest}"
            )
        trace = _run_contract_trace(
            orgize, contract_path, target_path, contract["contractId"]
        )
        receipts.append(_decode_evaluation(trace, contract))
    return receipts
