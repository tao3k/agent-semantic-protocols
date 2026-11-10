# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Keep query contracts independent from the complete ASP client binary."""

from pathlib import Path

from tools.tree_sitter import contract_gates
from tools.tree_sitter.contract_query_corpus import QUERY_CORPUS_COMMANDS


class _RecordedRuntimeEnv:
    def __init__(self, asp_bin: Path | None, seen: list[Path | None]) -> None:
        self.asp_bin = asp_bin
        seen.append(asp_bin)

    def __enter__(self) -> dict[str, str]:
        return {"contract": "env"}

    def __exit__(self, *_exc: object) -> None:
        return None


def test_query_corpus_contains_only_language_owned_validators() -> None:
    commands = [" ".join(command) for command in QUERY_CORPUS_COMMANDS]
    assert len(commands) == 3
    assert any("rust-query-corpus" in command for command in commands)
    assert any("typescript-query-corpus" in command for command in commands)
    assert any("python-query-corpus" in command for command in commands)
    assert all("cargo test" not in command for command in commands)


def test_query_only_gate_neither_builds_nor_exports_complete_asp(monkeypatch) -> None:
    built: list[Path | None] = []
    invoked: list[tuple[dict[str, str], str]] = []

    monkeypatch.setattr(
        contract_gates,
        "_GATES",
        {"query-corpus": lambda env, asp_bin: invoked.append((env, asp_bin))},
    )
    monkeypatch.setattr(contract_gates, "_build_runtime", built.append)
    monkeypatch.setattr(
        contract_gates,
        "_runtime_env",
        lambda _asp_bin: (_ for _ in ()).throw(
            AssertionError(
                "query-only gates must not enter the provider runtime environment"
            )
        ),
    )
    monkeypatch.setattr(contract_gates, "emit", lambda _message: None)

    assert contract_gates.main(["--gate", "query-corpus"]) == 0
    assert built == [None]
    assert invoked == [(contract_gates._contract_env(contract_gates.os.environ), "")]


def test_provider_registry_gate_retains_complete_asp_dependency(
    monkeypatch, tmp_path: Path
) -> None:
    environments: list[Path | None] = []
    invoked: list[tuple[dict[str, str], str]] = []
    asp_bin = tmp_path / "asp"

    monkeypatch.setattr(
        contract_gates,
        "_GATES",
        {"provider-registry": lambda env, path: invoked.append((env, path))},
    )
    monkeypatch.setattr(
        contract_gates,
        "_runtime_env",
        lambda path: _RecordedRuntimeEnv(path, environments),
    )
    monkeypatch.setattr(contract_gates, "emit", lambda _message: None)

    assert (
        contract_gates.main(
            ["--gate", "provider-registry", "--asp-bin", str(asp_bin), "--no-build"]
        )
        == 0
    )
    resolved = asp_bin.resolve()
    assert environments == [resolved]
    assert invoked == [({"contract": "env"}, str(resolved))]
