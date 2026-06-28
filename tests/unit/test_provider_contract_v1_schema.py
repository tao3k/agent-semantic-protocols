from __future__ import annotations

from copy import deepcopy
import json
from pathlib import Path
from typing import Any

import pytest
from jsonschema import Draft202012Validator
from jsonschema.exceptions import ValidationError


SCHEMA_PATH = (
    Path(__file__).resolve().parents[2]
    / "schemas/semantic-provider-contract.v1.schema.json"
)


def _validator() -> Draft202012Validator:
    return Draft202012Validator(json.loads(SCHEMA_PATH.read_text()))


def _python_provider_contract() -> dict[str, Any]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-provider-contract",
        "schemaVersion": "1",
        "contractId": "python.provider-contract.v1",
        "languageId": "python",
        "providerId": "python-lang-project-harness",
        "responsibilities": {
            "allowed": [
                "native-parser-facts",
                "bounded-owner-facts",
                "exact-projection",
                "batch-exact-projection",
                "packet-out",
            ],
            "forbidden": [
                "workspace-source-index",
                "broad-workspace-search",
                "owner-ranking",
                "graph-ranking",
                "prompt-graph-rendering",
                "dependency-cache",
                "source-content-cache",
                "route-engine",
                "cache-invalidation",
            ],
        },
        "commands": {
            "doctor": {
                "argv": ["asp", "python", "agent", "doctor", "--json"],
                "input": "workspace",
                "output": "doctor",
                "providerProcessPolicy": "cold-or-refresh-only",
            },
            "indexExport": {
                "argv": ["asp", "python", "index", "export", "--json"],
                "input": "workspace",
                "output": "bounded-json-facts",
                "providerProcessPolicy": "cold-or-refresh-only",
            },
            "ownerItems": {
                "argv": ["asp", "python", "search", "owner", "<owner>", "items"],
                "input": "owner",
                "output": "owner-local-items",
                "providerProcessPolicy": "cold-or-refresh-only",
            },
            "queryProjection": {
                "argv": ["asp", "python", "query", "--selector", "<selector>"],
                "input": "exact-selector",
                "output": "projection",
                "providerProcessPolicy": "exact-projection-only",
            },
            "queryCode": {
                "argv": ["asp", "python", "query", "--selector", "<selector>", "--code"],
                "input": "exact-selector",
                "output": "code",
                "providerProcessPolicy": "exact-projection-only",
            },
            "queryBatch": {
                "argv": ["asp", "python", "query", "--selector-batch", "-"],
                "input": "exact-selector-batch",
                "output": "projection",
                "providerProcessPolicy": "batch-exact-projection-only",
            },
        },
        "projectionPolicy": {
            "codeRequiresExactSelector": True,
            "sourceLocatorExecutable": False,
            "sourceLocatorRole": "hint-only",
            "exactIdentityKinds": ["structural-selector", "native-fact-id"],
            "forbiddenIdentityKinds": [
                "source-locator",
                "display-line-range",
                "owner-path-only",
            ],
        },
        "hotPathBoundary": {
            "warmBroadSearchProviderProcessCount": 0,
            "rustOwned": [
                "source-index",
                "fact-index",
                "dependency-index",
                "route-engine",
                "materializer",
                "render",
                "receipt",
                "cache",
            ],
            "providerOwned": [
                "native-parser-facts",
                "bounded-owner-facts",
                "exact-projection",
                "batch-exact-projection",
                "packet-out",
            ],
            "graphTurboOwned": [
                "graph-ranking",
                "ppr",
                "topology-ranking",
                "ablation",
            ],
        },
    }


def test_provider_contract_v1_accepts_owner_local_projection_boundary() -> None:
    validator = _validator()

    validator.validate(_python_provider_contract())


def test_provider_contract_v1_rejects_executable_source_locator() -> None:
    validator = _validator()
    contract = _python_provider_contract()
    contract["projectionPolicy"]["sourceLocatorExecutable"] = True

    with pytest.raises(ValidationError):
        validator.validate(contract)


def test_provider_contract_v1_rejects_provider_owned_broad_search() -> None:
    validator = _validator()
    contract = _python_provider_contract()
    contract["responsibilities"]["allowed"].append("broad-workspace-search")

    with pytest.raises(ValidationError):
        validator.validate(contract)


def test_provider_contract_v1_requires_complete_forbidden_surface() -> None:
    validator = _validator()
    contract = deepcopy(_python_provider_contract())
    contract["responsibilities"]["forbidden"].remove("route-engine")

    with pytest.raises(ValidationError):
        validator.validate(contract)


def test_provider_contract_v1_is_the_only_provider_contract_surface() -> None:
    repo_root = SCHEMA_PATH.parents[1]
    legacy_version = "v" + "2"
    forbidden_paths = [
        repo_root / "schemas" / f"semantic-provider-contract.{legacy_version}.schema.json",
        repo_root / "tests/unit" / f"test_provider_contract_{legacy_version}_schema.py",
    ]
    assert [str(path) for path in forbidden_paths if path.exists()] == []

    forbidden_terms = (
        f"semantic-provider-contract.{legacy_version}",
        f"provider-contract.{legacy_version}",
        f"Provider Contract {legacy_version}",
        f"ASP Hot Path {legacy_version}",
        f"ActionRoute {legacy_version}",
    )
    scan_roots = (
        repo_root / "schemas",
        repo_root / "tests/unit",
        repo_root / "docs/10-19-rfcs",
    )
    offenders: list[str] = []
    for root in scan_roots:
        for path in root.rglob("*"):
            if path.suffix not in {".json", ".py", ".org"}:
                continue
            text = path.read_text(encoding="utf-8")
            matched = [term for term in forbidden_terms if term in text]
            if matched:
                offenders.append(f"{path.relative_to(repo_root)}: {', '.join(matched)}")

    assert offenders == []


def test_python_harness_has_no_direct_source_read_compatibility_surface() -> None:
    repo_root = SCHEMA_PATH.parents[1]
    forbidden_terms = (
        "_semantic_search_direct_read_render",
        "SEMANTIC_READ_PACKET_SCHEMA_ID",
        "query/direct-source-read",
    )
    scan_roots = (
        repo_root / "languages/python-lang-project-harness/src",
        repo_root / "languages/python-lang-project-harness/tests/unit/harness",
    )
    offenders: list[str] = []
    for root in scan_roots:
        for path in root.rglob("*"):
            if path.suffix != ".py":
                continue
            text = path.read_text(encoding="utf-8")
            matched = [term for term in forbidden_terms if term in text]
            if matched:
                offenders.append(f"{path.relative_to(repo_root)}: {', '.join(matched)}")

    assert offenders == []


def test_rust_command_layer_has_no_inline_python_owner_items_surface() -> None:
    repo_root = SCHEMA_PATH.parents[1]
    forbidden_terms = (
        "search_pipe_python_owner_items",
        "run_inline_python_owner_items_query",
        "render_inline_python_owner_items",
        "try_inline_python",
        "inline_python_owner_items",
        "owner-items-rust-inline-python",
        "rust-inline-python-owner-items",
    )
    scan_root = repo_root / "crates/agent-semantic-protocol/src/command"
    offenders: list[str] = []
    for path in scan_root.rglob("*.rs"):
        text = path.read_text(encoding="utf-8")
        matched = [term for term in forbidden_terms if term in text]
        if matched:
            offenders.append(f"{path.relative_to(repo_root)}: {', '.join(matched)}")

    assert offenders == []
