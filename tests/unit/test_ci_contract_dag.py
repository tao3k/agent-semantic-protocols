# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Validate the artifact and ownership boundaries in the CI contract DAG."""

from pathlib import Path


CI_WORKFLOW = Path(__file__).resolve().parents[2] / ".github" / "workflows" / "ci.yml"


def test_contract_gates_form_a_parallel_library_dag_without_standalone_asp_binary() -> None:
    workflow = CI_WORKFLOW.read_text(encoding="utf-8")

    assert "cargo build --bin asp" not in workflow
    assert "orgize-ffi" not in workflow
    assert "  shared-contract-gates:" in workflow
    assert "  python-provider-gates:" in workflow
    assert "  catalog-provider-gates:" in workflow
    assert "  language-facade-gates:" in workflow
    assert "  tree-sitter-query-contract-gates:" in workflow

    python_provider = workflow.split("  python-provider-gates:", 1)[1].split(
        "  catalog-provider-gates:", 1,
    )[0]
    assert "languages/asp-python" in python_provider
    assert "scripts/providers_sync.py sync --workspace . --package asp-python" in python_provider
    assert "cargo test" not in python_provider

    catalog_provider = workflow.split("  catalog-provider-gates:", 1)[1].split(
        "  language-facade-gates:", 1,
    )[0]
    assert (
        "canonical_client_profile_publishes_the_shared_schema_bundle_route"
        in catalog_provider
    )
    assert "agent-semantic-runtime-server" not in catalog_provider

    language_facade = workflow.split("  language-facade-gates:", 1)[1].split(
        "  tree-sitter-query-contract-gates:", 1,
    )[0]
    assert "Language facade smoke gate" in language_facade

    query_contracts = workflow.split("  tree-sitter-query-contract-gates:", 1)[1]
    assert "--asp-bin" not in query_contracts
    assert "--gate runtime-boundary" in query_contracts
    assert "--gate query-corpus" in query_contracts
    assert "--no-build" in query_contracts
    assert "cargo test" not in query_contracts


def test_asp_rust_workspace_is_a_pure_library_workspace_with_zero_cli_binaries() -> None:
    crates_root = CI_WORKFLOW.parents[2] / "crates"
    main_files = sorted(crates_root.glob("*/src/main.rs"))
    bin_dirs = sorted(crates_root.glob("*/src/bin/*.rs"))
    manifests_with_bins = [
        manifest
        for manifest in sorted(crates_root.glob("*/Cargo.toml"))
        if "[[bin]]" in manifest.read_text(encoding="utf-8")
    ]
    assert main_files == []
    assert bin_dirs == []
    assert manifests_with_bins == []
