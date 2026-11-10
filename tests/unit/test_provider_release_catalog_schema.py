# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import json
import tomllib
from pathlib import Path

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[2]


def test_provider_release_catalog_is_v1_schema_valid_and_source_free() -> None:
    schema = json.loads(
        (ROOT / "schemas/provider-release-catalog.v1.schema.json").read_text()
    )
    catalog = tomllib.loads(
        (ROOT / "crates/agent-semantic-provider-protocol/provider-release-catalog.v1.toml").read_text()
    )
    Draft202012Validator.check_schema(schema)
    Draft202012Validator(schema).validate(catalog)

    assert catalog["schemaId"] == "agent.semantic-protocols.provider-release-catalog"
    assert catalog["schemaVersion"] == "1"
    assert set(catalog["releases"]) == {
        "rust",
        "typescript",
        "python",
        "julia",
        "gerbil-scheme",
    }
    serialized = json.dumps(catalog, sort_keys=True)
    for release in catalog["releases"].values():
        assert set(release["sha256ByTarget"]) == set(release["supportedTargets"])
        assert all(
            len(digest) == 64 and digest == digest.lower()
            for digest in release["sha256ByTarget"].values()
        )
    for development_only in (
        "sourceRoot",
        "workspaceInstall",
        "buildBinding",
        "artifactDomain",
        "checkoutRoot",
    ):
        assert development_only not in serialized
