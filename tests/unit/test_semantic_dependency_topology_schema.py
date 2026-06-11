from __future__ import annotations

from pathlib import Path

from .schema_validation import schema_validator_for


_ROOT = Path(__file__).resolve().parents[2]
_SCHEMA_PATH = _ROOT / "schemas" / "semantic-dependency-topology.v1.schema.json"


def test_semantic_dependency_topology_schema_accepts_manifest_first_packet() -> None:
    packet = {
        "schemaId": "agent.semantic-protocols.semantic-dependency-topology",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "packetKind": "dependency-topology",
        "languageId": "python",
        "projectRoot": "/tmp/example",
        "fingerprint": "sha256:" + "1" * 64,
        "generatedAt": "2026-06-10T00:00:00Z",
        "cacheKey": {
            "languageId": "python",
            "packageManager": "uv",
            "manifestHash": "sha256:" + "2" * 64,
            "lockfileHash": "sha256:" + "3" * 64,
            "projectPackageName": "example",
        },
        "sources": {
            "manifests": [{"path": "pyproject.toml", "sha256": "sha256:" + "4" * 64}],
            "lockfiles": [{"path": "uv.lock", "sha256": "sha256:" + "5" * 64}],
            "usageSites": ["src/app.py"],
        },
        "graph": {
            "nodes": [
                {
                    "id": "package:example",
                    "kind": "package",
                    "role": "workspace-package",
                    "value": "example",
                    "action": "package",
                },
                {
                    "id": "dependency:jsonschema",
                    "kind": "dependency",
                    "role": "package",
                    "value": "jsonschema",
                    "action": "deps",
                },
                {
                    "id": "dependency-version:jsonschema@4.23.0",
                    "kind": "dependency-version",
                    "role": "version",
                    "value": "jsonschema@4.23.0",
                    "action": "evidence",
                },
                {
                    "id": "import:src~app.py:1:jsonschema:jsonschema",
                    "kind": "import-site",
                    "role": "import",
                    "value": "jsonschema",
                    "action": "code",
                    "path": "src/app.py",
                    "startLine": 1,
                    "endLine": 1,
                    "locator": "src/app.py:1:1",
                },
            ],
            "edges": [
                {
                    "source": "package:example",
                    "target": "dependency:jsonschema",
                    "relation": "depends_on",
                },
                {
                    "source": "dependency:jsonschema",
                    "target": "dependency-version:jsonschema@4.23.0",
                    "relation": "version_locked",
                },
                {
                    "source": "import:src~app.py:1:jsonschema:jsonschema",
                    "target": "dependency:jsonschema",
                    "relation": "imports",
                },
            ],
        },
    }

    validator = schema_validator_for(_SCHEMA_PATH)
    errors = sorted(validator.iter_errors(packet), key=lambda error: list(error.path))

    assert not errors, [f"{list(error.path)}: {error.message}" for error in errors]
