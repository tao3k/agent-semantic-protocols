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
        crate::shell_parser::bash::parse_bash_command_candidates(action.command.as_deref()?)
            .ok()?;
    let mut language_ids = Vec::new();
    for projection in &runtime.policy_providers {
        if !language_ids.contains(&projection.language_id) {
            language_ids.push(projection.language_id.clone());
        }
    }
    for language_id in language_ids {
        for pattern in patterns {
            let concrete_prefix = pattern
                .iter()
                .map(|token| {
                    if token == "<registered-language>" {
                        language_id.as_str().to_owned()
                    } else {
                        token.clone()
                    }
                })
                .collect::<Vec<_>>();
            if agent_semantic_shell_parser::command_stages_match_wrapped_prefix(
                &stages,
                &concrete_prefix,
            )
            .routes_protected()
            {
                return Some(RegisteredAspMatch {
                    provider: runtime
                        .providers
                        .iter()
                        .find(|provider| provider.language_id == language_id),
                    language_id,
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
        "providerMaterialization".to_string(),
        serde_json::Value::String("static-route".to_string()),
    );
}
