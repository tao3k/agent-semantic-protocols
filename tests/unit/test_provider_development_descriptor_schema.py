# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import json
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator, ValidationError

from unit.schema_validation import schema_validator_for


ROOT = Path(__file__).resolve().parents[2]
SCHEMA_PATH = ROOT / "schemas" / "provider-manifest.schema.json"
def load_json(path: Path) -> dict[str, object]:
    return json.loads(path.read_text(encoding="utf-8"))


def development_validator() -> Draft202012Validator:
    schema = load_json(SCHEMA_PATH)
    return Draft202012Validator(schema["$defs"]["providerDevelopmentDescriptor"])


def test_all_install_registered_providers_have_valid_workspace_install_authority() -> None:
    register = load_json(ROOT / "schemas/provider-install-register.json")
    schema_validator_for(
        ROOT / "schemas/provider-install-register.schema.json"
    ).validate(register)
    install_validator = schema_validator_for(
        ROOT / "schemas/provider-workspace-install.schema.json"
    )
    for provider in register["providers"]:
        install_path = (
            ROOT / provider["sourceRoot"] / provider["workspaceInstall"]
        )
        install = load_json(install_path)
        install_validator.validate(install)
        assert install["languageId"] == provider["languageId"]
        assert install["providerId"] == provider["providerId"]


@pytest.mark.parametrize(
    "source_root",
    (
        "/absolute/provider",
        "../provider",
        "languages/../provider",
        "languages/provider/..",
    ),
)
def test_development_source_root_rejects_absolute_and_parent_escape(
    source_root: str,
) -> None:
    descriptor = {
        "schemaId": "agent.semantic-protocols.provider-development-descriptor",
        "schemaVersion": "1",
        "sourceRoot": source_root,
        "buildBinding": "root-development-installer-v1",
        "artifactDomain": "checkout",
    }
    with pytest.raises(ValidationError):
        development_validator().validate(descriptor)


def test_development_descriptor_rejects_unknown_binding_and_domain() -> None:
    descriptor = {
        "schemaId": "agent.semantic-protocols.provider-development-descriptor",
        "schemaVersion": "1",
        "sourceRoot": "languages/provider",
        "buildBinding": "legacy-installer",
        "artifactDomain": "path-fallback",
    }
    with pytest.raises(ValidationError):
        development_validator().validate(descriptor)
