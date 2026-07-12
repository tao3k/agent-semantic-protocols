"""Large-library runtime benchmark receipt tests."""

from __future__ import annotations

import json
import os
from pathlib import Path
import sys
import time

import pytest

from jsonschema import Draft202012Validator

from tools.semantic_sandtable.large_library_runtime_benchmark import (
    run_large_library_runtime_benchmark,
)
from tools.semantic_sandtable.large_library_runtime_deployment import (
    install_workspace_provider,
)
from tools.semantic_sandtable.large_library_runtime_invocation import (
    benchmark_command_from_descriptor,
)
from tools.semantic_sandtable.large_library_runtime_process import (
    descendant_process_groups,
    run_public_command,
)
from tools.semantic_sandtable.large_library_runtime_steps import benchmark_fd_step
from tools.semantic_sandtable.large_library_runtime_types import Corpus


_ROOT = Path(__file__).resolve().parents[3]
_SCHEMA = _ROOT / "schemas/semantic-sandtable-large-library-runtime-benchmark.v1.schema.json"
_CORPUS_SCHEMA = _ROOT / "schemas/semantic-sandtable-large-library-corpora.v1.schema.json"
_CORPUS_MANIFEST = _ROOT / "benchmarks/large-library-runtime-corpora.v1.json"


def test_runtime_benchmark_rejects_missing_release_binary_and_corpora(
    tmp_path: Path,
) -> None:
    receipt = run_large_library_runtime_benchmark(
        _ROOT,
        asp_binary=tmp_path / "missing-asp",
        corpus_root=tmp_path / "corpora",
    )

    Draft202012Validator(json.loads(_SCHEMA.read_text(encoding="utf-8"))).validate(
        receipt
    )
    assert receipt["status"] == "fail"
    assert receipt["binary"]["releaseVerified"] is False
    assert receipt["workspaceDeployments"] == []
    assert receipt["commandCoverage"] == {
        "registeredSearchMethodCount": 0,
        "targetSearchCommandCount": 0,
        "runtimeSearchCommandCount": 0,
        "registeredMethods": [],
        "runtimeMethods": [],
        "missingMethods": [],
        "missingCorpusMethods": [],
    }
    assert len(receipt["missingCorpora"]) == 14
    assert {entry["repository"] for entry in receipt["missingCorpora"]} >= {
        "JuliaData/DataFrames.jl",
        "fastapi/fastapi",
        "tokio-rs/tokio",
        "microsoft/TypeScript",
    }


def test_runtime_benchmark_uses_provider_template_through_public_language_facade() -> None:
    invocation = benchmark_command_from_descriptor(
        {
            "method": "search/lexical",
            "view": "lexical",
            "benchmarkInvocation": {
                "args": [
                    "search",
                    "lexical",
                    "--query",
                    "{query}",
                    "--workspace",
                    "{workspace}",
                    "--view",
                    "seeds",
                ],
                "expectsJson": False,
                "maxElapsedMs": 500,
            },
        },
        "python",
        {"workspace": "/tmp/python-large", "owner": "pkg/router.py", "query": "router", "dependency": "pydantic"},
    )

    assert invocation.command == [
        "asp",
        "python",
        "search",
        "lexical",
        "--query",
        "router",
        "--workspace",
        "/tmp/python-large",
        "--view",
        "seeds",
    ]


def test_runtime_benchmark_executes_fd_as_an_independent_path_stage(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    calls: list[list[str]] = []

    def fake_run(command: list[str], **_kwargs: object) -> object:
        calls.append(command)
        return type(
            "Completed",
            (),
            {
                "timed_out": False,
                "returncode": 0,
                "stdout": "path=src/worker-count.rs\n",
                "stderr": "",
                "process_tree_terminated": False,
            },
        )()

    monkeypatch.setattr(
        "tools.semantic_sandtable.large_library_runtime_steps.run_public_command",
        fake_run,
    )
    corpus = Corpus(
        scenario_id="runtime-fd-stage",
        language="rust",
        repository="example/runtime-fd-stage",
        directory="runtime-fd-stage",
        environment="ASP_RUNTIME_FD_STAGE",
        inputs={"owner": "src/lib.rs", "query": "worker-count", "dependency": "tokio"},
    )

    step = benchmark_fd_step(tmp_path / "asp", corpus, tmp_path / "workspace")

    assert step["status"] == "pass"
    assert step["method"] == "search/fd-path"
    assert calls == [
        [
            str(tmp_path / "asp"),
            "fd",
            "-query",
            "worker-count",
            "--workspace",
            str(tmp_path / "workspace"),
        ]
    ]


def test_runtime_corpus_manifest_has_all_unique_real_library_targets() -> None:
    manifest = json.loads(_CORPUS_MANIFEST.read_text(encoding="utf-8"))

    Draft202012Validator(json.loads(_CORPUS_SCHEMA.read_text(encoding="utf-8"))).validate(
        manifest
    )
    corpora = manifest["corpora"]
    assert len(corpora) == 14
    assert {entry["repository"] for entry in corpora} == {
        "JuliaData/DataFrames.jl",
        "FluxML/Flux.jl",
        "MakieOrg/Makie.jl",
        "fastapi/fastapi",
        "pandas-dev/pandas",
        "Textualize/rich",
        "tokio-rs/tokio",
        "tokio-rs/bytes",
        "BurntSushi/ripgrep",
        "microsoft/TypeScript",
        "Effect-TS/effect",
        "microsoft/playwright",
        "vitejs/vite",
        "vuejs/core",
    }


def test_runtime_workspace_deployment_uses_release_install_command(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    calls: list[dict[str, object]] = []

    def fake_run(command: list[str], **kwargs: object) -> object:
        calls.append({"command": command, **kwargs})
        return type("Completed", (), {"returncode": 0, "stdout": "installed", "stderr": ""})()

    monkeypatch.setattr(
        "tools.semantic_sandtable.large_library_runtime_deployment.subprocess.run",
        fake_run,
    )

    deployment = install_workspace_provider(tmp_path / "asp", tmp_path, "python")

    assert deployment["command"] == [
        "asp",
        "install",
        "language",
        "python",
        "--from-workspace",
        "--project",
        str(tmp_path),
    ]
    assert deployment["status"] == "pass"
    assert deployment["errors"] == []
    assert len(calls) == 1
    call = calls[0]
    assert call["command"] == [
        str(tmp_path / "asp"),
        "install",
        "language",
        "python",
        "--from-workspace",
        "--project",
        str(tmp_path),
    ]
    assert call["cwd"] == tmp_path
    assert call["text"] is True
    assert call["capture_output"] is True
    assert call["check"] is False
    assert call["timeout"] == 120
    assert isinstance(call["env"], dict)
    assert call["env"]["ASP_NO_AGENT_PLATFORM"] == "1"


def test_runtime_process_listing_failure_is_explicit(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    def denied_process_listing(*_args: object, **_kwargs: object) -> object:
        raise PermissionError("ps is unavailable")

    monkeypatch.setattr(
        "tools.semantic_sandtable.large_library_runtime_process.subprocess.run",
        denied_process_listing,
    )

    assert descendant_process_groups(12345) == (set(), False)


@pytest.mark.skipif(os.name != "posix", reason="process-group cleanup is POSIX-owned")
def test_runtime_timeout_terminates_provider_process_tree(tmp_path: Path) -> None:
    _, process_listing_available = descendant_process_groups(os.getpid())
    if not process_listing_available:
        pytest.skip("sandbox does not permit process-tree enumeration")
    child_pid = tmp_path / "provider-child.pid"
    child_program = (
        "from pathlib import Path; import os, sys, time; os.setpgrp(); "
        "Path(sys.argv[1]).write_text(str(os.getpid()), encoding='utf-8'); time.sleep(60)"
    )
    parent_program = (
        "import subprocess, sys, time; "
        "subprocess.Popen([sys.executable, '-c', sys.argv[1], sys.argv[2]]); time.sleep(60)"
    )

    result = run_public_command(
        [sys.executable, "-c", parent_program, child_program, str(child_pid)],
        timeout_seconds=1,
        env=dict(os.environ),
    )

    assert result.timed_out is True
    assert result.process_tree_terminated is True
    pid = int(child_pid.read_text(encoding="utf-8"))
    deadline = time.monotonic() + 2
    while time.monotonic() < deadline:
        try:
            os.kill(pid, 0)
        except ProcessLookupError:
            break
        time.sleep(0.05)
    else:
        pytest.fail(f"provider child process survived timeout cleanup: {pid}")
