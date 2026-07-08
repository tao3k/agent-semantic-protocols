"""Executable tree-sitter ABI rollout gates owned by the Python tools package."""

from __future__ import annotations

import argparse
import os
import shutil
import stat
import tempfile
from collections.abc import Callable, Mapping
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

_CORE_FAST_ASP_TOML = """[providers.gerbil-scheme]
enabled = false

[providers.julia]
enabled = false
"""


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
        env=env,
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
    gate(_contract_env(os.environ), _resolve_asp())
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
        self._asp_toml_backup: Path | None = None
        self._codex_config_backup: Path | None = None
        self._hook_config_backup: Path | None = None

    def __enter__(self) -> dict[str, str]:
        self._tmp = tempfile.TemporaryDirectory()
        shim_dir = Path(self._tmp.name)
        try:
            self._asp_toml_backup = _backup_runtime_file(
                shim_dir,
                ROOT / ".agents/asp.toml",
            )
            self._codex_config_backup = _backup_runtime_file(
                shim_dir,
                ROOT / ".codex/config.toml",
            )
            self._hook_config_backup = _backup_runtime_file(
                shim_dir,
                ROOT / ".codex/agent-semantic-protocol/hooks/config.toml",
            )
            (ROOT / ".agents/asp.toml").write_text(_CORE_FAST_ASP_TOML, encoding="utf-8")
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
            env["ASP_NO_AGENT_PLATFORM"] = "1"
            run([str(self.asp_bin), "install", "plugin", "--codex", "."], env=env)
            return env
        except Exception:
            self.__exit__(None, None, None)
            raise

    def __exit__(self, *_exc: object) -> None:
        _restore_runtime_file(ROOT / ".agents/asp.toml", self._asp_toml_backup)
        _restore_runtime_file(ROOT / ".codex/config.toml", self._codex_config_backup)
        _restore_runtime_file(
            ROOT / ".codex/agent-semantic-protocol/hooks/config.toml",
            self._hook_config_backup,
        )
        if self._tmp is not None:
            self._tmp.cleanup()


def _backup_runtime_file(shim_dir: Path, path: Path) -> Path | None:
    if not path.exists():
        return None
    backup = shim_dir / f"{path.name}.backup"
    shutil.copy2(path, backup)
    return backup


def _restore_runtime_file(path: Path, backup: Path | None) -> None:
    if backup is None:
        path.unlink(missing_ok=True)
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(backup, path)


def _write_shim(path: Path, body: str) -> None:
    path.write_text(f"#!/usr/bin/env bash\n{body}", encoding="utf-8")
    path.chmod(path.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)


def _contract_env(source: Mapping[str, str]) -> dict[str, str]:
    env = dict(source)
    env["ASP_NO_AGENT_PLATFORM"] = "1"
    return env


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
