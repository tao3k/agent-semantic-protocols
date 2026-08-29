"""Project-root inference for artifact timeline events."""

from __future__ import annotations

from pathlib import Path


def artifact_workspace_root(root: Path) -> Path:
    resolved = root.resolve()
    for candidate in (resolved, *resolved.parents):
        if candidate.name == ".cache":
            return candidate.parent
    return resolved if resolved.is_dir() else resolved.parent


def artifact_infer_project_root(workspace_root_path: Path, target: str) -> str:
    target_path = _target_path(target)
    if target_path is None or target_path.is_absolute():
        return ""
    matches = [
        candidate
        for candidate in _candidate_project_roots(workspace_root_path)
        if (candidate / target_path).exists()
    ]
    unique = sorted({match.resolve() for match in matches})
    return str(unique[0]) if len(unique) == 1 else ""


def artifact_project_root_arg(project_root: str, workspace_root_path: Path) -> str:
    if not project_root:
        return ""
    root_path = Path(project_root).resolve()
    workspace = workspace_root_path.resolve()
    try:
        relative = root_path.relative_to(workspace)
    except ValueError:
        return str(root_path)
    value = relative.as_posix()
    return value or "."


def _candidate_project_roots(workspace_root_path: Path) -> tuple[Path, ...]:
    roots = [workspace_root_path]
    for base in (
        workspace_root_path / "languages",
        workspace_root_path / "packages" / "python",
    ):
        if not base.is_dir():
            continue
        roots.extend(child for child in sorted(base.iterdir()) if child.is_dir())
    return tuple(roots)


def _target_path(target: str) -> Path | None:
    value = _strip_locator(target)
    if not value or " " in value:
        return None
    path = Path(value)
    if "/" not in value and "." not in path.name:
        return None
    return path


def _strip_locator(value: str) -> str:
    if ":" not in value:
        return value
    head, tail = value.rsplit(":", 1)
    if tail.isdigit() or "-" in tail:
        return head
    return value
