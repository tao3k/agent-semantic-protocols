from tools.search_wall_gate import (
    AGENT_FACING_WALL_BUDGET_MICROS,
    budget_status,
    build_receipt,
    validate_asp_command,
)


def test_wall_budget_is_strict_at_one_second() -> None:
    assert budget_status(999_999) == "within-budget"
    assert budget_status(1_000_000) == "budget-exceeded"
    assert budget_status(3_000_000) == "budget-exceeded"
    assert budget_status(10_300_000) == "budget-exceeded"


def test_receipt_contains_digest_not_raw_command() -> None:
    argv = ["asp", "rust", "query", "--selector", "secret-selector"]
    receipt = build_receipt(
        language_id="rust",
        surface="query",
        argv=argv,
        wall_time_micros=AGENT_FACING_WALL_BUDGET_MICROS,
        reply_kind="evidence",
        exit_code=0,
    )
    serialized = str(receipt)
    assert receipt["budgetStatus"] == "budget-exceeded"
    assert receipt["timingScope"] == "agent-facing-end-to-end"
    assert "secret-selector" not in serialized


def test_gate_accepts_only_matching_asp_facade_surface() -> None:
    validate_asp_command(["asp", "python", "search", "prime"], "python", "search")


def test_gate_rejects_provider_binary() -> None:
    try:
        validate_asp_command(["rs-harness", "rust", "query"], "rust", "query")
    except ValueError as error:
        assert "ASP facade" in str(error)
    else:
        raise AssertionError("direct provider binary must not be measurable through this gate")
