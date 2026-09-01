"""Locked per-language corpus coverage for Search/Query qualification."""

from jsonschema import Draft202012Validator

from tests.unit.live_corpus_search_query_qualification_support import (
    LOCK_PATH,
    PLAN_PATH,
    PLAN_SCHEMA_PATH,
    RECEIPT_SCHEMA_PATH,
    load_json,
)


def test_live_corpus_search_query_plan_covers_the_complete_locked_matrix() -> None:
    plan_schema = load_json(PLAN_SCHEMA_PATH)
    receipt_schema = load_json(RECEIPT_SCHEMA_PATH)
    Draft202012Validator.check_schema(plan_schema)
    Draft202012Validator.check_schema(receipt_schema)

    plan = load_json(PLAN_PATH)
    lock = load_json(LOCK_PATH)
    Draft202012Validator(plan_schema).validate(plan)
    under_sampled = dict(plan)
    under_sampled["residentSampleCount"] = 127
    assert list(Draft202012Validator(plan_schema).iter_errors(under_sampled))

    locked = {
        entry["resourceId"]: (
            entry["scenarioId"],
            entry["language"],
            entry["providerId"],
        )
        for entry in lock["corpora"]
    }
    planned = {
        entry["resourceId"]: (
            entry["scenarioId"],
            entry["languageId"],
            entry["providerId"],
        )
        for entry in plan["cases"]
    }

    assert planned == locked
    assert len(planned) == 17
    assert set(plan["requiredLanguages"]) == {
        "gerbil-scheme",
        "julia",
        "md",
        "org",
        "python",
        "rust",
        "typescript",
    }
    assert plan["clientProtocol"]["appliesToCaseCount"] == len(plan["cases"]) == 17
    assert plan["clientProtocol"]["maximumResidentMicros"] <= 1000
    assert plan["clientProtocol"]["sessionPolicy"] == "one-initialize-per-session"
    assert plan["clientProtocol"]["readyEffects"] == ["mpsc", "oneshot", "cancel", "response"]
    assert plan["clientProtocol"]["forbiddenReadyEffects"] == [
        "process",
        "filesystem",
        "dbWrite",
        "generationMutation",
        "providerActivation",
        "controlPoll",
    ]
    assert plan["clientProtocol"]["nonReadyDispatchCount"] == 0
    assert plan["clientProtocol"]["residualTaskCount"] == 0
    assert plan["clientProtocol"]["p50MaximumMicros"] == 250
    assert plan["clientProtocol"]["p99MaximumMicros"] == 700
    assert plan["clientProtocol"]["maxMaximumMicros"] == 1000
    assert {provider_id for _, _, provider_id in planned.values()} == {
        "asp-gerbil-scheme",
        "asp-julia",
        "asp-md",
        "asp-org",
        "asp-python",
        "asp-rust",
        "asp-typescript",
    }

    by_language = {language: [] for language in plan["requiredLanguages"]}
    for entry in plan["cases"]:
        by_language[entry["languageId"]].append(entry)
    assert all(by_language.values())
    for entries in by_language.values():
        assert all(entry["search"]["minimumCandidates"] >= 1 for entry in entries)
        assert all(entry["zeroMatchTerms"] for entry in entries)
