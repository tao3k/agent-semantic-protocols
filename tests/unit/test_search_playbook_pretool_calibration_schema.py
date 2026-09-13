# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import copy
import json
from pathlib import Path

import jsonschema
import pytest


ROOT = Path(__file__).resolve().parents[2]
SCHEMA = json.loads(
    (ROOT / "schemas/search-playbook-pretool-calibration.v1.schema.json").read_text()
)


def calibration() -> dict[str, object]:
    rg = {
        "field": "rg",
        "blockIndex": 0,
        "optionTokenIndex": 4,
        "argvStartTokenIndex": 5,
        "argvEndTokenIndexExclusive": 12,
        "argv": ["-n", "-g", "*.rs", "--type", "rust", "needle", "crates"],
    }
    rg_analysis = {
        "blockIndex": 0,
        "admitted": True,
        "exactArgv": ["-n", "-g", "*.rs", "--type", "rust", "needle", "crates"],
        "outputAttribution": "path-line",
        "options": [
            {"option": "-n", "optionTokenIndex": 0},
            {
                "option": "-g",
                "optionTokenIndex": 1,
                "valueTokenIndex": 2,
                "value": "*.rs",
            },
            {
                "option": "--type",
                "optionTokenIndex": 3,
                "valueTokenIndex": 4,
                "value": "rust",
            },
        ],
        "patterns": [{"value": "needle", "tokenIndex": 5}],
        "searchRoots": [{"value": "crates", "tokenIndex": 6}],
        "diagnostics": [],
    }
    return {
        "schemaId": "agent.semantic-protocols.search-playbook-pretool-calibration",
        "schemaVersion": "1",
        "state": "rejected",
        "reasonKind": "search-playbook-pretool-calibration",
        "tokenIndexBasis": "search-playbook-argv",
        "layout": {
            "layoutId": "rg-tantivy-structural-scope",
            "shape": "intersect(rg,tantivy)->structural-scope-facts->graph?",
            "requiredInputSets": [["rg", "tantivy"]],
            "graphBarrierAfter": "structural-scope-facts",
        },
        "producers": [
            {
                "field": "languages",
                "value": "rust",
                "optionTokenIndex": 2,
                "valueTokenIndex": 3,
            }
        ],
        "inputOccurrences": [rg],
        "rgArgvBoundaries": [copy.deepcopy(rg)],
        "rgAnalyses": [rg_analysis],
        "tantivyAnalyses": [],
        "missingFields": ["tantivy"],
        "conflictingFields": [],
        "issues": [
            {
                "reasonKind": "search-playbook-request-incomplete",
                "field": "tantivy",
                "message": "the default Search Layout requires --rg and --tantivy together",
            }
        ],
    }


def test_calibration_preserves_rg_argv_and_fill_form_fields() -> None:
    jsonschema.Draft202012Validator(SCHEMA).validate(calibration())


def test_calibration_rejects_an_untyped_issue() -> None:
    candidate = calibration()
    candidate["issues"][0]["reasonKind"] = "Search Problem"  # type: ignore[index]
    with pytest.raises(jsonschema.ValidationError):
        jsonschema.Draft202012Validator(SCHEMA).validate(candidate)
