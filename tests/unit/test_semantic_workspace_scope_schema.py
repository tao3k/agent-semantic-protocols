import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[2]
SCHEMA = json.loads(
    (ROOT / "schemas/semantic-workspace-scope.v1.schema.json").read_text()
)


def rust_scope():
    return {
        "schemaId": "agent.semantic-protocols.semantic-workspace-scope",
        "schemaVersion": "1",
        "workspaceId": "cargo:agent-semantic-protocols",
        "languageId": "rust",
        "providerId": "rs-harness",
        "packageManager": "cargo",
        "sourceExtensions": [".rs"],
        "discoveryRoot": ".",
        "anchors": [{"kind": "cargo-manifest", "path": "Cargo.toml", "sha256": "sha256:" + "a" * 64}],
        "packages": [{"packageId": "cargo:agent-semantic-search", "name": "agent-semantic-search", "root": "crates/agent-semantic-search", "manifestPath": "crates/agent-semantic-search/Cargo.toml", "languageId": "rust"}],
        "admittedRoots": ["crates/agent-semantic-search"],
        "fingerprint": "sha256:" + "b" * 64,
    }


class SemanticWorkspaceScopeSchemaTests(unittest.TestCase):
    def test_python_provider_schema_copy_matches_shared_contract(self):
        provider_schema = json.loads(
            (
                ROOT
                / "languages/python-lang-project-harness/schemas/semantic-workspace-scope.v1.schema.json"
            ).read_text()
        )
        self.assertEqual(provider_schema, SCHEMA)

    def test_rust_provider_schema_copy_matches_shared_contract(self):
        provider_schema = json.loads(
            (
                ROOT
                / "languages/rust-lang-project-harness/schemas/semantic-workspace-scope.v1.schema.json"
            ).read_text()
        )
        self.assertEqual(provider_schema, SCHEMA)

    def test_accepts_provider_resolved_workspace_scope(self):
        Draft202012Validator(SCHEMA).validate(rust_scope())

    def test_accepts_member_outside_discovery_root(self):
        value = rust_scope()
        value["discoveryRoot"] = "tools/cli"
        value["packages"][0]["root"] = "../shared/search-core"
        value["packages"][0]["manifestPath"] = "../shared/search-core/Cargo.toml"
        value["admittedRoots"] = ["../shared/search-core"]
        Draft202012Validator(SCHEMA).validate(value)

    def test_accepts_provider_owned_language_and_anchor_kinds(self):
        value = rust_scope()
        value["languageId"] = "future-lang"
        value["packageManager"] = "future-pm"
        value["anchors"][0]["kind"] = "future-workspace-anchor"
        value["packages"][0]["languageId"] = "future-lang"
        Draft202012Validator(SCHEMA).validate(value)

    def test_rejects_scope_without_admitted_package_roots(self):
        value = rust_scope()
        value["admittedRoots"] = []
        self.assertTrue(list(Draft202012Validator(SCHEMA).iter_errors(value)))

    def test_rejects_scope_without_provider_source_extensions(self):
        value = rust_scope()
        value["sourceExtensions"] = []
        self.assertTrue(list(Draft202012Validator(SCHEMA).iter_errors(value)))


if __name__ == "__main__":
    unittest.main()
