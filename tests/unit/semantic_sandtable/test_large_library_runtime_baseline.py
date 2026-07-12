from tools.semantic_sandtable.large_library_runtime_baseline import (
    BASELINE_SCHEMA_ID,
    validate_runtime_baseline,
)


def test_runtime_baseline_accepts_complete_receipt_within_budget() -> None:
    baseline = baseline_fixture()
    receipt = receipt_fixture(elapsed_ms=6_000)

    report = validate_runtime_baseline(baseline, receipt)

    assert report["status"] == "pass"
    assert report["scenarioBudgetsMs"] == {"python.pandas": 7_500}


def test_runtime_baseline_rejects_coverage_and_scenario_regression() -> None:
    baseline = baseline_fixture()
    receipt = receipt_fixture(elapsed_ms=8_000)
    receipt["commandCoverage"]["runtimeSearchCommandCount"] = 0

    report = validate_runtime_baseline(baseline, receipt)

    assert report["status"] == "fail"
    assert "coverage-runtimeSearchCommandCount" in report["errors"]
    assert "scenario-budget-python.pandas" in report["errors"]


def test_runtime_baseline_rejects_corpus_identity_drift() -> None:
    baseline = baseline_fixture()
    receipt = receipt_fixture(elapsed_ms=6_000)
    receipt["corpora"][0]["revision"] = "different"

    report = validate_runtime_baseline(baseline, receipt)

    assert report["status"] == "fail"
    assert "corpus-identity-python.pandas" in report["errors"]


def baseline_fixture() -> dict[str, object]:
    return {
        "schemaId": BASELINE_SCHEMA_ID,
        "schemaVersion": "1",
        "budget": {"maxCommandElapsedMs": 15_000},
        "commandCoverage": {
            "registeredSearchMethodCount": 1,
            "runtimeSearchCommandCount": 1,
            "targetSearchCommandCount": 1,
        },
        "workspaceDeployments": [{"language": "python", "elapsedMs": 1}],
        "corpora": [
            {
                "scenarioId": "python.pandas",
                "language": "python",
                "repository": "pandas-dev/pandas",
                "revision": "abc123",
                "directory": "python-pandas",
            }
        ],
        "scenarios": [
            {
                "id": "python.pandas",
                "commands": 1,
                "maxElapsedMs": 5_000,
                "observationsMs": [4_000, 5_000],
            }
        ],
    }


def receipt_fixture(*, elapsed_ms: int) -> dict[str, object]:
    return {
        "status": "pass",
        "commandCoverage": {
            "registeredSearchMethodCount": 1,
            "runtimeSearchCommandCount": 1,
            "targetSearchCommandCount": 1,
            "missingMethods": [],
            "missingCorpusMethods": [],
        },
        "workspaceDeployments": [{"language": "python", "status": "pass"}],
        "corpora": [
            {
                "scenarioId": "python.pandas",
                "language": "python",
                "repository": "pandas-dev/pandas",
                "revision": "abc123",
                "path": "/tmp/python-pandas",
            }
        ],
        "steps": [{"scenarioId": "python.pandas", "elapsedMs": elapsed_ms}],
    }
