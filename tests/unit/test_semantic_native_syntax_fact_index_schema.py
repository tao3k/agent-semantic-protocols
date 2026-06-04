"""Validate the shared Native Syntax Fact Index schema contract."""

import copy
import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator
from referencing import Registry, Resource


_REPO_ROOT = Path(__file__).resolve().parents[2]


def native_syntax_fact() -> dict[str, object]:
    return {
        "id": "rust:src/lib.rs:4:reexport:rules",
        "kind": "reexport",
        "source": "native-parser",
        "languageKind": "use",
        "name": "rules",
        "qualifiedName": "crate::rules",
        "ownerPath": "src/lib.rs",
        "location": {"path": "src/lib.rs", "lineRange": "4:4"},
        "visibility": "public",
        "exported": True,
        "queryKeys": ["pub use rules", "rules", "crate::rules"],
        "relations": [{"kind": "reexports", "target": "crate::rules"}],
        "fields": {"rustVisibility": "Public", "segments": ["crate", "rules"]},
    }


def native_syntax_index() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-native-syntax-fact-index",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "languageId": "rust",
        "providerId": "rs-harness",
        "projectRoot": ".",
        "packageName": "rust-lang-project-harness",
        "scope": "query",
        "query": "pub use rules",
        "queryIntent": {
            "kind": "syntax.import",
            "term": "rules",
            "languageKind": "use",
            "fields": {"visibility": "public"},
        },
        "facts": [native_syntax_fact()],
        "indexes": [
            {
                "name": "imports",
                "factKinds": ["import", "reexport"],
                "queryKeys": ["name", "qualifiedName", "segments"],
            }
        ],
        "notes": [],
    }


def julia_native_syntax_index() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-native-syntax-fact-index",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "languageId": "julia",
        "providerId": "julia-lang-project-harness",
        "projectRoot": "/workspace/Example",
        "packageName": "Example",
        "scope": "workspace",
        "facts": [
            {
                "id": "julia:src/Example.jl:4:function:run",
                "kind": "function",
                "source": "native-parser",
                "languageKind": "function",
                "name": "run",
                "ownerPath": "src/Example.jl",
                "location": {"path": "src/Example.jl", "lineRange": "4:4"},
                "visibility": "public",
                "exported": True,
                "test": False,
                "queryKeys": ["run", "function", "method"],
                "relations": [
                    {
                        "kind": "related",
                        "target": "owner:src/Example.jl",
                        "fields": {"role": "owner"},
                    },
                    {"kind": "calls", "target": "helper"},
                ],
                "fields": {
                    "juliaKind": "function",
                    "detail": "function run(value)",
                    "searchText": "function run(value)",
                    "tags": ["method", "function", "public"],
                    "column": 1,
                },
            },
            {
                "id": "julia:src/Example.jl:4:13:argument:value",
                "kind": "argument",
                "source": "native-parser",
                "languageKind": "argument",
                "name": "value",
                "qualifiedName": "src/Example.jl::value",
                "ownerPath": "src/Example.jl",
                "location": {"path": "src/Example.jl", "lineRange": "4:4"},
                "visibility": "unknown",
                "exported": False,
                "test": False,
                "queryKeys": ["value", "argument", "src/Example.jl"],
            },
            {
                "id": "julia:src/Example.jl:2:1:include:impl.jl",
                "kind": "include",
                "source": "native-parser",
                "languageKind": "include",
                "name": "impl.jl",
                "qualifiedName": "src/Example.jl::impl.jl",
                "ownerPath": "src/Example.jl",
                "location": {"path": "src/Example.jl", "lineRange": "2:2"},
                "queryKeys": ["impl.jl", "include", "src/Example.jl"],
                "relations": [{"kind": "references", "target": "impl.jl"}],
            }
        ],
        "indexes": [
            {
                "name": "julia",
                "factKinds": ["argument", "function", "include"],
                "queryKeys": ["name", "languageKind", "tags", "searchText"],
                "fields": {"authority": "JuliaSyntax"},
            }
        ],
        "notes": [
            {
                "kind": "julia-syntax-authority",
                "message": "Facts are derived from JuliaSyntax-backed provider entries.",
            }
        ],
    }


def search_packet_with_native_syntax_fact() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-search-packet",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "languageId": "rust",
        "providerId": "rs-harness",
        "binary": "rs-harness",
        "namespace": "agent.semantic-protocols.languages.rust.rs-harness",
        "method": "search/query",
        "projectRoot": ".",
        "view": "query",
        "renderMode": "seeds",
        "query": "pub use rules",
        "header": {"kind": "search-query", "fields": {"intent": "syntax.import"}},
        "nodes": [],
        "edges": [],
        "owners": [],
        "items": [],
        "nativeSyntaxFacts": [native_syntax_fact()],
        "hits": [],
        "findings": [],
        "nextActions": [{"kind": "owner", "target": "src/lib.rs"}],
        "notes": [],
        "searchSynthesis": {
            "algorithm": "native-syntax-query",
            "scope": "query",
            "summary": "parser-owned code-shaped query",
        },
    }


class SemanticNativeSyntaxFactIndexSchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        schema_dir = _REPO_ROOT / "schemas"
        with (schema_dir / "semantic-native-syntax-fact-index.v1.schema.json").open(
            "r", encoding="utf-8"
        ) as handle:
            self.native_schema = json.load(handle)
        with (schema_dir / "semantic-search-packet.v1.schema.json").open(
            "r", encoding="utf-8"
        ) as handle:
            self.search_schema = json.load(handle)
        registry = Registry().with_resources(
            [
                (self.native_schema["$id"], Resource.from_contents(self.native_schema)),
                (self.search_schema["$id"], Resource.from_contents(self.search_schema)),
            ]
        )
        self.index_validator = Draft202012Validator(
            self.native_schema, registry=registry
        )
        self.fact_validator = Draft202012Validator(
            {"$ref": f"{self.native_schema['$id']}#/$defs/nativeSyntaxFact"},
            registry=registry,
        )
        self.search_validator = Draft202012Validator(
            self.search_schema, registry=registry
        )

    def index_errors(self, payload: dict[str, object]) -> list[str]:
        return [error.message for error in self.index_validator.iter_errors(payload)]

    def fact_errors(self, payload: dict[str, object]) -> list[str]:
        return [error.message for error in self.fact_validator.iter_errors(payload)]

    def search_errors(self, payload: dict[str, object]) -> list[str]:
        return [error.message for error in self.search_validator.iter_errors(payload)]

    def test_native_syntax_index_accepts_parser_owned_reexport_fact(self) -> None:
        self.assertEqual([], self.index_errors(native_syntax_index()))

    def test_native_syntax_index_accepts_julia_provider_fact(self) -> None:
        self.assertEqual([], self.index_errors(julia_native_syntax_index()))

    def test_native_syntax_fact_rejects_rank_prefixed_owner_path(self) -> None:
        payload = copy.deepcopy(native_syntax_fact())
        payload["ownerPath"] = "1:src/lib.rs"
        self.assertIn(
            "'1:src/lib.rs' does not match",
            "\n".join(self.fact_errors(payload)),
        )

    def test_search_packet_accepts_native_syntax_facts_for_query_view(self) -> None:
        self.assertEqual(
            [],
            self.search_errors(search_packet_with_native_syntax_fact()),
        )


if __name__ == "__main__":
    unittest.main()
