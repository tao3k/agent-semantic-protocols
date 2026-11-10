# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = REPO_ROOT / ".github" / "workflows" / "large-library-runtime-benchmark.yml"


def test_v1_large_library_runtime_workflow_is_pinned_and_receipted() -> None:
    workflow = WORKFLOW.read_text(encoding="utf-8")

    assert "name: Large Library Runtime Benchmark" in workflow
    assert "workflow_dispatch:" in workflow
    assert "schedule:" in workflow
    assert "runtime-artifacts:" in workflow
    assert "provider-artifacts:" in workflow
    assert "Build V1 Runtime artifacts once" in workflow
    assert "Build provider workspace artifact without publication" in workflow
    benchmark = workflow.split("  benchmark:", 1)[1]
    assert "cargo " not in benchmark
    assert "agent-tools-install" not in benchmark
    assert "actions/download-artifact@v4" in benchmark
    assert '"$runner" sync' in benchmark
    assert '"$runner" materialize' in benchmark
    assert '"$runner" qualify' in benchmark
    assert "ASP_LIVE_CORPUS_SERVER_ARTIFACT" in benchmark
    assert "actions/upload-artifact@v4" in workflow
    assert "search-query-qualification/by-resource" in workflow
