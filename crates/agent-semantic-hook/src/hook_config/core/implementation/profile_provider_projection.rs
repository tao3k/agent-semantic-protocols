use std::collections::BTreeMap;

use agent_semantic_config::{HookClientProfileConfig, LanguageId, ProviderId};

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
        let language = profile.language_id.as_str();
        let command = |argv: Vec<String>, stdin_mode| CommandTemplate { argv, stdin_mode };
        provider_projections.push(HookProviderProjection {
            language_id: LanguageId::new(language),
            provider_id: ProviderId::new(profile.provider_id.as_str()),
            package_roots: Vec::new(),
            source_extensions: profile
                .extension_any
                .iter()
                .map(|extension| format!(".{}", extension.trim().trim_start_matches('.')))
                .collect(),
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
        });
    }
}
