"""Executable tree-sitter ABI rollout gates owned by the Python tools package."""

from __future__ import annotations

import argparse
import os
import shutil
import stat
import tempfile
from collections.abc import Callable
from pathlib import Path

from tools.console import emit
from tools.provider_registry_contracts import validate_provider_registries
from tools.tree_sitter.contract_exact_read import check_exact_direct_read_contract
from tools.tree_sitter.contract_frontier_code import check_frontier_code_contract
from tools.tree_sitter.contract_query_corpus import check_query_corpus_contracts
from tools.tree_sitter.contract_runtime_boundary import check_runtime_boundary
from tools.tree_sitter.contract_search_read_plan import (
    check_search_read_plan_frontier_contract,
)
from tools.tree_sitter.contract_support import ContractFailure, ROOT, run


Gate = Callable[[dict[str, str], str], None]


def main(argv: list[str] | None = None) -> int:
    args = _parse_args(argv)
    asp_bin = Path(args.asp_bin).resolve() if args.asp_bin else ROOT / "target/debug/asp"
    if args.build:
        _build_runtime(asp_bin)
    with _runtime_env(asp_bin) as env:
        selected = _selected_gates(args.gate)
        for name, gate in selected:
            emit(f"[tree-sitter-contract] gate={name} status=running")
            gate(env, str(asp_bin))
            emit(f"[tree-sitter-contract] gate={name} status=ok")
    emit(f"tree-sitter rollout contracts are valid: gates={len(selected)}")
    return 0


def runtime_boundary_main(argv: list[str] | None = None) -> int:
    _run_single_gate(argv, check_runtime_boundary, "tree-sitter runtime boundary contract is valid")
    return 0


def frontier_code_main(argv: list[str] | None = None) -> int:
    _run_single_gate(argv, check_frontier_code_contract, "tree-sitter frontier/code contract is valid")
    return 0


def search_read_plan_main(argv: list[str] | None = None) -> int:
    _run_single_gate(
        argv,
        check_search_read_plan_frontier_contract,
        "search/read-plan frontier contract is valid",
    )
    return 0


def exact_direct_read_main(argv: list[str] | None = None) -> int:
    _run_single_gate(argv, check_exact_direct_read_contract, "exact direct-read contract is valid")
    return 0


def check_provider_registry_contracts(env: dict[str, str], asp_bin: str) -> None:
    failures = validate_provider_registries(
        ROOT,
        provider_ids=["rust", "typescript", "python"],
        asp_bin=asp_bin,
    )
    if failures:
        raise ContractFailure("\n".join(failures))


def _parse_args(argv: list[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--asp-bin",
        default=None,
        help="Built asp binary to use. Defaults to target/debug/asp after building.",
    )
    parser.add_argument(
        "--no-build",
        dest="build",
        action="store_false",
        help="Skip cargo/npm builds and validate against the current environment.",
    )
    parser.add_argument(
        "--gate",
        action="append",
        choices=tuple(_GATES),
        help="Run only the selected gate. Repeatable.",
    )
    parser.set_defaults(build=True)
    return parser.parse_args(argv)


def _run_single_gate(argv: list[str] | None, gate: Gate, message: str) -> None:
    _ignore_args(argv)
    gate(os.environ.copy(), _resolve_asp())
    emit(message)


def _ignore_args(argv: list[str] | None) -> None:
    if argv:
        raise SystemExit(f"unexpected arguments: {' '.join(argv)}")


def _resolve_asp() -> str:
    return os.environ.get("SEMANTIC_AGENT_PROTOCOL_BIN") or shutil.which("asp") or "asp"


def _selected_gates(names: list[str] | None) -> list[tuple[str, Gate]]:
    if names is None:
        names = list(_GATES)
    return [(name, _GATES[name]) for name in names]


def _build_runtime(asp_bin: Path) -> None:
    run(["npm", "--prefix", "languages/typescript-lang-project-harness", "run", "build"])
    run(["cargo", "build", "-q", "-p", "agent-semantic-protocol", "--bin", "asp"])
    run(
        [
            "cargo",
            "build",
            "-q",
            "--manifest-path",
            "languages/rust-lang-project-harness/Cargo.toml",
            "--features",
            "cli,search",
            "--bin",
            "rs-harness",
        ],
    )
    if not asp_bin.exists():
        raise ContractFailure(f"asp binary not built: {asp_bin}")


class _runtime_env:
    def __init__(self, asp_bin: Path) -> None:
        self.asp_bin = asp_bin
        self._tmp: tempfile.TemporaryDirectory[str] | None = None

    def __enter__(self) -> dict[str, str]:
        self._tmp = tempfile.TemporaryDirectory()
        shim_dir = Path(self._tmp.name)
        _write_shim(
            shim_dir / "rs-harness",
            f'exec "{ROOT}/languages/rust-lang-project-harness/target/debug/rs-harness" "$@"\n',
        )
        _write_shim(
            shim_dir / "ts-harness",
            f'exec node "{ROOT}/languages/typescript-lang-project-harness/dist/src/cli/main.js" "$@"\n',
        )
        _write_shim(
            shim_dir / "py-harness",
            f'exec uv run --project "{ROOT}/languages/python-lang-project-harness" --frozen py-harness "$@"\n',
        )
        env = os.environ.copy()
        env["PATH"] = f"{shim_dir}{os.pathsep}{env.get('PATH', '')}"
        env["SEMANTIC_AGENT_PROTOCOL_BIN"] = str(self.asp_bin)
        run([str(self.asp_bin), "hook", "install", "--client", "codex", "."], env=env)
        return env

    def __exit__(self, *_exc: object) -> None:
        if self._tmp is not None:
            self._tmp.cleanup()


def _write_shim(path: Path, body: str) -> None:
    path.write_text(f"#!/usr/bin/env bash\n{body}", encoding="utf-8")
    path.chmod(path.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)


_GATES: dict[str, Gate] = {
    "provider-registry": check_provider_registry_contracts,
    "runtime-boundary": check_runtime_boundary,
    "frontier-code": check_frontier_code_contract,
    "search-read-plan": check_search_read_plan_frontier_contract,
    "exact-direct-read": check_exact_direct_read_contract,
    "query-corpus": check_query_corpus_contracts,
}


if __name__ == "__main__":
    raise SystemExit(main())
