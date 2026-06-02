"""Case model and fixture discovery for parser compact snapshots."""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[4]
FIXTURE_ROOT = REPO_ROOT / "tests" / "fixtures" / "parser-compact"


@dataclass(frozen=True)
class ParserCompactOutputSet:
    root: Path
    line: Path
    code: Path
    query_packet: Path
    token_cost: Path


@dataclass(frozen=True)
class ParserCompactCase:
    case_id: str
    variant_id: str
    language_id: str
    provider_id: str
    feature_class: str
    fixture_root: Path
    raw_source_path: Path
    line_command: list[str]
    json_command: list[str]
    expected_output: ParserCompactOutputSet
    real_output: ParserCompactOutputSet


def load_case(case_path: Path) -> ParserCompactCase:
    data = json.loads(case_path.read_text(encoding="utf-8"))
    fixture_root = REPO_ROOT / data["fixtureRoot"]
    output_key = Path(data["featureClass"]) / data["caseId"] / data["variantId"]
    return ParserCompactCase(
        case_id=data["caseId"],
        variant_id=data["variantId"],
        language_id=data["languageId"],
        provider_id=data["providerId"],
        feature_class=data["featureClass"],
        fixture_root=fixture_root,
        raw_source_path=fixture_root / data["rawSourcePath"],
        line_command=list(data["lineCommand"]),
        json_command=list(data["jsonCommand"]),
        expected_output=output_set(
            FIXTURE_ROOT / "expected-output" / output_key,
            data["languageId"],
        ),
        real_output=output_set(FIXTURE_ROOT / "real-output" / output_key, data["languageId"]),
    )


def output_set(root: Path, language_id: str) -> ParserCompactOutputSet:
    return ParserCompactOutputSet(
        root=root,
        line=root / "line.txt",
        code=root / f"code.{code_extension(language_id)}",
        query_packet=root / "query-packet.json",
        token_cost=root / "token-cost.json",
    )


def code_extension(language_id: str) -> str:
    if language_id == "python":
        return "py"
    if language_id == "rust":
        return "rs"
    if language_id == "typescript":
        return "ts"
    return "code"


def iter_case_paths(case_id: str | None) -> list[Path]:
    cases_dir = FIXTURE_ROOT / "cases"
    if case_id:
        direct_case_file = cases_dir / f"{case_id}.json"
        matching_nested_cases = sorted(cases_dir.glob(f"*/{case_id}/*.json"))
        if matching_nested_cases:
            return matching_nested_cases
        return [direct_case_file]
    return sorted(cases_dir.rglob("*.json"))


def load_matching_cases(
    case_id: str | None,
    language_id: str | None,
) -> list[ParserCompactCase]:
    cases: list[ParserCompactCase] = []
    for case_path in iter_case_paths(case_id):
        case = load_case(case_path)
        if language_id and language_id not in {case.language_id, case.variant_id}:
            continue
        cases.append(case)
    return cases


def case_label(case: ParserCompactCase) -> str:
    return f"{case.feature_class}/{case.case_id}/{case.variant_id}"
