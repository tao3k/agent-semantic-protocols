from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = REPO_ROOT / ".github" / "workflows" / "large-library-runtime-benchmark.yml"


def test_v1_large_library_runtime_workflow_is_pinned_and_receipted() -> None:
    workflow = WORKFLOW.read_text(encoding="utf-8")

    assert "name: Large Library Runtime Benchmark" in workflow
    assert "workflow_dispatch:" in workflow
    assert "schedule:" in workflow
    assert "direnv exec ." in workflow
    assert "source_index_1193_owner_cold_write_stays_inside_v1_gate" in workflow
    assert "cargo test --release" in workflow
    assert "benchmark-large-library-search-runtime-baseline" in workflow
    assert "large-library-runtime-search.v1.baseline.json" in workflow
    assert "git clone --filter=blob:none" in workflow
    assert "actions/upload-artifact@v4" in workflow
    assert "large-library-runtime-search.v1.receipt.json" in workflow
