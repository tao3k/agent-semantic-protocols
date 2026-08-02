"""Reconstruct a Relationship Contract from its parser-owned Org authority."""

from __future__ import annotations

import json
import subprocess
from collections import defaultdict
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from ._relationship_contract_model import (
    RelationshipContractVerificationError,
    normalize_rfc_source,
    require_mapping,
    require_string,
    sha256_text,
)
from ._relationship_contract_org import _run_contract_trace
from ._relationship_contract_projection import resolve_rfc_source_path


def _query_node_properties(
    orgize: str | Path, source_path: Path
) -> list[Mapping[str, Any]]:
    packet = json.dumps(
        {"schemaVersion": 1, "kind": "node-property"},
        sort_keys=True,
        separators=(",", ":"),
    )
    try:
        result = subprocess.run(
            [str(orgize), "elements-query", "--packet", packet, str(source_path)],
            check=False,
            capture_output=True,
            encoding="utf-8",
            timeout=30.0,
        )
    except subprocess.TimeoutExpired as error:
        raise RelationshipContractVerificationError(
            "relationship authority node-property query timed out after 30 seconds"
        ) from error
    except (OSError, UnicodeError) as error:
        raise RelationshipContractVerificationError(
            f"failed to query relationship authority: {error}"
        ) from error
    if result.returncode != 0:
        detail = result.stderr.strip() or f"exit status {result.returncode}"
        raise RelationshipContractVerificationError(
            f"relationship authority query failed: {detail}"
        )
    try:
        records = json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise RelationshipContractVerificationError(
            f"relationship authority query returned invalid JSON: {error}"
        ) from error
    if not isinstance(records, list):
        raise RelationshipContractVerificationError(
            "relationship authority query must return an array"
        )
    return [
        require_mapping(record, "relationship authority property") for record in records
    ]


def _property_scopes(
    records: Sequence[Mapping[str, Any]],
) -> dict[tuple[str, ...], dict[str, list[str]]]:
    scopes: dict[tuple[str, ...], dict[str, list[str]]] = defaultdict(
        lambda: defaultdict(list)
    )
    for record in records:
        outline = record.get("outlinePath")
        summary = require_mapping(record.get("summary"), "node-property summary")
        if not isinstance(outline, list) or any(
            not isinstance(item, str) for item in outline
        ):
            raise RelationshipContractVerificationError("invalid authority outlinePath")
        key = require_string(summary.get("key"), "node-property key")
        value = require_string(summary.get("value"), f"node-property {key}")
        scopes[tuple(outline)][key].append(value)
    return scopes


def _one(properties: Mapping[str, list[str]], key: str, label: str) -> str:
    values = properties.get(key, [])
    if len(values) != 1:
        raise RelationshipContractVerificationError(
            f"{label} must declare exactly one {key}"
        )
    return values[0]


def _reconstruct_artifacts(
    scopes: Mapping[tuple[str, ...], Mapping[str, list[str]]],
) -> list[dict[str, Any]]:
    artifacts = []
    for outline, properties in scopes.items():
        if "ARTIFACT_ID" not in properties:
            continue
        label = " / ".join(outline)
        artifact = {
            "artifactId": _one(properties, "ARTIFACT_ID", label),
            "artifactKind": _one(properties, "ARTIFACT_KIND", label),
            "artifactPath": _one(properties, "ARTIFACT_PATH", label),
            "canonicalization": _one(properties, "CANONICALIZATION", label),
        }
        selectors = properties.get("ARTIFACT_SELECTOR", [])
        if selectors:
            if len(selectors) != 1:
                raise RelationshipContractVerificationError(
                    f"{label} must declare at most one ARTIFACT_SELECTOR"
                )
            artifact["artifactSelector"] = selectors[0]
        artifacts.append(artifact)
    return sorted(artifacts, key=lambda item: item["artifactId"])


def _reconstruct_relationships(
    scopes: Mapping[tuple[str, ...], Mapping[str, list[str]]],
) -> list[dict[str, Any]]:
    relationships = []
    for outline, properties in scopes.items():
        if "RELATIONSHIP_ID" not in properties:
            continue
        label = " / ".join(outline)
        relationships.append(
            {
                "relationshipId": _one(properties, "RELATIONSHIP_ID", label),
                "subject": _one(properties, "SUBJECT", label),
                "predicate": _one(properties, "PREDICATE", label),
                "object": _one(properties, "OBJECT", label),
                "impact": _one(properties, "IMPACT", label),
                "evidenceGates": sorted(properties.get("EVIDENCE_GATE", [])),
            }
        )
    return sorted(relationships, key=lambda item: item["relationshipId"])


def _verify_trace(
    trace: Mapping[str, Any], contract_id: str, artifact_count: int, edge_count: int
) -> tuple[int, int]:
    files = trace.get("files")
    if (
        trace.get("schemaVersion") != 1
        or not isinstance(files, list)
        or len(files) != 1
    ):
        raise RelationshipContractVerificationError(
            "invalid relationship Org contract trace"
        )
    evaluations = require_mapping(files[0], "relationship trace file").get(
        "evaluations"
    )
    if not isinstance(evaluations, list):
        raise RelationshipContractVerificationError(
            "relationship trace evaluations must be an array"
        )
    expected = {
        contract_id: 1,
        "relationship.artifact.v1": artifact_count,
        "relationship.edge.v1": edge_count,
    }
    observed: dict[str, int] = defaultdict(int)
    assertions = []
    for raw_evaluation in evaluations:
        evaluation = require_mapping(raw_evaluation, "relationship evaluation")
        observed[str(evaluation.get("contractId"))] += 1
        raw_assertions = evaluation.get("assertions")
        if not isinstance(raw_assertions, list):
            raise RelationshipContractVerificationError(
                "relationship assertions must be an array"
            )
        assertions.extend(
            require_mapping(item, "relationship assertion") for item in raw_assertions
        )
    if dict(observed) != expected:
        raise RelationshipContractVerificationError(
            f"relationship contract evaluation coverage mismatch: expected {expected}, observed {dict(observed)}"
        )
    failed = [
        str(item.get("assertionId"))
        for item in assertions
        if item.get("status") != "passed"
    ]
    if failed:
        raise RelationshipContractVerificationError(
            "relationship Org contract assertions failed: " + ", ".join(sorted(failed))
        )
    return len(evaluations), len(assertions)


def verify_relationship_authority(
    authority: Mapping[str, Any],
    artifacts: Sequence[Mapping[str, Any]],
    relationships: Sequence[Mapping[str, Any]],
    repository_root: Path,
    orgize: str | Path,
) -> dict[str, Any]:
    """Verify source identity, Org Contracts, and exact node-property projection."""

    source_path = resolve_rfc_source_path(repository_root, str(authority["sourcePath"]))
    contract_path = resolve_rfc_source_path(
        repository_root, str(authority["contractPath"])
    )
    observed_sha256 = sha256_text(
        normalize_rfc_source(source_path.read_text(encoding="utf-8"))
    )
    if observed_sha256 != authority["sourceSha256"]:
        raise RelationshipContractVerificationError(
            f"relationship authority sourceSha256 mismatch: expected {authority['sourceSha256']}, observed {observed_sha256}"
        )
    scopes = _property_scopes(_query_node_properties(orgize, source_path))
    document = scopes.get((), {})
    observed_contract_id = _one(
        document, "RELATIONSHIP_CONTRACT_ID", "authority document"
    )
    if observed_contract_id != authority["relationshipContractId"]:
        raise RelationshipContractVerificationError(
            "relationship authority id mismatch"
        )
    if _one(document, "RELATIONSHIP_SCHEMA_VERSION", "authority document") != "1":
        raise RelationshipContractVerificationError(
            "relationship authority schemaVersion must be 1"
        )
    expected_artifacts = [
        {key: value for key, value in artifact.items() if key != "expectedSha256"}
        for artifact in artifacts
    ]
    if _reconstruct_artifacts(scopes) != sorted(
        expected_artifacts, key=lambda item: item["artifactId"]
    ):
        raise RelationshipContractVerificationError(
            "relationship authority artifact projection mismatch"
        )
    if _reconstruct_relationships(scopes) != sorted(
        relationships, key=lambda item: item["relationshipId"]
    ):
        raise RelationshipContractVerificationError(
            "relationship authority edge projection mismatch"
        )
    trace = _run_contract_trace(
        orgize, contract_path, source_path, str(authority["contractId"])
    )
    evaluation_count, assertion_count = _verify_trace(
        trace, str(authority["contractId"]), len(artifacts), len(relationships)
    )
    return {
        **dict(authority),
        "observedSha256": observed_sha256,
        "evaluationCount": evaluation_count,
        "assertionCount": assertion_count,
    }
