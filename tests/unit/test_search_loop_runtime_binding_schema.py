# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import copy
import json
from pathlib import Path

from jsonschema import Draft202012Validator
from referencing import Registry, Resource


ROOT = Path(__file__).resolve().parents[2]
SCHEMA_DIR = ROOT / "schemas"
FIXTURE_DIR = SCHEMA_DIR / "fixtures" / "search-interactive-loop"
RUNTIME_SCHEMA_PATH = SCHEMA_DIR / "search-loop-runtime-binding.v1.schema.json"
INTERACTIVE_SCHEMA_PATH = SCHEMA_DIR / "search-interactive-loop.v1.schema.json"
COMMAND_SCHEMA_PATH = SCHEMA_DIR / "semantic-command.v1.schema.json"
CURSOR_SCHEMA_PATH = SCHEMA_DIR / "search-graph-cursor.v1.schema.json"


def load_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def schema_registry() -> Registry:
    registry = Registry()
    for path in SCHEMA_DIR.glob("*.schema.json"):
        schema = load_json(path)
        schema_id = schema.get("$id")
        if schema_id is not None:
            registry = registry.with_resource(
                schema_id,
                Resource.from_contents(schema),
            )
    return registry


def runtime_validator() -> Draft202012Validator:
    return Draft202012Validator(
        load_json(RUNTIME_SCHEMA_PATH),
        registry=schema_registry(),
    )


def test_open_envelope_is_valid() -> None:
    runtime_validator().validate(load_json(FIXTURE_DIR / "open-envelope.v1.json"))


def test_active_runtime_binding_is_valid() -> None:
    runtime_validator().validate(load_json(FIXTURE_DIR / "runtime-active.v1.json"))


def test_terminal_runtime_binding_is_valid() -> None:
    runtime_validator().validate(load_json(FIXTURE_DIR / "runtime-terminal.v1.json"))


def test_runtime_binding_rejects_raw_bearer_token() -> None:
    errors = list(
        runtime_validator().iter_errors(
            load_json(FIXTURE_DIR / "invalid-runtime-with-raw-token.v1.json")
        )
    )
    assert errors


def test_terminal_runtime_binding_rejects_active_panel() -> None:
    errors = list(
        runtime_validator().iter_errors(
            load_json(FIXTURE_DIR / "invalid-terminal-with-active-panel.v1.json")
        )
    )
    assert errors


def test_interactive_projection_rejects_legacy_opaque_token_shape() -> None:
    projection = load_json(FIXTURE_DIR / "running-parallel.v1.json")
    projection["pollAction"]["pollToken"] = (
        "capability.poll.parallel-1.abcdefghijklmnopqrstuvwxyz012345"
    )
    validator = Draft202012Validator(
        load_json(INTERACTIVE_SCHEMA_PATH),
        registry=schema_registry(),
    )
    assert list(validator.iter_errors(projection))


def test_shared_command_token_shape_matches_runtime_issuer() -> None:
    schema = load_json(COMMAND_SCHEMA_PATH)
    token_validator = Draft202012Validator(schema["$defs"]["opaqueToken"])
    canonical = "capability." + ("a" * 64)
    token_validator.validate(canonical)

    for invalid in (
        "capability." + ("A" * 64),
        "capability." + ("a" * 63),
        "capability.advance." + ("a" * 64),
    ):
        assert list(token_validator.iter_errors(invalid))


def test_runtime_binding_rejects_unrecognized_artifact_fields() -> None:
    binding = copy.deepcopy(load_json(FIXTURE_DIR / "runtime-active.v1.json"))
    binding["activePanel"]["proposalSetArtifact"]["payload"] = {"raw": True}
    assert list(runtime_validator().iter_errors(binding))


def test_runtime_binding_requires_cursor_artifact_for_active_panel() -> None:
    binding = copy.deepcopy(load_json(FIXTURE_DIR / "runtime-active.v1.json"))
    del binding["activePanel"]["graphCursorArtifact"]
    assert list(runtime_validator().iter_errors(binding))


def test_open_envelope_requires_cursor_artifact() -> None:
    envelope = copy.deepcopy(load_json(FIXTURE_DIR / "open-envelope.v1.json"))
    del envelope["graphCursorArtifact"]
    assert list(runtime_validator().iter_errors(envelope))


def test_runtime_binding_rejects_non_cursor_artifact_schema() -> None:
    binding = copy.deepcopy(load_json(FIXTURE_DIR / "runtime-active.v1.json"))
    binding["activePanel"]["graphCursorArtifact"]["artifactSchemaId"] = (
        "agent.semantic-protocols.search-choice-panel"
    )
    assert list(runtime_validator().iter_errors(binding))


def test_graph_cursor_artifact_is_valid() -> None:
    validator = Draft202012Validator(
        load_json(CURSOR_SCHEMA_PATH),
        registry=schema_registry(),
    )
    validator.validate(load_json(FIXTURE_DIR / "graph-cursor.v1.json"))


def test_graph_cursor_schema_is_valid_draft_2020_12() -> None:
    Draft202012Validator.check_schema(load_json(CURSOR_SCHEMA_PATH))
