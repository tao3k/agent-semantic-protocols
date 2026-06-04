"""Compact AST projection token savings tests."""

from __future__ import annotations

from .support import (
    CompactionReport,
    ast_fact_projection,
    compact_projection,
    compaction_report,
    prompt_token_estimate,
    retained_agent_facts,
)

_FORMATTED_SOURCE = '''
def classify(value: int, debug: bool = False) -> int:
    """Verbose human-facing documentation that should not enter compact output."""
    # Human readers may want this explanation; the agent needs the branch facts.
    if debug:
        print("debug", value)

    if value > 10:
        return value + 1

    return 0
'''


def test_compact_projection_reports_savings_and_retains_agent_facts() -> None:
    projection = compact_projection(_FORMATTED_SOURCE)
    report = compaction_report(_FORMATTED_SOURCE)

    assert projection == (
        "def classify(value: int, debug: bool = False) -> int",
        "if debug",
        "call print('debug', value)",
        "if value > 10",
        "return value + 1",
        "return 0",
    )
    assert report.retained_agent_facts == (
        "declaration",
        "branch",
        "effect-call",
        "terminal-return",
    )
    assert report == CompactionReport(
        raw_chars=322,
        compact_chars=128,
        raw_prompt_tokens=89,
        compact_prompt_tokens=41,
        char_savings_percent=60,
        prompt_token_savings_percent=53,
        retained_agent_facts=(
            "declaration",
            "branch",
            "effect-call",
            "terminal-return",
        ),
    )


def test_ast_fact_projection_finds_remaining_token_waste() -> None:
    compact_text = "\n".join(compact_projection(_FORMATTED_SOURCE))
    fact_text = "\n".join(ast_fact_projection(_FORMATTED_SOURCE))

    assert ast_fact_projection(_FORMATTED_SOURCE) == (
        "F classify/2->int",
        "B debug",
        "E print/2",
        "B value>int",
        "R value+int",
        "R int",
    )
    assert retained_agent_facts(ast_fact_projection(_FORMATTED_SOURCE)) == (
        "declaration",
        "branch",
        "effect-call",
        "terminal-return",
    )
    assert len(fact_text) < len(compact_text)
    assert prompt_token_estimate(fact_text) < prompt_token_estimate(compact_text)
    assert (len(compact_text) - len(fact_text)) * 100 // len(compact_text) >= 45
    assert len(fact_text) == 65
    assert prompt_token_estimate(fact_text) == 27
    assert (len(compact_text) - len(fact_text)) * 100 // len(compact_text) == 49
    assert (
        (prompt_token_estimate(compact_text) - prompt_token_estimate(fact_text))
        * 100
        // prompt_token_estimate(compact_text)
        == 34
    )
