"""Case matrix for the cross-language workspace/search contract gate."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class SearchContractCase:
    language: str
    project_root: str
    ingest_pipes: tuple[str, ...]
    accepted_pipes_json: str
    workspace_needles: tuple[str, ...] = ()
    workspace_router_next_prime: bool = False


CONTRACT_CASES: tuple[SearchContractCase, ...] = (
    SearchContractCase(
        language="rust",
        project_root="languages/asp-rust",
        ingest_pipes=("items", "tests"),
        accepted_pipes_json='"acceptedPipes":["items","tests"]',
        workspace_needles=(
            "aliases: graph:{G=search,P=package}",
            "P=package:pkg(.)",
        ),
    ),
    SearchContractCase(
        language="typescript",
        project_root="languages/typescript-lang-project-harness",
        ingest_pipes=("items", "tests"),
        accepted_pipes_json='"acceptedPipes":["items","tests"]',
        workspace_needles=("O=owner:path(.)!owner",),
    ),
    SearchContractCase(
        language="python",
        project_root="languages/asp-python",
        ingest_pipes=("items", "tests"),
        accepted_pipes_json='"acceptedPipes":["items","tests"]',
        workspace_needles=("O=owner:path(.)!owner",),
        workspace_router_next_prime=True,
    ),
    SearchContractCase(
        language="julia",
        project_root="languages/AspJulia.jl",
        ingest_pipes=("owner", "tests"),
        accepted_pipes_json='"acceptedPipes":["owner","tests"]',
        workspace_needles=("aliases: graph:{G=search,O=owner}",),
    ),
)
