# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Validate the State Home runtime development authority schema."""

from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path
from typing import Callable


_REPO_ROOT = Path(__file__).resolve().parents[2]
_SCHEMA_VALIDATION_PATH = _REPO_ROOT / "tests" / "unit" / "schema_validation.py"
_SCHEMA_VALIDATION_SPEC = importlib.util.spec_from_file_location(
    "schema_validation", _SCHEMA_VALIDATION_PATH
)
assert _SCHEMA_VALIDATION_SPEC is not None
assert _SCHEMA_VALIDATION_SPEC.loader is not None
_SCHEMA_VALIDATION_MODULE = importlib.util.module_from_spec(_SCHEMA_VALIDATION_SPEC)
_SCHEMA_VALIDATION_SPEC.loader.exec_module(_SCHEMA_VALIDATION_MODULE)

schema_validator_for: Callable[[Path], object] = (
    _SCHEMA_VALIDATION_MODULE.schema_validator_for
)


class RuntimeDevConfigSchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        self.validator = schema_validator_for(
            _REPO_ROOT / "schemas" / "runtime-dev-config.v1.schema.json"
        )

    def validation_errors(self, config: dict[str, object]) -> list[str]:
        return [error.message for error in self.validator.iter_errors(config)]

    def test_enabled_absolute_root_is_the_canonical_dev_shape(self) -> None:
        self.assertEqual(
            [],
            self.validation_errors(
                {
                    "enabled": True,
                    "root": "/absolute/path/to/agent-semantic-protocols",
                }
            ),
        )

    def test_rejects_legacy_scope(self) -> None:
        self.assertTrue(
            self.validation_errors(
                {"enabled": True, "root": "/checkout", "scope": "checkout"}
            )
        )

    def test_disabled_table_is_a_valid_release_selection(self) -> None:
        self.assertEqual(
            [], self.validation_errors({"enabled": False, "root": "/checkout"})
        )

    def test_rejects_relative_dev_authority(self) -> None:
        self.assertTrue(
            self.validation_errors({"enabled": True, "root": "checkout"})
        )


class RuntimeServerArtifactCatalogSchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        schema = (
            _REPO_ROOT / "schemas" / "runtime-server-control.v1.schema.json"
        )
        self.validator = schema_validator_for(schema)

    @staticmethod
    def endpoint() -> dict[str, object]:
        return {
            "schemaId": "agent.semantic-protocols.runtime-server-endpoint",
            "schemaVersion": "1",
            "transportContractDigest": f"blake3-256:{'a' * 64}",
            "ownerEpoch": 1,
            "runtimeArtifactPath": "/runtime/bin/asp",
            "runtimeBinaryIdentity": "runtime-digest",
            "artifactMode": "dev",
            "artifactCatalogDigest": f"blake3-256:{'b' * 64}",
            "bindingToken": "binding",
            "controlEndpoint": {
                "transport": "loopback-tcp",
                "address": "127.0.0.1",
                "port": 41001,
            },
            "dataEndpoint": {
                "transport": "loopback-tcp",
                "address": "127.0.0.1",
                "port": 41002,
            },
            "providerEndpoint": {
                "transport": "loopback-tcp",
                "address": "127.0.0.1",
                "port": 41003,
            },
            "workspaceStorePath": "/state/runtime/workspaces",
            "statusMemoryPath": "/tmp/status.memory",
        }

    def validation_errors(self, value: dict[str, object]) -> list[str]:
        return [error.message for error in self.validator.iter_errors(value)]

    def test_endpoint_binds_dev_catalog_identity(self) -> None:
        self.assertEqual([], self.validation_errors(self.endpoint()))

    def test_endpoint_without_catalog_identity_is_rejected(self) -> None:
        endpoint = self.endpoint()
        endpoint.pop("artifactCatalogDigest")
        self.assertTrue(self.validation_errors(endpoint))

    def test_legacy_developer_mode_label_is_rejected(self) -> None:
        endpoint = self.endpoint()
        endpoint["artifactMode"] = "developer"
        self.assertTrue(self.validation_errors(endpoint))


if __name__ == "__main__":
    unittest.main()
