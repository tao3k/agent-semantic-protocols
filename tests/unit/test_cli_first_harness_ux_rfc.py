"""Contract checks for the CLI-first harness UX RFC."""

from pathlib import Path

_REPO_ROOT = Path(__file__).resolve().parents[2]
_RFC_PATH = _REPO_ROOT / "rfcs" / "cli-first-harness-ux.org"


def test_cli_first_rfc_defines_exact_direct_read_contract() -> None:
    text = _RFC_PATH.read_text(encoding="utf-8")

    required_terms = [
        "For exact selectors without =--code=",
        "bounded",
        "=read-owner= source window",
        "=read-plan= frontier",
        "must keep code omitted and provide executable read locators",
        "For an exact =direct-source-read= selector with =--code=",
        "must not repeat =line= or =endLine= metadata",
        "tools/validate-exact-direct-read-contract.sh",
    ]

    missing_terms = [term for term in required_terms if term not in text]

    assert missing_terms == []


def test_cli_first_rfc_keeps_search_read_plan_frontier_gate() -> None:
    text = _RFC_PATH.read_text(encoding="utf-8")

    required_terms = [
        "Search and",
        "read-plan flows must return locators/frontier rather than inline code",
        "tools/validate-search-read-plan-frontier-contract.sh",
    ]

    missing_terms = [term for term in required_terms if term not in text]

    assert missing_terms == []


def test_cli_first_rfc_requires_shell_safe_query_literals() -> None:
    text = _RFC_PATH.read_text(encoding="utf-8")

    required_terms = [
        "Agents must also quote literal query text",
        "Backticks are command substitution",
        "single-quoted argv literal",
        "--query-set 'Start with `asp <language> guide .`'",
        "provider-documented file/stdin input",
        "surface rather than interpolating raw prose into a shell command",
        "must not ask agents",
        "shell escaping side effects",
    ]

    missing_terms = [term for term in required_terms if term not in text]

    assert missing_terms == []


def test_cli_first_rfc_uses_guide_as_primary_agent_tool_map() -> None:
    text = _RFC_PATH.read_text(encoding="utf-8")
    start = text.index("Agent-facing guides must expose the syntax ABI")
    end = text.index("Hook recovery reason text is a compact Markdown prompt")
    guide_section = text[start:end]

    required_terms = [
        "[guide] lang=<language> provider=<asp-provider> protocol=guide.v1 root=.",
        "=guide= is the provider's default agent-facing guide command",
        "The spelling =agent guide= is retired",
        "=query guide treesitter .= as a low-frequency reference",
    ]

    missing_terms = [term for term in required_terms if term not in guide_section]

    assert missing_terms == []
    assert "[agent-guide]" not in guide_section
    assert "protocol=agent-guide.v1" not in guide_section
