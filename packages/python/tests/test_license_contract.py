# SPDX-FileCopyrightText: Contributors to Agent Semantic Protocols
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only
"""Verify Python distribution license metadata and packaged declarations."""

from __future__ import annotations

import importlib.util
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "check_license_contract.py"


def _license_contract_module():
    spec = importlib.util.spec_from_file_location("check_license_contract", SCRIPT)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_all_python_projects_use_the_required_dual_license() -> None:
    contract = _license_contract_module()

    assert contract.license_project_files(ROOT) == (
        ROOT / "asp_memory_engine" / "pyproject.toml",
        ROOT / "asp_python_graphs" / "pyproject.toml",
        ROOT / "asp_schema_manager" / "pyproject.toml",
        ROOT / "pyproject.toml",
        ROOT / "tools" / "pyproject.toml",
    )
    assert contract.validate_license_contract(ROOT) == []


# REUSE-IgnoreStart
def test_or_expression_is_rejected(tmp_path: Path) -> None:
    contract = _license_contract_module()
    (tmp_path / "pyproject.toml").write_text(
        "\n".join(
            (
                "# SPDX-FileCopyrightText: Contributors to Agent Semantic Protocols",
                "#",
                "# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only",
                "",
                "[project]",
                'name = "invalid-license-fixture"',
                'version = "0.0.0"',
                'license = "Apache-2.0 OR LGPL-2.1-only"',
                'license-files = ["LICENSE"]',
            )
        ),
        encoding="utf-8",
    )
    (tmp_path / "REUSE.toml").write_text("version = 1\n", encoding="utf-8")
    (tmp_path / "LICENSE").write_text(
        "\n".join(
            (
                "SPDX-License-Identifier: Apache-2.0 OR LGPL-2.1-only",
                "The AND operator is intentional",
                "Apache License",
                "Version 2.0, January 2004",
                "GNU LESSER GENERAL PUBLIC LICENSE",
                "Version 2.1, February 1999",
            )
        ),
        encoding="utf-8",
    )

    assert contract.validate_license_contract(tmp_path) == [
        "pyproject.toml: project.license must equal "
        "'Apache-2.0 AND LGPL-2.1-only'",
        "LICENSE: invalid SPDX declaration",
    ]
# REUSE-IgnoreEnd
