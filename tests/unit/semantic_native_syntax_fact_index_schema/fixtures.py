"""Fixtures for native syntax fact index schema tests."""

from __future__ import annotations


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
            },
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
