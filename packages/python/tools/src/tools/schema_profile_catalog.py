"""Language schema profile catalog."""

from __future__ import annotations

from collections.abc import Iterable
from dataclasses import dataclass


@dataclass(frozen=True)
class LanguageSchemaProfile:
    """Exact schema set allowed under one language provider package."""

    language_id: str
    package_root: str
    shared_schema_files: tuple[str, ...]
    provider_schema_files: tuple[str, ...]

    @property
    def allowed_schema_files(self) -> tuple[str, ...]:
        return tuple(sorted({*self.shared_schema_files, *self.provider_schema_files}))


_CORE_QUERY_SCHEMAS = (
    "semantic-search-packet.v1.schema.json",
    "semantic-query-packet.v1.schema.json",
    "semantic-org-elements-query-packet.v1.schema.json",
    "semantic-content-compaction.v1.schema.json",
    "semantic-read-packet.v1.schema.json",
    "semantic-source-location.v1.schema.json",
    "semantic-tree-sitter-provenance.v1.schema.json",
    "semantic-tree-sitter-query.v1.schema.json",
    "semantic-tree-sitter-grammar-profile.v1.schema.json",
    "semantic-relation-plan.v1.schema.json",
    "semantic-flow-lite.v1.schema.json",
    "semantic-codeql-evidence.v1.schema.json",
    "semantic-fact-graph.v1.schema.json",
    "semantic-fact-ontology.v1.schema.json",
    "semantic-dependency-topology.v1.schema.json",
    "semantic-structural-index.v1.schema.json",
    "semantic-graph.v1.schema.json",
    "semantic-type-surface.v1.schema.json",
    "semantic-handle.v1.schema.json",
    "semantic-language-registry.v1.schema.json",
)

_AGENT_REASONING_SCHEMAS = (
    "software-criterion-catalog.v1.schema.json",
    "semantic-verification-receipt.v1.schema.json",
    "semantic-behavior-snapshot.v1.schema.json",
    "semantic-determinism-readiness.v1.schema.json",
    "semantic-dev-command-log.v1.schema.json",
    "semantic-formal-proof-pilot.v1.schema.json",
    "semantic-review-packet.v1.schema.json",
    "semantic-evidence-graph.v1.schema.json",
    "semantic-assurance-case.v1.schema.json",
    "semantic-ast-patch.v1.schema.json",
    "semantic-ast-patch-receipt.v1.schema.json",
)


LANGUAGE_SCHEMA_PROFILES: tuple[LanguageSchemaProfile, ...] = (
    LanguageSchemaProfile(
        language_id="rust",
        package_root="languages/rust-lang-project-harness",
        shared_schema_files=(
            *_CORE_QUERY_SCHEMAS,
            "semantic-invariant-candidate.v1.schema.json",
            "semantic-native-syntax-fact-index.v1.schema.json",
            *_AGENT_REASONING_SCHEMAS,
            "rust-ast-patch-real-project-evidence.v1.schema.json",
        ),
        provider_schema_files=("rust-semantic-capabilities.v1.schema.json",),
    ),
    LanguageSchemaProfile(
        language_id="typescript",
        package_root="languages/typescript-lang-project-harness",
        shared_schema_files=(
            *_CORE_QUERY_SCHEMAS,
            *_AGENT_REASONING_SCHEMAS,
        ),
        provider_schema_files=("typescript-semantic-capabilities.v1.schema.json",),
    ),
    LanguageSchemaProfile(
        language_id="python",
        package_root="languages/python-lang-project-harness",
        shared_schema_files=(
            *_CORE_QUERY_SCHEMAS,
            *_AGENT_REASONING_SCHEMAS,
        ),
        provider_schema_files=("python-semantic-capabilities.v1.schema.json",),
    ),
    LanguageSchemaProfile(
        language_id="julia",
        package_root="languages/JuliaLangProjectHarness.jl",
        shared_schema_files=(
            *_CORE_QUERY_SCHEMAS,
            "semantic-native-syntax-fact-index.v1.schema.json",
            *_AGENT_REASONING_SCHEMAS,
        ),
        provider_schema_files=(),
    ),
)


def select_schema_profiles(
    profiles: Iterable[LanguageSchemaProfile],
    language_ids: Iterable[str] = (),
) -> tuple[LanguageSchemaProfile, ...]:
    requested = tuple(language_ids)
    all_profiles = tuple(profiles)
    if not requested:
        return all_profiles
    requested_set = set(requested)
    selected_profiles = tuple(
        profile for profile in all_profiles if profile.language_id in requested_set
    )
    missing = sorted(requested_set - {profile.language_id for profile in selected_profiles})
    if missing:
        raise ValueError(f"unknown language schema profile: {', '.join(missing)}")
    return selected_profiles
