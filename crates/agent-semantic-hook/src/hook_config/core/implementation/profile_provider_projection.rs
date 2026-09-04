use std::collections::BTreeMap;

use agent_semantic_config::HookClientProfileConfig;
use agent_semantic_config::HookClientProviderRouteIdentity;
use agent_semantic_config::LanguageId;
use agent_semantic_config::ProviderId;

use crate::protocol::ActionPolicy;
use crate::protocol::CommandTemplate;
use crate::protocol::HookPolicy;
use crate::protocol_activation::protocol_activation_manifest::HookProviderProjection;

pub(super) fn extend_profile_provider_projections(
    profiles: &BTreeMap<String, HookClientProfileConfig>,
    provider_projections: &mut Vec<HookProviderProjection>,
) {
    for profile in profiles.values() {
        if let Some(provider) = provider_projections.iter_mut().find(|provider| {
            provider.language_id.as_str() == profile.language_id
                && provider.provider_id.as_str() == profile.provider_id
        }) {
            extend_unique(
                &mut provider.source_extensions,
                profile
                    .extension_any
                    .iter()
                    .map(|extension| format!(".{}", extension.trim().trim_start_matches('.'))),
            );
            extend_unique(
                &mut provider.package_roots,
                profile.source_root_any.iter().cloned(),
            );
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
            profile.source_root_any.clone(),
        ));
    }
}

fn extend_unique(target: &mut Vec<String>, values: impl IntoIterator<Item = String>) {
    for value in values {
        if !target.iter().any(|existing| existing == &value) {
            target.push(value);
        }
    }
}

pub(super) fn merge_provider_projection_facts(
    provider_projections: &mut Vec<HookProviderProjection>,
    supplemental_projections: &[HookProviderProjection],
) {
    for supplemental in supplemental_projections {
        if let Some(provider) = provider_projections.iter_mut().find(|provider| {
            provider.language_id == supplemental.language_id
                && provider.provider_id == supplemental.provider_id
        }) {
            extend_unique(
                &mut provider.source_extensions,
                supplemental.source_extensions.iter().cloned(),
            );
            extend_unique(
                &mut provider.package_roots,
                supplemental.package_roots.iter().cloned(),
            );
            extend_unique(
                &mut provider.config_files,
                supplemental.config_files.iter().cloned(),
            );
            continue;
        }
        provider_projections.push(supplemental.clone());
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
            Vec::new(),
        ));
    }
}

fn provider_projection(
    language: &str,
    provider_id: &str,
    source_extensions: Vec<String>,
    package_roots: Vec<String>,
) -> HookProviderProjection {
    let command = |argv: Vec<String>, stdin_mode| CommandTemplate { argv, stdin_mode };
    HookProviderProjection {
        language_id: LanguageId::new(language),
        provider_id: ProviderId::new(provider_id),
        package_roots,
        source_extensions,
        config_files: Vec::new(),
        policy: HookPolicy {
            direct_source_read: ActionPolicy::Block,
            bulk_source_dump: ActionPolicy::Block,
            raw_source_search: ActionPolicy::Block,
            agent_search_json: ActionPolicy::Block,
        },
        playbook_route: command(
            [
                "asp",
                language,
                "search",
                "playbook",
                "{query}",
                "--scope",
                "owner:{owner}",
                "--workspace",
                "{workspace}",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            None,
        ),
    }
}
