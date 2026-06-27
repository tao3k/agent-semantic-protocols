from __future__ import annotations

import json
import re
from pathlib import Path
from typing import Any

import pytest
from jsonschema import Draft202012Validator
from referencing import Registry, Resource

from tools.semantic_sandtable.trace_receipts import (
    TraceReceiptConfig,
    build_receipt_from_trace_path,
)


REPO_ROOT = Path(__file__).resolve().parents[3]
SCHEMA_DIR = REPO_ROOT / "schemas"
ABSOLUTE_PATH_RE = re.compile(r"(^|[\"'\s])(?:/Users/|/home/|/tmp/|[A-Za-z]:\\)")


@pytest.mark.parametrize(
    ("language", "owner_path"),
    [
        ("rust", "src/lib.rs"),
        ("python", "src/package/module.py"),
        ("typescript", "src/index.ts"),
    ],
)
def test_route_verification_accepts_owner_query_without_prime_for_languages(
    tmp_path: Path,
    language: str,
    owner_path: str,
) -> None:
    trace_path = tmp_path / "trace.jsonl"
    trace_path.write_text(
        "\n".join(
            [
                json.dumps(_owner_event(language, owner_path)),
                json.dumps(_query_code_event(language, owner_path)),
            ]
        )
        + "\n",
        encoding="utf-8",
    )

    receipt = build_receipt_from_trace_path(
        trace_path,
        config=TraceReceiptConfig(
            scenario_id=f"route.{language}-known-owner-skips-prime",
            language=language,
            project_name="fixture",
            project_source="fixture",
            intent="Known owner evidence should route through owner items and exact code.",
            route_verification=_strict_route_expectation(),
        ),
    )

    trace = receipt["routeVerificationTrace"]
    _receipt_validator().validate(receipt)
    _route_validator().validate(trace)
    assert trace["chosenRoute"]["route"] == "owner-items"
    assert [step["route"] for step in trace["executedTrace"]] == [
        "owner-items",
        "query-code",
    ]
    assert {route["route"] for route in trace["rejectedRoutes"]} >= {
        "prime",
        "direct-read",
    }
    assert trace["riskFlags"] == []
    assert "qualityFindings" not in receipt
    assert "/Users/" not in json.dumps(receipt, sort_keys=True)


def test_route_verification_flags_forbidden_prime(
    tmp_path: Path,
) -> None:
    trace_path = tmp_path / "trace.jsonl"
    trace_path.write_text(
        "\n".join(
            [
                json.dumps(_prime_event()),
                json.dumps(_owner_event("rust", "src/lib.rs")),
                json.dumps(_query_code_event("rust", "src/lib.rs")),
            ]
        )
        + "\n",
        encoding="utf-8",
    )

    receipt = build_receipt_from_trace_path(
        trace_path,
        config=TraceReceiptConfig(
            scenario_id="route.known-owner-forbids-prime",
            language="rust",
            project_name="fixture",
            project_source="fixture",
            intent="Known owner evidence must not route through prime.",
            route_verification=_strict_route_expectation(),
        ),
    )

    trace = receipt["routeVerificationTrace"]
    _receipt_validator().validate(receipt)
    _route_validator().validate(trace)
    assert trace["chosenRoute"]["route"] == "prime"
    assert any(flag["kind"] == "unnecessary-prime" for flag in trace["riskFlags"])
    finding_ids = {finding["id"] for finding in receipt["qualityFindings"]}
    assert "route.forbidden.prime" in finding_ids
    assert "route.risk.unnecessary-prime" in finding_ids
    assert "route.first-route.prime" in finding_ids


def test_route_verification_flags_line_range_selector(
    tmp_path: Path,
) -> None:
    trace_path = tmp_path / "trace.jsonl"
    trace_path.write_text(
        json.dumps(
            {
                "id": "line-read",
                "kind": "search",
                "argv": [
                    "asp",
                    "rust",
                    "query",
                    "--from-hook",
                    "direct-source-read",
                    "--selector",
                    "src/lib.rs:10-20",
                    "--workspace",
                    ".",
                    "--code",
                ],
                "metrics": {"elapsedMs": 1, "stdoutBytes": 10, "stderrBytes": 0},
            }
        )
        + "\n",
        encoding="utf-8",
    )

    receipt = build_receipt_from_trace_path(
        trace_path,
        config=TraceReceiptConfig(
            scenario_id="route.line-range-not-executable",
            language="rust",
            project_name="fixture",
            project_source="fixture",
            intent="Line ranges are display hints, not executable selectors.",
            route_verification=_strict_route_expectation(),
        ),
    )

    trace = receipt["routeVerificationTrace"]
    _receipt_validator().validate(receipt)
    _route_validator().validate(trace)
    assert trace["chosenRoute"]["route"] == "direct-read"
    assert any(flag["kind"] == "executable-line-range" for flag in trace["riskFlags"])
    finding_ids = {finding["id"] for finding in receipt["qualityFindings"]}
    assert "route.executable-line-range" in finding_ids
    assert "route.risk.executable-line-range" in finding_ids


def test_committed_fixtures_do_not_contain_absolute_local_paths() -> None:
    fixture_roots = [REPO_ROOT / "tests" / "fixtures"]
    fixture_files = [
        REPO_ROOT / "tests" / "unit" / "semantic_sandtable" / "trace_receipt_fixtures.py",
        *sorted((REPO_ROOT / "schemas").glob("*.fixtures.v1.json")),
    ]
    checked_paths: list[Path] = []
    offenders: list[str] = []
    for root in fixture_roots:
        for path in root.rglob("*"):
            if not path.is_file() or path.suffix not in {".json", ".jsonl", ".py"}:
                continue
            if path.name.startswith("."):
                continue
            checked_paths.append(path)
            text = path.read_text(encoding="utf-8")
            if ABSOLUTE_PATH_RE.search(text):
                offenders.append(path.relative_to(REPO_ROOT).as_posix())
    for path in fixture_files:
        checked_paths.append(path)
        text = path.read_text(encoding="utf-8")
        if ABSOLUTE_PATH_RE.search(text):
            offenders.append(path.relative_to(REPO_ROOT).as_posix())

    assert checked_paths
    assert offenders == []


def _strict_route_expectation() -> dict[str, Any]:
    return {
        "expectedEvidenceAnchors": ["owner-path", "exact-selector"],
        "allowedFirstRoutes": ["owner-items", "query-code"],
        "forbiddenRoutes": ["prime"],
        "requiredRejectedRoutes": ["prime", "direct-read"],
        "forbiddenRiskFlags": ["unnecessary-prime", "executable-line-range"],
        "minScores": {
            "routeFaithfulness": 4,
            "routeEfficiency": 4,
            "semanticPrecision": 4,
            "fallbackDiscipline": 4,
            "verificationHonesty": 4,
            "finalAnswerGrounding": 4,
        },
        "requireRouteJustification": True,
        "requireExactCodeIdentity": True,
        "requireNoExecutableLineRange": True,
        "requireVerificationEvidence": True,
    }


def _prime_event() -> dict[str, Any]:
    return {
        "id": "search-prime",
        "kind": "search",
        "argv": [
            "asp",
            "rust",
            "search",
            "prime",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ],
        "metrics": {"elapsedMs": 4, "stdoutBytes": 80, "stderrBytes": 0},
    }


def _owner_event(language: str, owner_path: str) -> dict[str, Any]:
    return {
        "id": "owner-items",
        "kind": "search",
        "argv": [
            "asp",
            language,
            "search",
            "owner",
            owner_path,
            "items",
            "--query",
            "route_verification",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ],
        "metrics": {"elapsedMs": 2, "stdoutBytes": 60, "stderrBytes": 0},
    }


def _query_code_event(language: str, owner_path: str) -> dict[str, Any]:
    return {
        "id": "query-code",
        "kind": "search",
        "argv": [
            "asp",
            language,
            "query",
            "--from-hook",
            "query-code",
            "--selector",
            f"{language}://{owner_path}#item/function/route_verification",
            "--workspace",
            ".",
            "--code",
        ],
        "metrics": {"elapsedMs": 3, "stdoutBytes": 120, "stderrBytes": 0},
    }


def _route_validator() -> Draft202012Validator:
    return Draft202012Validator(
        _load_schema("semantic-route-verification-trace.v1.schema.json"),
        registry=_schema_registry(),
    )


def _receipt_validator() -> Draft202012Validator:
    return Draft202012Validator(
        _load_schema("semantic-sandtable-receipt.v1.schema.json"),
        registry=_schema_registry(),
    )


def _load_schema(name: str) -> dict[str, Any]:
    return json.loads((SCHEMA_DIR / name).read_text(encoding="utf-8"))


def _schema_registry() -> Registry:
    resources = []
    for schema_path in SCHEMA_DIR.glob("*.schema.json"):
        schema = json.loads(schema_path.read_text(encoding="utf-8"))
        schema_id = schema.get("$id")
        if schema_id:
            resources.append((schema_id, Resource.from_contents(schema)))
    return Registry().with_resources(resources)
