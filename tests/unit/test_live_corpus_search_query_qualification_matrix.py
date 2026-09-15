# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Locked per-language corpus coverage for Search/Query qualification."""

from jsonschema import Draft202012Validator

from tests.unit.live_corpus_search_query_qualification_support import (
    LOCK_PATH,
    PLAN_SCHEMA_PATH,
    RECEIPT_SCHEMA_PATH,
    load_json, load_plan,
)


def test_live_corpus_search_query_plan_covers_the_complete_locked_matrix() -> None:
    plan_schema = load_json(PLAN_SCHEMA_PATH)
    receipt_schema = load_json(RECEIPT_SCHEMA_PATH)
    Draft202012Validator.check_schema(plan_schema)
    Draft202012Validator.check_schema(receipt_schema)

    plan = load_plan()
    lock = load_json(LOCK_PATH)
    Draft202012Validator(plan_schema).validate(plan)
    under_sampled = dict(plan)
    under_sampled["resident_sample_count"] = 127
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
        entry["resource_id"]: (
            entry["scenario_id"],
            entry["language_id"],
            entry["provider_id"],
        )
        for entry in plan["cases"]
    }

    assert planned == locked
    assert len(planned) == 17
    assert set(plan["required_languages"]) == {
        "gerbil-scheme",
        "julia",
        "md",
        "org",
        "python",
        "rust",
        "typescript",
    }
    assert plan["client_protocol"]["applies_to_case_count"] == len(plan["cases"]) == 17
    assert plan["client_protocol"]["maximum_resident_micros"] <= 1000
    assert plan["client_protocol"]["workspace_scheduling"] == "tokio-join-set"
    assert plan["client_protocol"]["session_policy"] == "one-initialize-per-session"
    assert plan["client_protocol"]["ready_effects"] == ["mpsc", "oneshot", "cancel", "response"]
    assert plan["client_protocol"]["forbidden_ready_effects"] == [
        "process",
        "filesystem",
        "dbWrite",
        "generationMutation",
        "providerActivation",
        "controlPoll",
    ]
    assert plan["client_protocol"]["non_ready_dispatch_count"] == 0
    assert plan["client_protocol"]["residual_task_count"] == 0
    assert plan["client_protocol"]["p50_maximum_micros"] == 250
    assert plan["client_protocol"]["p99_maximum_micros"] == 700
    assert plan["client_protocol"]["max_maximum_micros"] == 1000
    assert {provider_id for _, _, provider_id in planned.values()} == {
        "asp-gerbil-scheme",
        "asp-julia",
        "asp-md",
        "asp-org",
        "asp-python",
        "asp-rust",
        "asp-typescript",
    }

    by_language = {language: [] for language in plan["required_languages"]}
    for entry in plan["cases"]:
        by_language[entry["language_id"]].append(entry)
    assert all(by_language.values())
    for entries in by_language.values():
        assert all(entry["minimum_candidates"] >= 1 for entry in entries)
        assert all(entry["search"].startswith("(search ") for entry in entries)
        assert all(entry["zero_match_search"].startswith("(search ") for entry in entries)
