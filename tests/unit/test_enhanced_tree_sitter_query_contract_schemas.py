# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Executable shared-schema contracts for resident enhanced Query V1."""

from __future__ import annotations

import copy
import json
from pathlib import Path

import pytest
from jsonschema import ValidationError

from unit.schema_validation import schema_validator_for


ROOT = Path(__file__).resolve().parents[2]
DIGEST = "blake3-256:" + "1" * 64


def resident_plan() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.resident-syntax-query-plan",
        "schemaVersion": "1",
        "profileId": "asp.enhanced-tree-sitter-query.v1",
        "planDigest": DIGEST,
        "queryDigest": DIGEST,
        "languageId": "rust",
        "providerId": "asp-rust",
        "parserAbiDigest": DIGEST,
        "queryGrammarDigest": DIGEST,
        "operatorTableDigest": DIGEST,
        "capabilityTableDigest": DIGEST,
        "generationDigest": DIGEST,
        "patterns": [
            {
                "index": 0,
                "captures": [
                    {
                        "name": "item",
                        "residentFactPath": "selector",
                        "cardinality": {"minimum": 1, "maximum": 1},
                        "capabilityRowId": "rust.capture.item",
                    }
                ],
                "structure": {
                    "kind": "true",
                    "origin": {
                        "kind": "capture",
                        "capabilityRowId": "rust.capture.item",
                    },
                },
                "predicates": [],
            }
        ],
        "selectedFields": ["selector", "name"],
        "requiredCapabilityRows": [
            "rust.capture.item",
        ],
        "regexPrograms": [],
    }


def test_resident_plan_preserves_pattern_local_structure() -> None:
    validator = schema_validator_for(ROOT / "schemas/resident-syntax-query-plan.v1.schema.json")
    validator.validate(resident_plan())


def test_resident_plan_wildcard_uses_explicit_true_structure() -> None:
    value = resident_plan()
    assert value["patterns"][0]["structure"]["kind"] == "true"
    validator = schema_validator_for(ROOT / "schemas/resident-syntax-query-plan.v1.schema.json")
    validator.validate(value)


def test_resident_plan_rejects_flat_grammarless_inventory() -> None:
    value = resident_plan()
    value["patterns"] = [
        {
            "index": 0,
            "captures": ["item"],
            "nodeTypes": ["function_item"],
            "fields": ["name"],
        }
    ]
    validator = schema_validator_for(ROOT / "schemas/resident-syntax-query-plan.v1.schema.json")
    with pytest.raises(ValidationError):
        validator.validate(value)


def test_resident_plan_binds_regex_program_shape_to_the_operator() -> None:
    validator = schema_validator_for(ROOT / "schemas/resident-syntax-query-plan.v1.schema.json")
    value = resident_plan()
    value["patterns"][0]["predicates"] = [
        {
            "kind": "scalar",
            "capture": "item",
            "factPath": "name",
            "operator": "match",
            "value": "^run$",
            "origin": {
                "kind": "asp-predicate",
                "capabilityRowId": "rust.fact.name",
            },
        }
    ]
    with pytest.raises(ValidationError):
        validator.validate(value)

    value["patterns"][0]["predicates"][0]["regexProgramId"] = "regex.1"
    validator.validate(value)
    value["patterns"][0]["predicates"][0]["operator"] = "eq"
    with pytest.raises(ValidationError):
        validator.validate(value)


def test_resident_plan_any_of_requires_a_nonempty_literal_set() -> None:
    validator = schema_validator_for(ROOT / "schemas/resident-syntax-query-plan.v1.schema.json")
    value = resident_plan()
    predicate = {
        "kind": "scalar",
        "capture": "item",
        "factPath": "name",
        "operator": "any-of",
        "value": "run",
        "origin": {
            "kind": "standard-predicate",
            "capabilityRowId": "rust.capture.item",
        },
    }
    value["patterns"][0]["predicates"] = [predicate]
    with pytest.raises(ValidationError):
        validator.validate(value)

    predicate["value"] = ["run", "build"]
    validator.validate(value)


def test_runtime_capability_row_requires_equivalence_evidence() -> None:
    value = {
        "schemaId": "agent.semantic-protocols.enhanced-tree-sitter-query-capability-table",
        "schemaVersion": "1",
        "languageId": "rust",
        "providerId": "asp-rust",
        "parserAbi": {"id": "asp-rust-native", "version": "1", "digest": DIGEST},
        "queryGrammar": {"id": "tree-sitter-rust", "version": "0.24", "digest": DIGEST},
        "operatorTableDigest": DIGEST,
        "tableDigest": DIGEST,
        "rows": [
            {
                "rowId": "rust.node.function-item",
                "kind": "node-type",
                "sourceName": "function_item",
                "publicationState": "runtime",
                "lowering": {
                    "residentFactPath": "kind",
                    "constraintKind": "scalar",
                    "residentValue": "function",
                },
                "equivalenceEvidence": {
                    "method": "native-parser-tree-sitter-differential-v1",
                    "corpusDigest": DIGEST,
                    "receiptDigest": DIGEST,
                },
            }
        ],
    }
    validator = schema_validator_for(
        ROOT / "schemas/enhanced-tree-sitter-query-capability-table.v1.schema.json"
    )
    validator.validate(value)
    del value["rows"][0]["equivalenceEvidence"]
    with pytest.raises(ValidationError):
        validator.validate(value)


def test_nonruntime_capability_row_cannot_claim_equivalence_receipt() -> None:
    value = {
        "schemaId": "agent.semantic-protocols.enhanced-tree-sitter-query-capability-table",
        "schemaVersion": "1",
        "languageId": "rust",
        "providerId": "asp-rust",
        "parserAbi": {"id": "asp-rust-native", "version": "1", "digest": DIGEST},
        "queryGrammar": {"id": "tree-sitter-rust", "version": "0.24", "digest": DIGEST},
        "operatorTableDigest": DIGEST,
        "tableDigest": DIGEST,
        "rows": [
            {
                "rowId": "rust.fact.control-flow",
                "kind": "fact-path",
                "sourceName": "control-flow",
                "publicationState": "provider-local",
                "equivalenceEvidence": {
                    "method": "native-parser-tree-sitter-differential-v1",
                    "corpusDigest": DIGEST,
                    "receiptDigest": DIGEST,
                },
            }
        ],
    }
    validator = schema_validator_for(
        ROOT / "schemas/enhanced-tree-sitter-query-capability-table.v1.schema.json"
    )
    with pytest.raises(ValidationError):
        validator.validate(value)


def test_rust_provider_capability_table_validates_against_shared_v1() -> None:
    value = json.loads(
        (
            ROOT
            / "languages/asp-rust/tree-sitter/tree-sitter-rust/enhanced-query-capabilities.v1.json"
        ).read_text(encoding="utf-8")
    )
    validator = schema_validator_for(
        ROOT / "schemas/enhanced-tree-sitter-query-capability-table.v1.schema.json"
    )
    validator.validate(value)
    assert value["schemaVersion"] == "1"
    assert any(
        row["rowId"] == "rust.node.macro-definition"
        and row["publicationState"] == "provider-local"
        for row in value["rows"]
    )


def test_mrr_operator_table_is_namespaced_and_closed() -> None:
    value = {
        "schemaId": "agent.semantic-protocols.enhanced-tree-sitter-query-operator-table",
        "schemaVersion": "1",
        "profileId": "mrr.enhanced-tree-sitter-query.v1",
        "owner": "mrr-gerbil-aot",
        "declarationDigest": DIGEST,
        "operators": [
            {
                "spelling": "#asp-eq?",
                "kind": "predicate",
                "minimumArity": 3,
                "maximumArity": 3,
                "operands": [
                    {"position": 0, "kind": "capture", "domain": "item-capture", "cardinality": "one"},
                    {"position": 1, "kind": "string", "domain": "scalar-fact-path", "cardinality": "one"},
                    {"position": 2, "kind": "string", "domain": "literal", "cardinality": "one"},
                ],
                "lowering": "scalar-eq",
                "failureCode": "enhanced-query-operand-invalid",
            }
        ],
        "recoveries": [
            {
                "site": "operator",
                "code": "enhanced-query-operator-unsupported",
                "strategy": "reject",
            }
        ],
    }
    validator = schema_validator_for(
        ROOT / "schemas/enhanced-tree-sitter-query-operator-table.v1.schema.json"
    )
    validator.validate(value)
    unnamespaced = copy.deepcopy(value)
    unnamespaced["operators"][0]["spelling"] = "#eq?"
    with pytest.raises(ValidationError):
        validator.validate(unnamespaced)
