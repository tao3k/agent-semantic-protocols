"""Runtime dependency boundary gate for tree-sitter rollout."""

from __future__ import annotations

import re
from pathlib import Path

from .contract_support import ContractFailure, ROOT, root_relative


MANIFESTS_WITHOUT_TREE_SITTER_RUNTIME = (
    ROOT / "languages/rust-lang-project-harness/Cargo.toml",
    ROOT / "languages/typescript-lang-project-harness/package.json",
    ROOT / "languages/python-lang-project-harness/pyproject.toml",
)
TREE_SITTER_DEPENDENCY_RE = re.compile(
    r'(^|["\s])(@?tree-sitter[^"\s]*|tree_sitter[^"\s]*)(["\s]*:|\s*=)',
)


def check_runtime_boundary(_env: dict[str, str], _asp_bin: str) -> None:
    violations = [
        violation
        for manifest in MANIFESTS_WITHOUT_TREE_SITTER_RUNTIME
        for violation in _manifest_tree_sitter_violations(manifest)
    ]
    if violations:
        raise ContractFailure(
            "language providers must not depend on tree-sitter runtime packages; "
            "ASP owns tree-sitter query ABI/runtime\n" + "\n".join(violations)
        )


def _manifest_tree_sitter_violations(manifest: Path) -> list[str]:
    if not manifest.is_file():
        return []
    return [
        f"{root_relative(manifest)}:{line_number}: {line}"
        for line_number, line in enumerate(
            manifest.read_text(encoding="utf-8").splitlines(), 1
        )
        if TREE_SITTER_DEPENDENCY_RE.search(line)
    ]
