from __future__ import annotations

import json
import re
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[3]
FIXTURE_DIR = REPO_ROOT / "tests" / "fixtures" / "semantic_sandtable" / "large-repo"
FAMILY_FILES = {
    "rust": FIXTURE_DIR / "route-choice-deep-semantic-rust-lang.v1.json",
    "typescript": FIXTURE_DIR / "route-choice-deep-semantic-effect-ts.v1.json",
}
EXPECTED_CASE_IDS = {
    "rust-lang-core-array-const-generic-unsafe-layout",
    "rust-lang-rustc-borrowck-live-long-enough-diagnostic",
    "effect-layer-memomap-scope-finalizer-flow",
    "effect-fiberruntime-interrupt-async-runtime-flow",
}
LOCAL_ABSOLUTE_PATH = re.compile(r"(^|[\s\"'=])/(Users|home|tmp|var|private|Volumes)/")
LINE_RANGE_SELECTOR = re.compile(r":[0-9]+(?::|-)[0-9]+$")


def _load_family(path: Path) -> dict[str, Any]:
    with path.open(encoding="utf-8") as handle:
        data = json.load(handle)
    assert isinstance(data, dict), path
    return data


def _walk_strings(value: Any) -> list[str]:
    if isinstance(value, str):
        return [value]
    if isinstance(value, list):
        strings: list[str] = []
        for item in value:
            strings.extend(_walk_strings(item))
        return strings
    if isinstance(value, dict):
        strings = []
        for item in value.values():
            strings.extend(_walk_strings(item))
        return strings
    return []


def _walk_keys(value: Any) -> list[str]:
    if isinstance(value, list):
        keys: list[str] = []
        for item in value:
            keys.extend(_walk_keys(item))
        return keys
    if isinstance(value, dict):
        keys = list(value)
        for item in value.values():
            keys.extend(_walk_keys(item))
        return keys
    return []


def _all_cases() -> list[tuple[str, dict[str, Any], dict[str, Any]]]:
    cases: list[tuple[str, dict[str, Any], dict[str, Any]]] = []
    for language, path in FAMILY_FILES.items():
        family = _load_family(path)
        for case in family["cases"]:
            cases.append((language, family, case))
    return cases


def test_large_repo_route_choice_family_covers_required_cases() -> None:
    cases = _all_cases()

    assert {case["id"] for _, _, case in cases} == EXPECTED_CASE_IDS
    assert {family["repo"] for _, family, _ in cases} == {
        "rust-lang/rust",
        "Effect-TS/effect",
    }
    assert {language for language, _, _ in cases} == {"rust", "typescript"}


def test_large_repo_route_choice_forbids_prime_full_read_and_line_selectors() -> None:
    for _, _, case in _all_cases():
        route = case["routeExpectation"]
        metrics = case["metrics"]
        packet = case["simulatedPacket"]

        assert route["allowedFirstRoutes"]
        assert "search-prime" not in route["allowedFirstRoutes"]
        assert "search-prime" in route["forbiddenFirstRoutes"]
        assert "line-selector-code" in route["requiredRejectedRoutes"]
        assert route["queryCodeRequiresExact"] is True
        assert route["requireNoExecutableLineRange"] is True

        assert metrics["commandBudgetMax"] <= 6
        assert metrics["firstRouteIsPrime"] is False
        assert metrics["unnecessaryPrimeCount"] == 0
        assert metrics["fullFileReadCount"] == 0
        assert metrics["lineRangeExecutableSelectorCount"] == 0
        assert metrics["ownerSkeletonCountMin"] >= 1

        assert packet["routeGraph"]
        assert packet["actionFrontier"]
        assert packet["recommendedNext"]
        assert "search-prime" in packet["avoid"]
        assert not {"command", "argv", "nextCommand"}.intersection(_walk_keys(packet))


def test_large_repo_route_choice_uses_structural_gold_selectors() -> None:
    for language, _, case in _all_cases():
        selector_prefix = "rust://" if language == "rust" else "ts://"
        selectors = case["expectedGoldSelectors"]

        assert selectors
        for selector in selectors:
            assert selector.startswith(selector_prefix)
            assert "#" in selector
            assert not LINE_RANGE_SELECTOR.search(selector)


def test_large_repo_route_choice_has_projection_specific_gates() -> None:
    capsule_gates = {
        "diagnosticCapsuleCountMin",
        "capsuleProjectionCountMin",
        "runtimeFlowCapsuleCountMin",
        "unsafeBlockProjectionCountMin",
    }
    for _, _, case in _all_cases():
        metrics = case["metrics"]
        packet = case["simulatedPacket"]

        assert metrics.get("syntaxFactQueryCountMin", 0) >= 1
        assert capsule_gates.intersection(metrics), case["id"]
        assert any("skeleton" in action for action in packet["actionFrontier"])
        assert any("capsule" in action or "syntax-fact-query" in action for action in packet["actionFrontier"])


def test_large_repo_route_choice_fixtures_are_github_safe() -> None:
    for path in FAMILY_FILES.values():
        family = _load_family(path)

        assert family["globalGates"]["orgArtifactByDefault"] is False
        for text in _walk_strings(family):
            assert not text.startswith("/")
            assert not LOCAL_ABSOLUTE_PATH.search(text)
