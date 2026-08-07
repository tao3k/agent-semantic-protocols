"""CLI gate behavior tests."""

import io
import json

import pytest

from asp_schema_manager import cli


def _report(
    *,
    family_local: int,
    cross_family: int,
    mixed: int = 0,
    unclassified: int = 0,
) -> dict:
    return {
        "summary": {
            "unclassifiedSchemaCount": unclassified,
            "referenceOpportunityScopeCounts": {
                "family-local": family_local,
                "cross-family": cross_family,
                "mixed": mixed,
                "unclassified": 0,
            }
        },
        "diagnostics": [],
        "referenceOpportunities": [],
    }


def test_check_fail_on_family_local_refs(monkeypatch) -> None:
    monkeypatch.setattr(
        cli,
        "audit_workspace",
        lambda *args, **kwargs: _report(family_local=1, cross_family=0),
    )
    output = io.StringIO()
    status = cli.main(
        ["check", "--fail-on-family-local-refs", "--json"], stdout=output
    )
    assert status == 1
    payload = json.loads(output.getvalue())
    assert any(
        item["code"] == "family-local-reference-opportunity"
        for item in payload["diagnostics"]
    )


def test_check_cross_family_refs_remains_report_only(monkeypatch) -> None:
    monkeypatch.setattr(
        cli,
        "audit_workspace",
        lambda *args, **kwargs: _report(family_local=0, cross_family=2),
    )
    output = io.StringIO()
    status = cli.main(
        ["check", "--fail-on-family-local-refs", "--json"], stdout=output
    )
    assert status == 0
    payload = json.loads(output.getvalue())
    assert not any(
        item["code"] == "family-local-reference-opportunity"
        for item in payload["diagnostics"]
    )


def test_family_local_gate_requires_check_command() -> None:
    with pytest.raises(SystemExit) as exc_info:
        cli.main(
            ["audit", "--fail-on-family-local-refs", "--json"],
            stdout=io.StringIO(),
        )
    assert exc_info.value.code == 2


def test_check_fail_on_unclassified_schemas(monkeypatch) -> None:
    monkeypatch.setattr(
        cli,
        "audit_workspace",
        lambda *args, **kwargs: _report(
            family_local=0, cross_family=0, unclassified=1
        ),
    )
    output = io.StringIO()
    status = cli.main(
        ["check", "--fail-on-unclassified-schemas", "--json"], stdout=output
    )
    assert status == 1
    payload = json.loads(output.getvalue())
    assert any(
        item["code"] == "unclassified-schema" for item in payload["diagnostics"]
    )


def test_check_fail_on_mixed_family_refs(monkeypatch) -> None:
    monkeypatch.setattr(
        cli,
        "audit_workspace",
        lambda *args, **kwargs: _report(family_local=0, cross_family=0, mixed=1),
    )
    output = io.StringIO()
    status = cli.main(
        ["check", "--fail-on-mixed-family-refs", "--json"], stdout=output
    )
    assert status == 1
    payload = json.loads(output.getvalue())
    assert any(
        item["code"] == "mixed-family-reference-opportunity"
        for item in payload["diagnostics"]
    )


@pytest.mark.parametrize(
    "flag",
    [
        "--fail-on-unclassified-schemas",
        "--fail-on-mixed-family-refs",
        "--fail-on-reference-decision-drift",
    ],
)
def test_family_registry_gates_require_check_command(flag: str) -> None:
    with pytest.raises(SystemExit) as exc_info:
        cli.main(["audit", flag, "--json"], stdout=io.StringIO())
    assert exc_info.value.code == 2


def test_check_fail_on_reference_decision_drift(monkeypatch) -> None:
    report = _report(family_local=0, cross_family=1)
    report["diagnostics"].append(
        {
            "severity": "warning",
            "code": "cross-family-reference-decision-missing",
            "message": "missing",
        }
    )
    monkeypatch.setattr(cli, "audit_workspace", lambda *args, **kwargs: report)
    output = io.StringIO()
    status = cli.main(
        ["check", "--fail-on-reference-decision-drift", "--json"],
        stdout=output,
    )
    assert status == 1
    payload = json.loads(output.getvalue())
    assert any(
        item["code"] == "reference-decision-drift"
        for item in payload["diagnostics"]
    )


def test_check_fail_on_stale_reference_decision(monkeypatch) -> None:
    report = _report(family_local=0, cross_family=0)
    report["diagnostics"].append(
        {
            "severity": "warning",
            "code": "stale-reference-decision",
            "message": "stale",
        }
    )
    monkeypatch.setattr(cli, "audit_workspace", lambda *args, **kwargs: report)
    status = cli.main(
        ["check", "--fail-on-reference-decision-drift", "--json"],
        stdout=io.StringIO(),
    )
    assert status == 1
