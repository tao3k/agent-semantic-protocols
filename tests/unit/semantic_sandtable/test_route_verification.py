from __future__ import annotations

import json
import re
from pathlib import Path
from typing import Any

import pytest
from jsonschema import Draft202012Validator
from referencing import Registry, Resource

from tools.semantic_sandtable.route_verification import build_route_verification_trace
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
    assert trace["userFeedbackDatasetVersion"] == "2026-06-27"
    assert trace["chosenRoute"]["route"] == "owner-items"
    assert [step["route"] for step in trace["executedTrace"]] == [
        "owner-items",
        "query-code",
    ]
    assert {route["route"] for route in trace["rejectedRoutes"]} >= {
        "prime",
        "direct-read",
    }
    assert _check_status(trace, "route.first.allowed") == "pass"
    assert _check_status(trace, "route.forbidden.avoided") == "pass"
    assert _check_status(trace, "code.exact-identity") == "pass"
    assert _check_status(trace, "selector.line-range-not-executable") == "pass"
    assert _check_status(trace, "feedback.dataset-linked") == "pass"
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
    assert _check_status(trace, "route.first.allowed") == "fail"
    assert _check_status(trace, "route.forbidden.avoided") == "fail"
    assert _check_status(trace, "risk.forbidden.absent") == "fail"
    assert any(
        signal["reason"] == "inefficiency"
        for signal in trace["feedbackSignals"]
    )
    inefficiency_signal = _feedback_signal(trace, "inefficiency")
    assert inefficiency_signal["riskKind"] == "unnecessary-prime"
    assert inefficiency_signal["patternId"] == "route.unnecessary-prime"
    assert inefficiency_signal["feedbackDatasetVersion"] == "2026-06-27"
    assert inefficiency_signal["userFeedbackRefs"] == [
        "route-feedback:avoid-prime-when-owner-known"
    ]
    assert _check_status(trace, "feedback.dataset-linked") == "pass"
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
    assert _check_status(trace, "selector.line-range-not-executable") == "fail"
    assert _check_status(trace, "code.exact-identity") == "fail"
    assert any(signal["reason"] == "overaction" for signal in trace["feedbackSignals"])
    overaction_signal = _feedback_signal_for_risk(trace, "executable-line-range")
    assert overaction_signal["riskKind"] == "executable-line-range"
    assert overaction_signal["patternId"] == "selector.executable-line-range"
    assert overaction_signal["userFeedbackRefs"] == [
        "route-feedback:line-range-is-display-hint"
    ]
    assert _check_status(trace, "feedback.dataset-linked") == "pass"
    finding_ids = {finding["id"] for finding in receipt["qualityFindings"]}
    assert "route.executable-line-range" in finding_ids
    assert "route.risk.executable-line-range" in finding_ids


def test_route_verification_flags_owner_only_frontier_redundancy() -> None:
    trace = build_route_verification_trace(
        [
            {
                "id": "owner-only-pipe",
                "kind": "search",
                "argv": [
                    "asp",
                    "python",
                    "search",
                    "pipe",
                    "route feedback",
                    "--workspace",
                    ".",
                    "--view",
                    "seeds",
                ],
                "stdout": "\n".join(
                    [
                        "[search-pipe] q=route feedback",
                        "aliases: graph:{G=search,O=owner}",
                        "O=owner:path(src/a.py)!owner",
                        "O2=owner:path(src/b.py)!owner",
                        "W=workspace:root(.)@.!topology",
                        "P=provider-root:language-root(python:.)@.!topology",
                        "G>{O:selects,O2:selects,W:contains,P:contains}",
                        "rank=O,O2,W,P frontier=O.owner,O2.owner,W.topology,P.topology",
                    ]
                ),
                "metrics": {"elapsedMs": 1, "stdoutBytes": 320, "stderrBytes": 0},
            }
        ],
        {"allowedFirstRoutes": ["pipe"], "requireVerificationEvidence": True},
    )

    _route_validator().validate(trace)
    assert any(
        flag["kind"] == "owner-only-frontier-redundancy"
        for flag in trace["riskFlags"]
    )
    signal = _feedback_signal_for_risk(trace, "owner-only-frontier-redundancy")
    assert signal["reason"] == "inefficiency"
    assert signal["patternId"] == "graph.owner-only-frontier-redundancy"
    assert signal["userFeedbackRefs"] == [
        "route-feedback:owner-only-frontier-is-redundant"
    ]
    assert trace["behaviorScores"]["routeEfficiency"] == 3


def test_route_verification_allows_actionable_item_frontier_output() -> None:
    trace = build_route_verification_trace(
        [
            {
                "id": "item-pipe",
                "kind": "search",
                "argv": [
                    "asp",
                    "python",
                    "search",
                    "pipe",
                    "route feedback",
                    "--workspace",
                    ".",
                    "--view",
                    "seeds",
                ],
                "stdout": "\n".join(
                    [
                        "[search-pipe] q=route feedback",
                        "O=owner:path(src/a.py)!owner",
                        "Q=query:term(route feedback)!query",
                        "I=item:symbol(RouteVerifier)@python://src/a.py#item/class/RouteVerifier!syntax",
                        "G>{O:selects,Q:matches}",
                        "O>{I:contains}",
                        "rank=I,O frontier=I.syntax",
                    ]
                ),
                "metrics": {"elapsedMs": 1, "stdoutBytes": 260, "stderrBytes": 0},
            }
        ],
        {"allowedFirstRoutes": ["pipe"], "requireVerificationEvidence": True},
    )

    _route_validator().validate(trace)
    assert not any(
        flag["kind"] == "owner-only-frontier-redundancy"
        for flag in trace["riskFlags"]
    )


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


def _check_status(trace: dict[str, Any], check_id: str) -> str:
    for item in trace["judgeChecklist"]:
        if item["id"] == check_id:
            return str(item["status"])
    raise AssertionError(f"missing judge checklist item: {check_id}")


def _feedback_signal(trace: dict[str, Any], reason: str) -> dict[str, Any]:
    for signal in trace["feedbackSignals"]:
        if signal["reason"] == reason:
            return signal
    raise AssertionError(f"missing feedback signal: {reason}")


def _feedback_signal_for_risk(trace: dict[str, Any], risk_kind: str) -> dict[str, Any]:
    for signal in trace["feedbackSignals"]:
        if signal["riskKind"] == risk_kind:
            return signal
    raise AssertionError(f"missing feedback signal for risk: {risk_kind}")


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
