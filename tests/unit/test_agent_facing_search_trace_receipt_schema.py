# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import json
from pathlib import Path

import jsonschema


ROOT = Path(__file__).resolve().parents[2]
SCHEMA = ROOT / "schemas" / "agent-facing-search-trace-receipt.v1.schema.json"
FIXTURES = ROOT / "schemas" / "fixtures" / "agent-facing-search-trace-receipt"


def load(path: Path) -> object:
    return json.loads(path.read_text(encoding="utf-8"))


def test_merged_tree_sitter_incremental_trace_is_valid() -> None:
    jsonschema.Draft202012Validator(load(SCHEMA)).validate(
        load(FIXTURES / "valid-merged-tree-sitter-incremental.v1.json")
    )


def test_stacked_terminal_emission_is_rejected() -> None:
    validator = jsonschema.Draft202012Validator(load(SCHEMA))
    errors = list(
        validator.iter_errors(load(FIXTURES / "invalid-stacked-terminal.v1.json"))
    )
    assert errors


def test_sibling_incremental_trace_is_rejected() -> None:
    validator = jsonschema.Draft202012Validator(load(SCHEMA))
    errors = list(
        validator.iter_errors(
            load(FIXTURES / "invalid-sibling-incremental-trace.v1.json")
        )
    )
    assert errors
