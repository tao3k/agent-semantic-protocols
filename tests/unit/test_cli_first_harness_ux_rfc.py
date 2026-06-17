"""Contract checks for the CLI-first harness UX RFC."""

from pathlib import Path

_REPO_ROOT = Path(__file__).resolve().parents[2]
_RFC_PATH = _REPO_ROOT / "docs" / "10-19-rfcs" / "10.05-cli-first-harness-ux.org"
_RFC_MODULE_DIR = (
    _REPO_ROOT / "docs" / "10-19-rfcs" / "10.05-cli-first-harness-ux"
)


def _read_cli_first_rfc_text() -> str:
    module_text = "\n".join(
        path.read_text(encoding="utf-8")
        for path in sorted(_RFC_MODULE_DIR.glob("*.org"))
    )
    return _RFC_PATH.read_text(encoding="utf-8") + "\n" + module_text


def test_cli_first_rfc_is_module_index() -> None:
    text = _RFC_PATH.read_text(encoding="utf-8")

    required_terms = [
        "This file is the stable RFC 005 entry point.",
        "split into focused Org modules under =docs/10-19-rfcs/10.05-cli-first-harness-ux/=",
        "Tests read this file plus the module directory explicitly",
        '10.05-cli-first-harness-ux/10.05.10-search-query-surface.org',
        '10.05-cli-first-harness-ux/10.05.40-daemon-guide-hooks.org',
        'docs/10-19-rfcs/10.12-asp-native-relation-flow-codeql.org (012)',
    ]

    missing_terms = [term for term in required_terms if term not in text]

    assert missing_terms == []


def test_cli_first_rfc_defines_exact_direct_read_contract() -> None:
    text = _read_cli_first_rfc_text()

    required_terms = [
        "For exact selectors without =--code=",
        "bounded",
        "=read-owner= source window",
        "=read-plan= frontier",
        "must keep code omitted and provide executable read locators",
        "For an exact =direct-source-read= selector with =--code=",
        "must not repeat =line= or =endLine= metadata",
        "python -m tools tree-sitter validate exact-direct-read-contract",
    ]

    missing_terms = [term for term in required_terms if term not in text]

    assert missing_terms == []


def test_cli_first_rfc_keeps_search_read_plan_frontier_gate() -> None:
    text = _read_cli_first_rfc_text()

    required_terms = [
        "Search and",
        "read-plan flows must return locators/frontier rather than inline code",
        "python -m tools tree-sitter validate search-read-plan-frontier-contract",
    ]

    missing_terms = [term for term in required_terms if term not in text]

    assert missing_terms == []


def test_cli_first_rfc_uses_path_context_prime_instead_of_workspace_route() -> None:
    text = _read_cli_first_rfc_text()

    required_terms = [
        "Phase 1 starts with =<lang-harness> search prime=.",
        "There is no shared",
        "=search workspace= surface",
        "provider-owned path-context facts",
        "provider resolves path context",
    ]
    forbidden_command_terms = [
        "asp <language> search workspace",
        "asp typescript search workspace",
        "ts-harness search workspace",
        "py-harness search workspace",
        "rs-harness search workspace",
    ]

    missing_terms = [term for term in required_terms if term not in text]
    command_violations = [term for term in forbidden_command_terms if term in text]

    assert missing_terms == []
    assert command_violations == []


def test_cli_first_rfc_requires_shell_safe_query_literals() -> None:
    text = _read_cli_first_rfc_text()

    required_terms = [
        "Agents must also quote literal query text",
        "Backticks are command substitution",
        "single-quoted argv literal",
        "--query-set 'Start with `asp <language> guide --workspace .`'",
        "provider-documented file/stdin input",
        "surface rather than interpolating raw prose into a shell command",
        "must not ask agents",
        "shell escaping side effects",
    ]

    missing_terms = [term for term in required_terms if term not in text]

    assert missing_terms == []


def test_cli_first_rfc_uses_guide_as_primary_agent_tool_map() -> None:
    text = _read_cli_first_rfc_text()
    start = text.index("Agent-facing guides must expose the syntax ABI")
    end = text.index("Hook recovery reason text is a compact Markdown prompt")
    guide_section = text[start:end]

    required_terms = [
        "[guide] lang=<language> provider=<asp-provider> protocol=guide.v1 root=.",
        "=guide= is the provider's default agent-facing guide command",
        "The agent-prefixed guide spelling is retired",
        "=query guide treesitter .= as a low-frequency reference",
    ]

    missing_terms = [term for term in required_terms if term not in guide_section]

    assert missing_terms == []
    assert "[agent-guide]" not in guide_section
    assert "protocol=agent-guide.v1" not in guide_section


def test_cli_first_rfc_defines_agent_facing_guide_acceptance() -> None:
    text = _read_cli_first_rfc_text()

    required_terms = [
        "Agent-facing guide acceptance:",
        'guide-main        command="asp <language> guide --workspace ."',
        'guide-help        command="asp <language> guide --help ."',
        "lowFrequency=true",
        "inlineSubguides=false",
        "search-prime      output=handles/profiles/frontier code=false json=false",
        "syntax-locate     command=\"query --treesitter-query <pattern>\"",
        "syntax-code       command=\"query --selector <exact> --treesitter-query <pattern> --code\" output=pure-code",
        "hook-recovery     output=markdown-prompt runNext=query-from-hook detectedBinaries=runtime-profile",
        "debug-surfaces    json,receipts,cachePaths,artifactIds hiddenFromDefault=true",
        "retired-routes    agent-prefixed-guide,search-wrapper-hook-query,ts-query,syntax-query rejectedFromGuides=true",
        "stable, low-token, and executable through",
    ]

    missing_terms = [term for term in required_terms if term not in text]

    assert missing_terms == []
