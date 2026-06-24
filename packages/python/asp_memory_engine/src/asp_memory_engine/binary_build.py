"""Build a PATH-executable ASP memory engine artifact."""

from __future__ import annotations

import os
import stat
import zipapp
from pathlib import Path

DEFAULT_BINARY_INTERPRETER = "/usr/bin/env -S python3 -S"
DEFAULT_BINARY_MAIN = "asp_memory_engine.cli:main"


def build_memory_engine_binary(
    output: str | os.PathLike[str],
    *,
    source_root: str | os.PathLike[str] | None = None,
    interpreter: str = DEFAULT_BINARY_INTERPRETER,
    compressed: bool = False,
) -> Path:
    """Build an executable zipapp that can be used as `asp-memory-engine`.

    The resulting artifact is intentionally independent from `.venv` console
    scripts. It can be put on PATH or passed through ASP_MEMORY_ENGINE for
    end-to-end performance runs. The default shebang uses `python3 -S` and the
    archive is uncompressed because cold-start latency matters more than site
    initialization or artifact size for agent recall.
    """

    output_path = Path(output)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    src_root = Path(source_root) if source_root is not None else _default_source_root()
    zipapp.create_archive(
        src_root,
        target=output_path,
        interpreter=interpreter,
        main=DEFAULT_BINARY_MAIN,
        compressed=compressed,
    )
    _make_executable(output_path)
    return output_path


def _default_source_root() -> Path:
    return Path(__file__).resolve().parents[1]


def _make_executable(path: Path) -> None:
    mode = path.stat().st_mode
    path.chmod(mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
