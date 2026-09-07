# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Agent method registry schema tests."""

from .support import (
    language_descriptor_errors,
    language_registry_errors,
    registry_with_descriptor,
)


def test_agent_compact_method_can_omit_output_schema_when_no_json() -> None:
    errors = language_descriptor_errors(
        {
            "method": "agent/guide",
            "command": "agent",
            "supportsJson": False,
            "supportsCompact": True,
            "clients": ["codex"],
            "requiredOptions": ["--client codex"],
        }
    )

    assert errors == []


def test_language_registration_accepts_provider_command_prefix() -> None:
    registry = registry_with_descriptor(
        {
            "method": "agent/guide",
            "command": "agent",
            "supportsJson": False,
            "supportsCompact": True,
        }
    )
    language = registry["languages"][0]
    language["languageId"] = "julia"
    language["providerId"] = "asp-julia"
    language["binary"] = "asp-julia"
    language["providerCommandPrefix"] = [
        "julia",
        "--project=languages/AspJulia.jl",
        "languages/AspJulia.jl/bin/asp-julia.jl",
    ]
    language["namespace"] = "agent.semantic-protocols.languages.julia.asp-julia"

    assert language_registry_errors(registry) == []


def test_agent_json_method_requires_output_schema_ids() -> None:
    errors = language_descriptor_errors(
        {
            "method": "agent/hook",
            "command": "agent",
            "supportsJson": True,
            "supportsCompact": False,
            "clients": ["codex"],
        }
    )

    assert "'outputSchemaIds' is a required property" in errors
