import json
from pathlib import Path

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[2]
PLAN_PATH = ROOT / "benchmarks/live-corpus-search-query-qualification.json"
LOCK_PATH = ROOT / "benchmarks/large-library-runtime-corpora.json"
PLAN_SCHEMA_PATH = (
    ROOT / "schemas/asp.live-corpus-search-query-qualification-plan.schema.json"
)
RECEIPT_SCHEMA_PATH = (
    ROOT / "schemas/asp.live-corpus-search-query-qualification-receipt.schema.json"
)


def load_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


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
    assert {provider_id for _, _, provider_id in planned.values()} == {
        "asp-gerbil-scheme",
        "asp-julia",
        "asp-md",
        "asp-org",
        "asp-python",
        "asp-rust",
        "asp-typescript",
    }


def test_every_locked_corpus_has_fixed_search_query_and_telemetry_budgets() -> None:
    plan = load_json(PLAN_PATH)
    assert plan["residentSampleCount"] >= 128
    case_ids = [entry["caseId"] for entry in plan["cases"]]
    assert len(case_ids) == len(set(case_ids))

    for entry in plan["cases"]:
        assert entry["search"]["maximumResidentMicros"] == 1_000
        assert entry["search"]["minimumCandidates"] >= 1
        assert entry["query"]["maximumResidentMicros"] == 1_000
        assert entry["query"]["selectorStrategy"] == "first-ranked-parser-owned"
        assert entry["query"]["projectionScope"] == "live-corpus"
        assert entry["zeroMatchTerms"]
        assert set(entry["requiredTelemetryEvents"]) == {
            "runtime_resident_search_terminal",
            "runtime_exact_projection_terminal",
        }


def test_receipt_requires_complete_sub_millisecond_latency_distributions() -> None:
    receipt_schema = load_json(RECEIPT_SCHEMA_PATH)
    case_schema = receipt_schema["$defs"]["caseReceipt"]
    required = set(case_schema["required"])
    assert {
        "residentSampleCount",
        "searchResidentReadLatencyMicros",
        "searchServiceLatencyMicros",
        "searchTotalLatencyMicros",
        "exactSourceLatencyMicros",
        "callableSkeletonLatencyMicros",
    } <= required

    distribution = receipt_schema["$defs"]["latencyDistribution"]
    assert distribution["additionalProperties"] is False
    assert set(distribution["required"]) == {
        "sampleCount",
        "minMicros",
        "p50Micros",
        "p95Micros",
        "p99Micros",
        "maxMicros",
    }
    assert distribution["properties"]["sampleCount"]["minimum"] == 128
    for field in ("minMicros", "p50Micros", "p95Micros", "p99Micros", "maxMicros"):
        assert distribution["properties"][field]["maximum"] == 1_000
