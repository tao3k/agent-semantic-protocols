"""Provider registry runtime tests."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path
from typing import Any


_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(_ROOT / "packages/python/tools/src"))

from tools.provider_registry_runtime import provider_registry_with_env  # noqa: E402


def _doctor_result(payload: dict[str, Any]) -> subprocess.CompletedProcess[str]:
    return subprocess.CompletedProcess(
        args=["/bin/asp"],
        returncode=0,
        stdout=json.dumps(payload),
        stderr="",
    )


def test_provider_registry_accepts_direct_registry(monkeypatch, tmp_path: Path) -> None:
    registry = {"registryId": "registry", "languages": []}
    monkeypatch.setattr(
        "tools.provider_registry_runtime.subprocess.run",
        lambda *_args, **_kwargs: _doctor_result(registry),
    )

    result = provider_registry_with_env(
        "/bin/asp",
        "rust",
        tmp_path,
        env={},
    )

    assert result.error is None
    assert result.registry == registry


def test_provider_registry_unwraps_doctor_receipt(monkeypatch, tmp_path: Path) -> None:
    registry = {"registryId": "registry", "languages": []}
    doctor_receipt = {
        "schemaId": "agent.semantic-protocols.semantic-provider-doctor",
        "registry": registry,
    }
    monkeypatch.setattr(
        "tools.provider_registry_runtime.subprocess.run",
        lambda *_args, **_kwargs: _doctor_result(doctor_receipt),
    )

    result = provider_registry_with_env(
        "/bin/asp",
        "typescript",
        tmp_path,
        env={},
    )

    assert result.error is None
    assert result.registry == registry
