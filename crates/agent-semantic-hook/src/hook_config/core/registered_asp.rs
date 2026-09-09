// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::collections::BTreeMap;

use crate::provider_projection::HookRuntime;
use crate::tool_action::ToolAction;

pub(super) struct RegisteredAspMatch {
    pub(super) language_id: agent_semantic_config::LanguageId,
    pub(super) provider_id: agent_semantic_config::ProviderId,
}

/// Match one parsed command stage against registered ASP language identities.
pub(super) fn match_registered_asp_command(
    patterns: &[Vec<String>],
    runtime: &HookRuntime,
    action: &ToolAction,
) -> Option<RegisteredAspMatch> {
    if patterns.is_empty() {
        return None;
    }
    let stages =
        crate::shell_parser::bash::parse_bash_command_candidates(action.command.as_deref()?)
            .ok()?;
    for provider in &runtime.policy_providers {
        let language_id = &provider.language_id;
        for pattern in patterns {
            if registered_root_search_playbook_pattern(pattern) {
                if registered_root_search_playbook_matches(&stages, language_id.as_str()) {
                    return Some(RegisteredAspMatch {
                        provider_id: provider.provider_id.clone(),
                        language_id: language_id.clone(),
                    });
                }
                continue;
            }
            if registered_root_query_playbook_pattern(pattern) {
                if registered_root_query_playbook_matches(&stages, language_id.as_str()) {
                    return Some(RegisteredAspMatch {
                        provider_id: provider.provider_id.clone(),
                        language_id: language_id.clone(),
                    });
                }
                continue;
            }
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
                    provider_id: provider.provider_id.clone(),
                    language_id: language_id.clone(),
                });
            }
        }
    }
    None
}

fn registered_root_search_playbook_pattern(pattern: &[String]) -> bool {
    pattern == ["asp".to_owned(), "search".to_owned(), "playbook".to_owned()]
}

fn registered_root_query_playbook_pattern(pattern: &[String]) -> bool {
    pattern == ["asp".to_owned(), "query".to_owned(), "playbook".to_owned()]
}

/// Match the canonical root playbook spelling against the same declarative
/// registered-provider search pattern. Producer-set union is data carried by
/// `--language` or `--documents`; producer-first command namespaces are not aliases.
fn registered_root_search_playbook_matches(
    stages: &[agent_semantic_shell_parser::CommandStage],
    language_id: &str,
) -> bool {
    stages.iter().any(|stage| {
        let words = stage.words();
        words.windows(3).any(|prefix| {
            prefix == ["asp", "search", "playbook"]
                && words.windows(2).any(|argument| {
                    matches!(argument[0].as_str(), "--language" | "--documents")
                        && argument[1]
                            .split('|')
                            .any(|producer| producer == language_id)
                })
        })
    })
}

/// Query uses the same root Playbook producer boundaries as Search. The
/// canonical selector scheme must agree with that producer set.
fn registered_root_query_playbook_matches(
    stages: &[agent_semantic_shell_parser::CommandStage],
    language_id: &str,
) -> bool {
    let selector_prefix = format!("{language_id}://");
    stages.iter().any(|stage| {
        let words = stage.words();
        words
            .windows(3)
            .any(|prefix| prefix == ["asp", "query", "playbook"])
            && words.windows(2).any(|argument| {
                matches!(argument[0].as_str(), "--language" | "--documents")
                    && argument[1]
                        .split('|')
                        .any(|producer| producer == language_id)
            })
            && words.windows(2).any(|argument| {
                argument[0] == "--selector" && argument[1].starts_with(&selector_prefix)
            })
    })
}

pub(super) fn append_registered_provider_fields(
    fields: &mut BTreeMap<String, serde_json::Value>,
    matched: &RegisteredAspMatch,
) {
    fields.insert(
        "registeredLanguageId".to_string(),
        serde_json::Value::String(matched.language_id.as_str().to_owned()),
    );
    fields.insert(
        "providerId".to_string(),
        serde_json::Value::String(matched.provider_id.as_str().to_owned()),
    );
}

#[cfg(test)]
#[path = "../../../tests/unit/registered_asp.rs"]
mod tests;
