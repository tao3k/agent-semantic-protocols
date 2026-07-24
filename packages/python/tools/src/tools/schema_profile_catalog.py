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


_PRIVATE_SCHEMA_OWNERS = {
    "semantic-org-elements-query-packet.v1.schema.json": "org",
    "semantic-gerbil-scheme-harness-info.v1.schema.json": "gerbil-scheme",
    "python-semantic-capabilities.v1.schema.json": "python",
    "rust-ast-patch-real-project-evidence.v1.schema.json": "rust",
    "rust-semantic-capabilities.v1.schema.json": "rust",
    "typescript-semantic-capabilities.v1.schema.json": "typescript",
}


def schema_private_owner(schema_name: str) -> str | None:
    """Return the provider language that owns a private schema name, if known."""

    return _PRIVATE_SCHEMA_OWNERS.get(schema_name)


def schema_profile_contract_errors(
    profiles: Iterable[LanguageSchemaProfile],
) -> tuple[str, ...]:
    """Validate that package profiles do not downsync another provider's schema."""

    errors: list[str] = []
    for profile in profiles:
        for schema_name in profile.shared_schema_files:
            owner = schema_private_owner(schema_name)
            if owner is not None:
                errors.append(
                    f"{profile.language_id}: shared-schema-owned-by-{owner} {schema_name}"
                )
        for schema_name in profile.provider_schema_files:
            owner = schema_private_owner(schema_name)
            if owner is not None and owner != profile.language_id:
                errors.append(
                    f"{profile.language_id}: provider-schema-owned-by-{owner} {schema_name}"
                )
    return tuple(errors)


_CORE_QUERY_SCHEMAS = (
    "semantic-search-packet.v1.schema.json",
    "semantic-query-packet.v1.schema.json",
    "semantic-exact-selector-receipt.v1.schema.json",
    "semantic-owner-item-evidence.v1.schema.json",
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
    "semantic-native-syntax-fact-index.v1.schema.json",
    "semantic-graph.v1.schema.json",
    "semantic-type-surface.v1.schema.json",
    "semantic-handle.v1.schema.json",
    "semantic-language-registry.v1.schema.json",
    "semantic-language-projection.v1.schema.json",
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
    "semantic-graph-turbo-request.v1.schema.json",
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
            *_AGENT_REASONING_SCHEMAS,
            "semantic-compare-packet.v1.schema.json",
        ),
        provider_schema_files=(
            "rust-ast-patch-real-project-evidence.v1.schema.json",
            "rust-semantic-capabilities.v1.schema.json",
        ),
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
            *_AGENT_REASONING_SCHEMAS,
        ),
        provider_schema_files=(),
    ),
    LanguageSchemaProfile(
        language_id="gerbil-scheme",
        package_root="languages/gerbil-scheme-language-project-harness",
        shared_schema_files=(
            "semantic-agent-hook-provider-manifest.v1.schema.json",
            "semantic-compare-packet.v1.schema.json",
            "semantic-content-compaction.v1.schema.json",
            "semantic-evidence-graph.v1.schema.json",
            "semantic-extension-pattern-mapping.v1.schema.json",
            "semantic-graph-turbo-request.v1.schema.json",
            "semantic-handle.v1.schema.json",
            "semantic-invariant-candidate.v1.schema.json",
            "semantic-language-evidence.v1.schema.json",
            "semantic-language-registry.v1.schema.json",
            "semantic-language-projection.v1.schema.json",
        "semantic-native-syntax-fact-index.v1.schema.json",
        "semantic-query-packet.v1.schema.json",
        "semantic-exact-selector-receipt.v1.schema.json",
        "semantic-read-packet.v1.schema.json",
            "semantic-runtime-source-acquisition.v1.schema.json",
            "semantic-search-packet.v1.schema.json",
            "semantic-source-location.v1.schema.json",
            "semantic-structural-index.v1.schema.json",
            "semantic-tree-sitter-provenance.v1.schema.json",
            "semantic-type-proof.v1.schema.json",
            "semantic-type-surface.v1.schema.json",
        ),
        provider_schema_files=(
            "semantic-gerbil-scheme-harness-info.v1.schema.json",
        ),
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
