use std::collections::BTreeMap;

use crate::protocol_activation::protocol_activation_manifest::{ActivatedProvider, HookRuntime};
use crate::tool_action::ToolAction;

pub(super) struct RegisteredAspMatch<'a> {
    pub(super) language_id: agent_semantic_config::LanguageId,
    pub(super) provider: Option<&'a ActivatedProvider>,
}

/// Match one parsed command stage against activated ASP language capabilities.
pub(super) fn match_registered_asp_command<'a>(
    patterns: &[Vec<String>],
    runtime: &'a HookRuntime,
    action: &ToolAction,
) -> Option<RegisteredAspMatch<'a>> {
    if patterns.is_empty() {
        return None;
    }
    let stages =
        crate::command_match::bash::parse_bash_command_candidates(action.command.as_deref()?)
            .ok()?;
    for stage in stages {
        for pattern in patterns {
            if stage.words().len() < pattern.len() {
                continue;
            }
            for candidate in stage.words().windows(pattern.len()) {
                let mut registered_language = None;
                let matches =
                    candidate
                        .iter()
                        .zip(pattern)
                        .enumerate()
                        .all(|(index, (actual, expected))| {
                            if expected == "<registered-language>" {
                                if let Some(language_id) =
                                    crate::provider_registry::registered_language_id(actual)
                                {
                                    registered_language = Some(language_id);
                                    return true;
                                }
                                return false;
                            }
                            actual.eq_ignore_ascii_case(expected)
                                || (index == 0
                                    && actual
                                        .rsplit(['/', '\\'])
                                        .next()
                                        .is_some_and(|name| name.eq_ignore_ascii_case(expected)))
                        });
                if !matches {
                    continue;
                }
                let language_id = registered_language?;
                let provider = runtime
                    .providers
                    .iter()
                    .find(|provider| provider.language_id == language_id);
                return Some(RegisteredAspMatch {
                    language_id,
                    provider,
                });
            }
        }
    }
    None
}

pub(super) fn append_materialization_fields(
    fields: &mut BTreeMap<String, serde_json::Value>,
    matched: &RegisteredAspMatch<'_>,
    lazy_provider: Option<agent_semantic_config::HookClientLazyProviderPolicy>,
) {
    fields.insert(
        "registeredLanguageId".to_string(),
        serde_json::Value::String(matched.language_id.as_str().to_owned()),
    );
    let Some(provider) = matched.provider else {
        fields.insert(
            "providerMaterialization".to_string(),
            serde_json::Value::String("activation-required".to_string()),
        );
        fields.insert(
            "providerActivationRefresh".to_string(),
            serde_json::Value::String("hook-auto".to_string()),
        );
        if matches!(
            lazy_provider,
            Some(agent_semantic_config::HookClientLazyProviderPolicy::MatchedLanguage)
        ) {
            fields.insert(
                "providerLazyLoadCommand".to_string(),
                serde_json::Value::String(format!("asp install language {}", matched.language_id)),
            );
        }
        return;
    };
    fields.insert(
        "providerId".to_string(),
        serde_json::Value::String(provider.provider_id.as_str().to_owned()),
    );
    fields.insert(
        "providerBinary".to_string(),
        serde_json::Value::String(provider.binary.clone()),
    );
    fields.insert(
        "providerMaterialization".to_string(),
        serde_json::Value::String("activated".to_string()),
    );
}
