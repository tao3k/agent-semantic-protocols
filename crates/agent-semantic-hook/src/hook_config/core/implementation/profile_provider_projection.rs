use std::collections::BTreeMap;

use agent_semantic_config::{
    HookClientProfileConfig, HookClientProviderRouteIdentity, LanguageId, ProviderId,
};

use crate::protocol::{ActionPolicy, CommandTemplate, HookPolicy, StdinMode};
use crate::protocol_activation::protocol_activation_manifest::HookProviderProjection;

pub(super) fn extend_profile_provider_projections(
    profiles: &BTreeMap<String, HookClientProfileConfig>,
    provider_projections: &mut Vec<HookProviderProjection>,
) {
    for profile in profiles.values() {
        if provider_projections.iter().any(|provider| {
            provider.language_id.as_str() == profile.language_id
                && provider.provider_id.as_str() == profile.provider_id
        }) {
            continue;
        }
        provider_projections.push(provider_projection(
            profile.language_id.as_str(),
            profile.provider_id.as_str(),
            profile
                .extension_any
                .iter()
                .map(|extension| format!(".{}", extension.trim().trim_start_matches('.')))
                .collect(),
        ));
    }
}

pub(super) fn extend_registered_provider_route_projections(
    routes: &[HookClientProviderRouteIdentity],
    provider_projections: &mut Vec<HookProviderProjection>,
) {
    for route in routes {
        if provider_projections.iter().any(|provider| {
            provider.language_id.as_str() == route.language_id
                && provider.provider_id.as_str() == route.provider_id
        }) {
            continue;
        }
        provider_projections.push(provider_projection(
            route.language_id.as_str(),
            route.provider_id.as_str(),
            Vec::new(),
        ));
    }
}

fn provider_projection(
    language: &str,
    provider_id: &str,
    source_extensions: Vec<String>,
) -> HookProviderProjection {
    let command = |argv: Vec<String>, stdin_mode| CommandTemplate { argv, stdin_mode };
    HookProviderProjection {
        language_id: LanguageId::new(language),
        provider_id: ProviderId::new(provider_id),
        package_roots: Vec::new(),
        source_extensions,
        config_files: Vec::new(),
        policy: HookPolicy {
            direct_source_read: ActionPolicy::Block,
            bulk_source_dump: ActionPolicy::Block,
            raw_source_search: ActionPolicy::Block,
            agent_search_json: ActionPolicy::Block,
        },
        owner_route: command(
            [
                "asp",
                language,
                "search",
                "owner",
                "{owner}",
                "items",
                "--workspace",
                "{workspace}",
                "--view",
                "seeds",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            None,
        ),
        lexical_route: command(
            [
                "asp",
                language,
                "search",
                "lexical",
                "{query}",
                "owner",
                "tests",
                "--workspace",
                "{workspace}",
                "--view",
                "seeds",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            None,
        ),
        ingest_route: command(
            [
                "asp",
                language,
                "search",
                "ingest",
                "owner",
                "tests",
                "--workspace",
                "{workspace}",
                "--view",
                "seeds",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            Some(StdinMode::PipeCandidates),
        ),
    }
}
