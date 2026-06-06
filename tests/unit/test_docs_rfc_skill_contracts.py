"""Documentation contract checks for RFC ownership and generated skills."""

from pathlib import Path

_REPO_ROOT = Path(__file__).resolve().parents[2]
_README_PATH = _REPO_ROOT / "README.md"
_JUSTFILE_PATH = _REPO_ROOT / "Justfile"
_CI_PATH = _REPO_ROOT / ".github" / "workflows" / "ci.yml"
_ROADMAP_PATH = (
    _REPO_ROOT
    / "docs"
    / "30-39-research"
    / "31.18-tree-sitter-query-rfc-roadmap.org"
)
_ROOT_SKILL_PATH = _REPO_ROOT / "SKILL.md"
_INSTALLED_SKILL_PATH = (
    _REPO_ROOT / ".agents" / "skills" / "agent-semantic-protocols" / "SKILL.md"
)
_ACTIVE_DOC_PATHS = [
    _README_PATH,
    _ROOT_SKILL_PATH,
    _INSTALLED_SKILL_PATH,
    _REPO_ROOT / "rfcs" / "agent-hook-interception-protocol.org",
    _REPO_ROOT / "rfcs" / "cli-first-harness-ux.org",
    _REPO_ROOT / "rfcs" / "semantic-tree-sitter-query-protocol.org",
    _REPO_ROOT / "schemas" / "README.md",
    _ROADMAP_PATH,
]


def test_readme_points_to_rfc_and_docs_owners() -> None:
    text = _README_PATH.read_text(encoding="utf-8")

    required_terms = [
        "## Documentation Map",
        "rfcs/semantic-tree-sitter-query-protocol.org",
        "tree-sitter-compatible syntax ABI",
        "rfcs/cli-first-harness-ux.org",
        "asp <language> guide",
        "rfcs/agent-hook-interception-protocol.org",
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


def test_root_skill_template_and_installed_skill_contract() -> None:
    root_skill = _ROOT_SKILL_PATH.read_text(encoding="utf-8")
    installed_skill = _INSTALLED_SKILL_PATH.read_text(encoding="utf-8")

    assert "<!-- ASP_INSTALLED_SKILL_NOTICE -->" in root_skill
    assert "<!-- ASP_PROVIDER_SUMMARY -->" in root_skill
    assert "Do not edit this installed copy" not in root_skill

    assert "Do not edit this installed copy" in installed_skill
    assert "## Active Providers" in installed_skill
    assert "Start with `asp <language> guide .`" in installed_skill
    assert "path-context resolution" in root_skill
    assert "path-context resolution" in installed_skill
    assert "workspace discovery" not in root_skill
    assert "<!-- ASP_INSTALLED_SKILL_NOTICE -->" not in installed_skill
    assert "<!-- ASP_PROVIDER_SUMMARY -->" not in installed_skill
    assert "asp <language> agent guide" not in installed_skill
    assert str(_REPO_ROOT) not in installed_skill
    assert "| `/" not in installed_skill


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
        "SKILL.org",
    ]

    violations = []
    for path in _ACTIVE_DOC_PATHS:
        text = path.read_text(encoding="utf-8")
        for term in retired_terms:
            if term in text:
                violations.append(f"{path.relative_to(_REPO_ROOT)}: {term}")

    assert violations == []
