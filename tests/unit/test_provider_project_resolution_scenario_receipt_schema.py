# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import json
from pathlib import Path

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[2]
SCHEMA = json.loads(
    (
        ROOT
        / "schemas"
        / "provider-project-resolution-scenario-receipt.v1.schema.json"
    ).read_text()
)
VALIDATOR = Draft202012Validator(SCHEMA)


def test_package_manager_scenario_receipt_is_provider_owned_and_scan_free() -> None:
    VALIDATOR.validate(
        {
            "schemaId": "agent.semantic-protocols.provider-project-resolution-scenario-receipt",
            "schemaVersion": "1",
            "languageId": "rust",
            "providerId": "asp-rust",
            "packageManager": "cargo",
            "scenarioId": "workspace-target-manifest-delta",
            "state": "passed",
            "dimensions": [
                "workspace-membership",
                "explicit-targets",
                "path-dependencies",
                "manifest-delta",
            ],
            "projectResolutionDigest": "blake3:resolution",
            "metrics": {
                "wallMicros": 900,
                "fullWorkspaceReads": 0,
                "databaseOpens": 0,
            },
        }
    )


def test_scenario_receipt_rejects_local_scan_or_database_fallback() -> None:
    packet = {
        "schemaId": "agent.semantic-protocols.provider-project-resolution-scenario-receipt",
        "schemaVersion": "1",
        "languageId": "typescript",
        "providerId": "asp-typescript",
        "packageManager": "npm",
        "scenarioId": "workspace-members",
        "state": "passed",
        "dimensions": ["workspace-membership"],
        "projectResolutionDigest": "blake3:resolution",
        "metrics": {
            "wallMicros": 800,
            "fullWorkspaceReads": 1,
            "databaseOpens": 0,
        },
    }
    assert list(VALIDATOR.iter_errors(packet))
