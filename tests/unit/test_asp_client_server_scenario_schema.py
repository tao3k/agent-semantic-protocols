import copy
import json
from pathlib import Path

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[2]
SCHEMA = json.loads(
    (ROOT / "schemas/asp-client-server-scenario.v1.schema.json").read_text()
)
NATIVE = json.loads(
    (ROOT / "schemas/fixtures/asp-client-server-scenario.native.v1.json").read_text()
)
LIVE_CORPUS = json.loads(
    (
        ROOT
        / "schemas/fixtures/asp-client-server-scenario.live-corpus.v1.json"
    ).read_text()
)
LIVE_CORPUS_LOCK = json.loads(
    (ROOT / "benchmarks/large-library-runtime-corpora.v1.json").read_text()
)
VALIDATOR = Draft202012Validator(SCHEMA)


def test_native_and_live_corpus_scenarios_validate() -> None:
    Draft202012Validator.check_schema(SCHEMA)
    VALIDATOR.validate(NATIVE)
    VALIDATOR.validate(LIVE_CORPUS)


def test_live_corpus_scenario_requires_qualification_evidence() -> None:
    invalid = copy.deepcopy(LIVE_CORPUS)
    invalid.pop("liveCorpusEvidence")
    assert list(VALIDATOR.iter_errors(invalid))


def test_native_scenario_rejects_live_corpus_evidence() -> None:
    invalid = copy.deepcopy(NATIVE)
    invalid["liveCorpusEvidence"] = copy.deepcopy(LIVE_CORPUS["liveCorpusEvidence"])
    assert list(VALIDATOR.iter_errors(invalid))


def test_scenario_rejects_legacy_provider_and_transport_namespaces() -> None:
    invalid_provider = copy.deepcopy(NATIVE)
    invalid_provider["authority"]["providerId"] = "rs-harness"
    assert list(VALIDATOR.iter_errors(invalid_provider))

    invalid_transport = copy.deepcopy(NATIVE)
    invalid_transport["transport"] = "unsupported-transport"
    assert list(VALIDATOR.iter_errors(invalid_transport))


def test_resident_step_budget_is_millisecond_bounded() -> None:
    invalid = copy.deepcopy(NATIVE)
    invalid["steps"][0]["budgetMicros"] = 1001
    assert list(VALIDATOR.iter_errors(invalid))


def test_every_locked_live_corpus_has_a_canonical_scenario_case() -> None:
    corpora = LIVE_CORPUS_LOCK["corpora"]
    assert len(corpora) == 17

    scenario_ids = set()
    for corpus in corpora:
        scenario = copy.deepcopy(LIVE_CORPUS)
        scenario_id = f"live-corpus.{corpus['resourceId']}"
        scenario["scenarioId"] = scenario_id
        scenario["authority"]["languageId"] = corpus["language"]
        scenario["authority"]["providerId"] = corpus["providerId"]
        scenario["liveCorpusEvidence"]["resourceId"] = corpus["resourceId"]
        scenario["liveCorpusEvidence"]["revision"] = corpus["git"]["revision"]

        VALIDATOR.validate(scenario)
        assert scenario_id not in scenario_ids
        scenario_ids.add(scenario_id)

    assert {corpus["language"] for corpus in corpora} == {
        "gerbil-scheme",
        "julia",
        "md",
        "org",
        "python",
        "rust",
        "typescript",
    }
