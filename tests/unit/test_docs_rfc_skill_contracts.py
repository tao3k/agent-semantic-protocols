"""Documentation contract checks for RFC ownership and generated skills."""

from pathlib import Path

_REPO_ROOT = Path(__file__).resolve().parents[2]
_README_PATH = _REPO_ROOT / "README.md"
_JUSTFILE_PATH = _REPO_ROOT / "Justfile"
_CI_PATH = _REPO_ROOT / ".github" / "workflows" / "ci.yml"
_ROADMAP_PATH = (
    _REPO_ROOT / "docs" / "30-39-research" / "31.18-tree-sitter-query-rfc-roadmap.org"
)
_ASP_SKILL_CONTRACT_PATH = (
    _REPO_ROOT / "languages" / "org" / "contracts" / "asp.skill.v1.org"
)
_RETIRED_PLUGIN_SKILL_PATH = (
    _REPO_ROOT
    / "asp-codex-plugin"
    / "skills"
    / "agent-semantic-protocols"
    / "SKILL.org"
)
_ACTIVE_DOC_PATHS = [
    _README_PATH,
    _ASP_SKILL_CONTRACT_PATH,
    _REPO_ROOT / "docs" / "10-19-rfcs" / "10.15-agent-hook-interception-protocol.org",
    _REPO_ROOT / "docs" / "10-19-rfcs" / "10.05-cli-first-harness-ux.org",
    _REPO_ROOT
    / "docs"
    / "10-19-rfcs"
    / "10.11-semantic-tree-sitter-query-protocol.org",
    _REPO_ROOT / "schemas" / "README.md",
    _ROADMAP_PATH,
]


def test_readme_points_to_rfc_and_docs_owners() -> None:
    text = _README_PATH.read_text(encoding="utf-8")
    required_terms = [
        "## Documentation Map",
        "docs/10-19-rfcs/10.11-semantic-tree-sitter-query-protocol.org",
        "tree-sitter-compatible syntax ABI",
        "docs/10-19-rfcs/10.05-cli-first-harness-ux.org",
        "asp <language> guide",
        "docs/10-19-rfcs/10.15-agent-hook-interception-protocol.org",
        "Detected Binaries",
        "schemas/README.md",
        "share tree-sitter provenance without merging packet envelopes",
        "31.18-tree-sitter-query-rfc-roadmap.org",
    ]
    missing_terms = [term for term in required_terms if term not in text]
    assert missing_terms == []


def test_tree_sitter_roadmap_records_closure_plan() -> None:
    text = _ROADMAP_PATH.read_text(encoding="utf-8")
    required_terms = [
        "tree-sitter-compatible native projection",
        "sourceAuthority = native-parser-adapter",
        "Single-capture rows",
        "Field semantics",
        "Pattern graph",
        "Documentation Ownership Matrix",
        "semantic-tree-sitter-provenance.v1",
        "Doc gates",
        "Real evidence",
        "Detected Binaries",
        "experimental.semanticAstPatch.enabled",
    ]
    missing_terms = [term for term in required_terms if term not in text]
    assert missing_terms == []


def test_root_skill_template_contract() -> None:
    root_skill = _ASP_SKILL_CONTRACT_PATH.read_text(encoding="utf-8")

    assert "* asp-skill-v1" in root_skill
    assert "** What This Produces" in root_skill
    assert "** ASP Org" in root_skill
    assert "*** State Workflow" in root_skill
    assert "*** Contract Capture" in root_skill
    assert "ASP_ORG_SKILL.org" in root_skill
    assert "single ASP Org skill entry" in root_skill
    assert "do not reintroduce a wrapper skill" in root_skill
    assert "asp org capture --contract CONTRACT_ID" in root_skill
    assert "Do not edit the audited template output directly" in root_skill
    assert "| Stage | Command | What the agent learns |" not in root_skill
    assert "semantic AST provider boundary" in root_skill
    assert "query AST facts" in root_skill
    assert not _RETIRED_PLUGIN_SKILL_PATH.exists()


def test_rfc_docs_contracts_are_in_local_and_ci_gates() -> None:
    justfile = _JUSTFILE_PATH.read_text(encoding="utf-8")
    ci = _CI_PATH.read_text(encoding="utf-8")
    required_just_terms = [
        "check-rfc-docs:",
        "tests/unit/test_*rfc.py",
        "tests/unit/test_docs_rfc_skill_contracts.py",
        "provider-gate: check-rust-warnings check-schema-profiles check-rfc-docs",
        "tests/unit/test_agent_hook_interception_protocol_rfc.py",
    ]
    missing_just_terms = [term for term in required_just_terms if term not in justfile]
    required_ci_terms = [
        "Root schema gates",
        "tests/unit/semantic_tree_sitter_query_rfc",
        "tests/unit/test_cli_first_harness_ux_rfc.py",
        "tests/unit/test_agent_hook_interception_protocol_rfc.py",
        "tests/unit/test_docs_rfc_skill_contracts.py",
    ]
    missing_ci_terms = [term for term in required_ci_terms if term not in ci]
    assert missing_just_terms == []
    assert missing_ci_terms == []


def test_active_docs_do_not_teach_retired_guide_or_hook_routes() -> None:
    retired_terms = [
        "[agent-guide]",
        "protocol=agent-guide.v1",
        "asp <language> agent guide",
        "search query --from-hook",
    ]

    violations = []
    for path in _ACTIVE_DOC_PATHS:
        text = path.read_text(encoding="utf-8")
        for term in retired_terms:
            if term in text:
                violations.append(f"{path.relative_to(_REPO_ROOT)}: {term}")
    assert violations == []
