from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
CI_WORKFLOW = REPO_ROOT / ".github" / "workflows" / "ci.yml"
JUSTFILE = REPO_ROOT / "Justfile"

LANGUAGE_RELEASE_WORKFLOWS = {
    "languages/rust-lang-project-harness": {
        "binary": "rs-harness",
        "darwin_os": "macos-14",
        "targets": {
            "x86_64-unknown-linux-gnu",
            "aarch64-apple-darwin",
            "x86_64-pc-windows-msvc",
        },
    },
    "languages/typescript-lang-project-harness": {
        "binary": "ts-harness",
        "darwin_os": "ubuntu-latest",
        "targets": {
            "x86_64-unknown-linux-gnu",
            "aarch64-apple-darwin",
        },
    },
    "languages/python-lang-project-harness": {
        "binary": "py-harness",
        "darwin_os": "ubuntu-latest",
        "targets": {
            "x86_64-unknown-linux-gnu",
            "aarch64-apple-darwin",
        },
    },
    "languages/JuliaLangProjectHarness.jl": {
        "binary": "asp-julia-harness",
        "darwin_os": "macos-14",
        "targets": {
            "x86_64-unknown-linux-gnu",
            "aarch64-apple-darwin",
        },
    },
    "languages/gerbil-scheme-language-project-harness": {
        "binary": "gerbil-scheme-harness",
        "darwin_os": "ubuntu-latest",
        "targets": {
            "x86_64-unknown-linux-gnu",
            "aarch64-apple-darwin",
        },
    },
    "languages/orgize": {
        "binary": "orgize",
        "darwin_os": "macos-14",
        "targets": {
            "x86_64-unknown-linux-gnu",
            "aarch64-apple-darwin",
            "x86_64-pc-windows-msvc",
        },
    },
}


def test_language_release_workflows_are_project_owned_and_publish_assets() -> None:
    for language_path, contract in LANGUAGE_RELEASE_WORKFLOWS.items():
        workflow_path = REPO_ROOT / language_path / ".github" / "workflows" / "release.yml"
        assert workflow_path.exists(), language_path

        workflow = workflow_path.read_text(encoding="utf-8")

        assert "name: Release provider binary" in workflow
        assert "workflow_dispatch:" in workflow
        assert "release:" in workflow
        assert "types:" in workflow
        assert "- published" in workflow
        assert "push:" in workflow
        assert "tags:" in workflow
        assert '- "v*"' in workflow
        assert "permissions:\n  contents: write" in workflow
        assert f"BINARY: {contract['binary']}" in workflow
        assert "github.event.release.tag_name || inputs.tag || github.ref_name" in workflow
        assert "- name: Ensure release tag" in workflow
        assert "if: github.event_name == 'workflow_dispatch'" in workflow
        assert "release tag must start with v" in workflow
        assert 'git push origin "refs/tags/${RELEASE_TAG}"' in workflow
        assert "gh release create" in workflow
        assert "gh release upload" in workflow
        assert "--clobber" in workflow
        assert ".sha256" in workflow
        assert "x86_64-apple-darwin" not in workflow

        if "x86_64-pc-windows-msvc" in contract["targets"]:
            assert "- name: Enable Windows long paths" in workflow
            assert "git config --global core.longpaths true" in workflow
            assert 'CARGO_NET_GIT_FETCH_WITH_CLI=true' in workflow
            build_step = workflow.split("- name: Build release binary", 1)[1]
            build_step = build_step.split("- name: Package provider binary", 1)[0]
            assert "shell: bash" in build_step

        for target in contract["targets"]:
            assert target in workflow, f"{language_path} missing {target}"

        assert (
            f"- os: {contract['darwin_os']}\n"
            "            target: aarch64-apple-darwin"
        ) in workflow


def test_asp_rust_ci_checks_out_provider_catalog_submodules() -> None:
    workflow = CI_WORKFLOW.read_text(encoding="utf-8")

    rust_checkout_step = workflow.split("- name: Checkout provider catalog submodules", 1)[1]
    rust_checkout_step = rust_checkout_step.split("- name: Setup Rust", 1)[0]
    schema_checkout_step = workflow.split("- name: Checkout provider submodules", 1)[1]
    schema_checkout_step = schema_checkout_step.split("- name: Install uv", 1)[0]

    for checkout_step in (rust_checkout_step, schema_checkout_step):
        assert "languages/JuliaLangProjectHarness.jl" in checkout_step
        assert "languages/gerbil-scheme-language-project-harness" in checkout_step


def test_tree_sitter_contract_gate_uses_packaged_cli() -> None:
    workflow = CI_WORKFLOW.read_text(encoding="utf-8")

    assert 'tools/run-tree-sitter-query-contracts.sh' not in workflow
    assert (
        "uv run --project packages/python --frozen python -m tools "
        "tree-sitter validate contracts"
    ) in workflow


def test_language_evidence_ci_hot_path_stays_core_fast() -> None:
    workflow = CI_WORKFLOW.read_text(encoding="utf-8")

    step = workflow.split("- name: Language evidence and facade smoke gate", 1)[1]
    step = step.split("- name: Tree-sitter query contract gates", 1)[0]

    assert "ASP_LANGUAGE_EVIDENCE_SMOKE_SCOPE=core-fast" in step
    assert "ASP_LANGUAGE_EVIDENCE_LANGUAGES=rust,python,typescript" in step
    assert "language-evidence-smoke-core-fast.json" in step
    assert "asp-julia-harness" not in step
    assert "agent-tools-install-julia" not in step


def test_julia_full_provider_gate_uses_fresh_compiled_harness_perf_guard() -> None:
    justfile = JUSTFILE.read_text(encoding="utf-8")

    install_julia = justfile.split('agent-tools-install-jl bin_dir="":', 1)[1]
    install_julia = install_julia.split("agent-hooks-doctor-providers:", 1)[0]
    assert 'find "{{julia_harness_project}}/src"' in install_julia
    assert '"{{julia_harness_project}}/juliac"' in install_julia
    assert '"{{julia_harness_project}}/Project.toml"' in install_julia
    assert '-newer "{{julia_compiled_harness}}"' in install_julia
    assert "rm -rf build/juliac-asp-local" in install_julia
    assert "juliac/build_provider.sh" in install_julia
    assert 'install -m 755 "{{julia_compiled_harness}}" "${bin_dir}/asp-julia-harness"' in install_julia

    all_smoke = justfile.split("check-language-evidence-smoke-all-setup:", 1)[1]
    all_smoke = all_smoke.split("provider-gate:", 1)[0]
    assert "just agent-tools-install-julia .bin" in all_smoke
    assert ".bin/asp julia guide {{julia_harness_project}} >/dev/null" in all_smoke
    assert "ASP_LANGUAGE_EVIDENCE_SMOKE_SCOPE=all-providers" in all_smoke
    assert "ASP_LANGUAGE_EVIDENCE_MAX_COMMAND_SECONDS_JULIA=2" in all_smoke

    provider_gate_julia = justfile.split("provider-gate-julia:", 1)[1]
    provider_gate_julia = provider_gate_julia.split("provider-gate-semantic-facts-setup:", 1)[0]
    assert "just check-language-evidence-smoke-all" in provider_gate_julia
