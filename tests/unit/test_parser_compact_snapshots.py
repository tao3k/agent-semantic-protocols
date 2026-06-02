"""Tests for shared parser compact snapshot fixtures."""

from __future__ import annotations

import json
import shutil
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator

from tools import parser_compact_snapshots as snapshots


_REPO_ROOT = Path(__file__).resolve().parents[2]
_FIXTURE_ROOT = _REPO_ROOT / "tests" / "fixtures" / "parser-compact"


def _load_json(path: Path) -> dict[str, object]:
    return json.loads(path.read_text(encoding="utf-8"))


def _validator(schema_name: str) -> Draft202012Validator:
    schema = _load_json(_REPO_ROOT / "schemas" / schema_name)
    return Draft202012Validator(schema)


def _assert_valid(schema_name: str, value: dict[str, object]) -> None:
    errors = sorted(
        _validator(schema_name).iter_errors(value),
        key=lambda error: list(error.path),
    )
    assert errors == []


def _case_paths() -> list[Path]:
    return sorted((_FIXTURE_ROOT / "cases").rglob("*.json"))


def _cases() -> list[snapshots.ParserCompactCase]:
    return [snapshots.load_case(case_path) for case_path in _case_paths()]


def test_parser_compact_case_manifest_is_valid() -> None:
    for case_path in _case_paths():
        _assert_valid("parser-compact-case.v1.schema.json", _load_json(case_path))


@pytest.mark.parametrize("case", _cases(), ids=snapshots.case_label)
def test_parser_compact_expected_packets_are_valid(case: snapshots.ParserCompactCase) -> None:
    _assert_valid(
        "semantic-query-packet.v1.schema.json",
        _load_json(case.expected_output.query_packet),
    )
    _assert_valid(
        "parser-compact-token-cost.v1.schema.json",
        _load_json(case.expected_output.token_cost),
    )


@pytest.mark.parametrize("case", _cases(), ids=snapshots.case_label)
def test_parser_compact_expected_code_is_separate_from_query_packet(
    case: snapshots.ParserCompactCase,
) -> None:
    packet = _load_json(case.expected_output.query_packet)
    matches = packet["matches"]

    assert case.expected_output.code.read_text(encoding="utf-8").strip()
    assert isinstance(matches, list)
    assert all("code" not in match for match in matches if isinstance(match, dict))


def test_parser_compact_expected_output_omits_local_absolute_paths() -> None:
    expected_output_root = _FIXTURE_ROOT / "expected-output"
    for artifact_path in expected_output_root.rglob("*"):
        if not artifact_path.is_file():
            continue
        artifact_text = artifact_path.read_text(encoding="utf-8")

        assert str(_REPO_ROOT) not in artifact_text


def test_parser_compact_snapshot_runner_matches_expected_token_cost() -> None:
    assert snapshots.main(["--case", "control-flow-basic"]) == 0


def test_parser_compact_token_report_keeps_compact_line_smaller_than_raw() -> None:
    for case in _cases():
        report = _load_json(case.expected_output.token_cost)

        assert report["tokenizerId"] == "tiktoken:o200k_base"
        assert report["caseId"] == case.case_id
        assert report["variantId"] == case.variant_id
        assert report["providerId"] == case.provider_id
        assert report["compactLineTokens"] <= report["rawSourceTokens"]
        assert report["compactCodeTokens"] <= report["rawSourceTokens"]
        assert report["compactCodeTokens"] <= report["compactLineTokens"]
        assert report["queryPacketTokens"] > report["compactLineTokens"]
        assert report["compactLineRatio"] <= 1
        assert report["compactCodeRatio"] <= 1


@pytest.mark.parametrize("case", _cases(), ids=snapshots.case_label)
def test_parser_compact_provider_snapshots_match_expected(
    case: snapshots.ParserCompactCase,
) -> None:
    if shutil.which(case.provider_id) is None:
        pytest.skip(f"{case.provider_id} is not installed")
    assert (
        snapshots.main(
            [
                "--case",
                case.case_id,
                "--language",
                case.variant_id,
                "--tokenizer",
                "tiktoken:o200k_base",
                "--check-provider",
            ]
        )
        == 0
    )
