"""Shared fixtures for Relationship Contract verification tests."""

from __future__ import annotations

import hashlib
import json
import tempfile
import unittest
from pathlib import Path

CLAUSE_A = "ASP-RFC-10.05.64-COST-VECTOR"
CLAUSE_B = "ASP-RFC-10.05.64-LEXICOGRAPHIC-ORDER"
RECEIPT_CHAIN_ID = "sha256:" + "a" * 64


def source_digest(raw: str) -> str:
    """Hash a fixture using the protocol line-ending normalization."""

    normalized = raw.replace("\r\n", "\n").replace("\r", "\n")
    return hashlib.sha256(normalized.encode("utf-8")).hexdigest()


def dependency_digest(*commitments: dict[str, object]) -> str:
    """Hash fixture dependencies using the recursive canonical shape."""

    canonical = json.dumps(
        [
            {
                "clauseId": commitment["clauseId"],
                "sourceSha256": commitment["sourceSha256"],
                "dependencySetSha256": commitment["dependencySetSha256"],
            }
            for commitment in sorted(
                commitments, key=lambda item: str(item["clauseId"])
            )
        ],
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
    )
    return hashlib.sha256(canonical.encode("utf-8")).hexdigest()


class RelationshipContractFixture(unittest.TestCase):
    """Own a fake Orgize boundary and typed packet builders."""

    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name)
        self.contract_id = "fixture.rfc.v1"
        self.contract_path = self.root / "contracts/rfc.v1.org"
        self.contract_path.parent.mkdir(parents=True)
        self.contract_source = "* fixture RFC contract\n"
        self.contract_path.write_text(self.contract_source)
        self.authority_path = self.root / "org/relationships/fixture.v1.org"
        self.authority_path.parent.mkdir(parents=True)
        self.authority_source = "#+RELATIONSHIP_CONTRACT_ID: fixture.relationship.v1\n"
        self.authority_path.write_text(self.authority_source)
        self.relationship_contract_path = (
            self.root / "org/contracts/relationship.v1.org"
        )
        self.relationship_contract_path.parent.mkdir(parents=True)
        self.relationship_contract_source = "* fixture relationship contract\n"
        self.relationship_contract_path.write_text(self.relationship_contract_source)
        self.orgize = self.root / "fake-orgize"
        self.orgize.write_text(
            """#!/usr/bin/env python3
import json
from pathlib import Path
import sys

if sys.argv[1:3] == ["elements-query", "--packet"] and len(sys.argv) == 5:
    query = json.loads(sys.argv[3])
    source = Path(sys.argv[4])
    if query == {"schemaVersion": 1, "kind": "node-property"}:
        print(Path(str(source) + ".properties-response.json").read_text())
    else:
        fixture = json.loads(Path(str(source) + ".response.json").read_text())
        outline = query.get("outlinePathPrefix")
        if query != {
            "category": "section",
            "outlinePathExactLen": len(outline),
            "outlinePathPrefix": outline,
            "schemaVersion": 1,
        }:
            raise SystemExit("query is not an exact section outline query")
        records = [
            record for record in fixture["records"]
            if record.get("outlinePath") == outline
        ]
        print(json.dumps(records))
elif sys.argv[1:3] == ["contract", "trace"] and len(sys.argv) == 7:
    target = Path(sys.argv[6])
    print(Path(str(target) + ".contract-response.json").read_text())
else:
    raise SystemExit("unexpected invocation")
"""
        )
        self.orgize.chmod(0o755)

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def write_source(
        self,
        relative_path: str,
        outline_path: list[str],
        raw: str,
        *,
        records: list[dict[str, object]] | None = None,
        contract_status: str = "passed",
    ) -> None:
        source = self.root / relative_path
        source.parent.mkdir(parents=True, exist_ok=True)
        source.write_text("fixture source\n")
        response = {
            "records": records
            if records is not None
            else [
                {
                    "category": "section",
                    "outlinePath": outline_path,
                    "source": {"raw": raw},
                }
            ],
        }
        Path(str(source) + ".response.json").write_text(json.dumps(response))
        contract_response = {
            "schemaVersion": 1,
            "files": [
                {
                    "path": str(source),
                    "evaluations": [
                        {
                            "schemaVersion": 1,
                            "contractId": self.contract_id,
                            "scope": {
                                "kind": "document",
                                "range": {"start": 0, "end": 0},
                            },
                            "assertions": [
                                {
                                    "assertionId": "fixture.rfc-structure",
                                    "status": contract_status,
                                }
                            ],
                        }
                    ],
                }
            ],
        }
        Path(str(source) + ".contract-response.json").write_text(
            json.dumps(contract_response)
        )

    def commitment(
        self,
        clause_id: str,
        source_path: str,
        outline_path: list[str],
        raw: str,
        *,
        depends_on: list[str] | None = None,
        dependency_set_sha256: str | None = None,
    ) -> dict[str, object]:
        return {
            "clauseId": clause_id,
            "sourcePath": source_path,
            "outlinePath": outline_path,
            "sourceSha256": source_digest(raw),
            "dependsOn": depends_on or [],
            "dependencySetSha256": dependency_set_sha256
            if dependency_set_sha256 is not None
            else dependency_digest(),
        }

    def write_packet(self, clauses: list[dict[str, object]]) -> Path:
        packet = self.root / "handoff.json"
        targets = sorted({str(clause["sourcePath"]) for clause in clauses})
        if len(targets) != 1:
            raise AssertionError("fixture application profile requires one target")
        artifacts = [
            {
                "artifactId": "doc.fixture-authority",
                "artifactKind": "org-document",
                "artifactPath": "org/relationships/fixture.v1.org",
                "canonicalization": "org-source-raw-lf-utf8-v1",
                "expectedSha256": source_digest(self.authority_source),
            },
            {
                "artifactId": "contract.fixture-relationship",
                "artifactKind": "org-contract",
                "artifactPath": "org/contracts/relationship.v1.org",
                "canonicalization": "org-source-raw-lf-utf8-v1",
                "expectedSha256": source_digest(self.relationship_contract_source),
            },
        ]
        relationships = [
            {
                "relationshipId": "fixture-conforms-v1",
                "subject": "doc.fixture-authority",
                "predicate": "conforms-to",
                "object": "contract.fixture-relationship",
                "impact": "invalidates-subject",
                "evidenceGates": ["fixture-relationship-gate"],
            }
        ]
        property_records = []
        for key, value in {
            "RELATIONSHIP_CONTRACT_ID": "fixture.relationship.v1",
            "RELATIONSHIP_SCHEMA_VERSION": "1",
        }.items():
            property_records.append(
                {"outlinePath": [], "summary": {"key": key, "value": value}}
            )
        for index, artifact in enumerate(artifacts):
            outline = ["Artifacts", f"Artifact {index}"]
            for key, value in {
                "ARTIFACT_ID": artifact["artifactId"],
                "ARTIFACT_KIND": artifact["artifactKind"],
                "ARTIFACT_PATH": artifact["artifactPath"],
                "CANONICALIZATION": artifact["canonicalization"],
            }.items():
                property_records.append(
                    {"outlinePath": outline, "summary": {"key": key, "value": value}}
                )
        relationship = relationships[0]
        outline = ["Relationships", "Fixture edge"]
        for key, value in {
            "RELATIONSHIP_ID": relationship["relationshipId"],
            "SUBJECT": relationship["subject"],
            "PREDICATE": relationship["predicate"],
            "OBJECT": relationship["object"],
            "IMPACT": relationship["impact"],
            "EVIDENCE_GATE": relationship["evidenceGates"][0],
        }.items():
            property_records.append(
                {"outlinePath": outline, "summary": {"key": key, "value": value}}
            )
        Path(str(self.authority_path) + ".properties-response.json").write_text(
            json.dumps(property_records)
        )
        authority_assertion = {
            "assertionId": "fixture.relationship",
            "status": "passed",
        }
        authority_evaluations = [
            {
                "contractId": "relationship.document.v1",
                "assertions": [authority_assertion],
            },
            *[
                {
                    "contractId": "relationship.artifact.v1",
                    "assertions": [authority_assertion],
                }
                for _ in artifacts
            ],
            {
                "contractId": "relationship.edge.v1",
                "assertions": [authority_assertion],
            },
        ]
        Path(str(self.authority_path) + ".contract-response.json").write_text(
            json.dumps(
                {
                    "schemaVersion": 1,
                    "files": [{"evaluations": authority_evaluations}],
                }
            )
        )
        packet.write_text(
            json.dumps(
                {
                    "receiptChainId": RECEIPT_CHAIN_ID,
                    "relationshipContract": {
                        "authority": {
                            "sourcePath": "org/relationships/fixture.v1.org",
                            "contractPath": "org/contracts/relationship.v1.org",
                            "contractId": "relationship.document.v1",
                            "relationshipContractId": "fixture.relationship.v1",
                            "canonicalization": "org-contract-source-raw-lf-utf8-v1",
                            "sourceSha256": source_digest(self.authority_source),
                        },
                        "artifacts": artifacts,
                        "relationships": relationships,
                        "evidenceGates": ["fixture-relationship-gate"],
                        "applicationProfiles": {
                            "sectionCommitments": {
                                "canonicalization": "orgize-section-source-raw-lf-utf8-v1",
                                "orgContract": {
                                    "targetPath": targets[0],
                                    "contractId": self.contract_id,
                                    "contractPath": "contracts/rfc.v1.org",
                                    "canonicalization": (
                                        "org-contract-source-raw-lf-utf8-v1"
                                    ),
                                    "contractSha256": source_digest(
                                        self.contract_source
                                    ),
                                    "assertionCount": 1,
                                },
                                "commitments": clauses,
                            }
                        },
                    },
                }
            )
        )
        return packet
