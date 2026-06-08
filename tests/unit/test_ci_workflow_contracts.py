from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
CI_WORKFLOW = REPO_ROOT / ".github" / "workflows" / "ci.yml"


def test_asp_rust_ci_checks_out_julia_query_catalog() -> None:
    workflow = CI_WORKFLOW.read_text(encoding="utf-8")

    rust_checkout_step = workflow.split("- name: Checkout provider catalog submodules", 1)[1]
    rust_checkout_step = rust_checkout_step.split("- name: Setup Rust", 1)[0]

    assert 'languages/JuliaLangProjectHarness.jl' in rust_checkout_step


def test_tree_sitter_contract_gate_uses_packaged_cli() -> None:
    workflow = CI_WORKFLOW.read_text(encoding="utf-8")

    assert 'tools/run-tree-sitter-query-contracts.sh' not in workflow
    assert (
        "uv run --project packages/python --frozen python -m tools "
        "tree-sitter validate contracts"
    ) in workflow
