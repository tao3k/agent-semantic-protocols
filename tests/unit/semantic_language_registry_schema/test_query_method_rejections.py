# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Query method registry rejection tests."""

from jsonschema import Draft202012Validator

from .support import (
    language_registry_errors,
    language_registry_schema_validator,
    registry_with_descriptor,
)


def test_evidence_method_is_not_a_language_provider_command() -> None:
    schema = language_registry_schema_validator().schema
    method_validator = Draft202012Validator(schema["$defs"]["method"])
    command_validator = Draft202012Validator(schema["$defs"]["command"])

    assert list(method_validator.iter_errors("evidence/assurance"))
    assert list(command_validator.iter_errors("evidence"))


def test_policy_check_is_not_a_provider_command() -> None:
    registry = registry_with_descriptor(
        {
            "method": "check/changed",
            "command": "check",
            "input": "workspace",
        }
    )

    errors = language_registry_errors(registry)
    assert any("check/changed" in error for error in errors)
    assert any("'check' is not one of" in error for error in errors)


def test_verification_is_not_a_provider_command() -> None:
    registry = registry_with_descriptor(
        {
            "method": "verification/run",
            "command": "verification",
            "input": "workspace",
        }
    )

    errors = language_registry_errors(registry)
    assert any("verification/run" in error for error in errors)
    assert any("'verification' is not one of" in error for error in errors)

