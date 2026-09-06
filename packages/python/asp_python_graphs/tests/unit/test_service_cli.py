# SPDX-FileCopyrightText: Contributors to Agent Semantic Protocols
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

"""Exercise the project-owned gRPC service subcommand."""

from __future__ import annotations

from pathlib import Path

import pytest

from asp_python_graphs import service_cli


def test_serve_subcommand_forwards_one_absolute_socket_and_capacity(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    observed: list[tuple[Path, int]] = []

    async def fake_serve(socket_path: Path, max_in_flight: int) -> None:
        observed.append((socket_path, max_in_flight))

    monkeypatch.setattr(service_cli, "serve", fake_serve)
    socket_path = tmp_path / "graphs.sock"

    assert (
        service_cli.main(
            ["serve", "--socket", str(socket_path), "--max-in-flight", "7"]
        )
        == 0
    )
    assert observed == [(socket_path, 7)]


def test_serve_subcommand_rejects_relative_socket() -> None:
    with pytest.raises(SystemExit):
        service_cli.main(["serve", "--socket", "graphs.sock"])


def test_service_entry_rejects_algorithm_commands() -> None:
    with pytest.raises(SystemExit):
        service_cli.main(["rank", "--socket", "/tmp/graphs.sock"])
