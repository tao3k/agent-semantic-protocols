from __future__ import annotations

import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
LOCK_PATH = ROOT / "benchmarks/large-library-runtime-corpora.json"
PLAN_PATH = ROOT / "benchmarks/live-corpus-search-query-qualification.json"


def load(path: Path) -> dict[str, object]:
    return json.loads(path.read_text(encoding="utf-8"))


def test_every_locked_live_corpus_has_one_fixed_search_query_case() -> None:
    lock = load(LOCK_PATH)
    plan = load(PLAN_PATH)
    corpora = lock["corpora"]
    cases = plan["cases"]

    assert len(corpora) == 17
    assert len(cases) == 17

    locked = {
        corpus["resourceId"]: (
            corpus["scenarioId"],
            corpus["language"],
            corpus["providerId"],
        )
        for corpus in corpora
    }
    planned = {
        case["resourceId"]: (
            case["scenarioId"],
            case["languageId"],
            case["providerId"],
        )
        for case in cases
    }
    assert len(locked) == len(corpora)
    assert len(planned) == len(cases)
    assert planned == locked

    case_ids = [case["caseId"] for case in cases]
    assert all(case_ids)
    assert len(set(case_ids)) == len(case_ids)


def test_required_languages_are_exactly_the_locked_language_set() -> None:
    lock = load(LOCK_PATH)
    plan = load(PLAN_PATH)
    locked_languages = {corpus["language"] for corpus in lock["corpora"]}
    assert set(plan["requiredLanguages"]) == locked_languages
    assert locked_languages == {
        "gerbil-scheme",
        "julia",
        "md",
        "org",
        "python",
        "rust",
        "typescript",
    }
