//! Configuration-authoritative combinatorial Hook policy coverage planning.

use std::collections::BTreeSet;

use super::{
    HookClientActionAuthority, HookClientActionKind, HookClientActionSubjectKind,
    HookClientConfigDecision, HookClientConfigFile, HookClientRuleConfig,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HookPolicyCoveragePolarity {
    Black,
    White,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HookPolicyCoverageSurface {
    Direct,
    Shell,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HookPolicyCoverageSettings {
    pub max_wrapper_depth: usize,
    pub include_negative_extension_mutation: bool,
    pub direct_envelope_count: usize,
    pub shell_envelope_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HookPolicyCoverageCase {
    pub polarity: HookPolicyCoveragePolarity,
    pub surface: HookPolicyCoverageSurface,
    pub language_id: String,
    pub provider_id: String,
    pub source_extension: String,
    pub path: String,
    pub expected_rule_id: Option<String>,
    pub expected_reason_kind: &'static str,
    pub command_prefix: Option<Vec<String>>,
    pub envelope_slot: usize,
    pub wrapper_depth: usize,
}

/// Derive the abstract covering plan from configuration only. Host-specific
/// JSON envelopes are deliberately absent from this authority layer.
pub fn derive_hook_policy_coverage_cases(
    config: &HookClientConfigFile,
    settings: HookPolicyCoverageSettings,
) -> Result<Vec<HookPolicyCoverageCase>, String> {
    if settings.direct_envelope_count == 0 || settings.shell_envelope_count == 0 {
        return Err("Hook policy coverage requires non-empty Host envelope projections".to_owned());
    }
    let direct_rule = composed_rule(config, "raw-host-read", "registered-language-source")?;
    let shell_rules = config
        .rules
        .iter()
        .filter(|rule| is_shell_registered_read_rule(rule))
        .collect::<Vec<_>>();
    let read_prefixes = shell_rules
        .iter()
        .flat_map(|rule| rule.match_config.effect_rules.iter())
        .filter(|effect| effect.effect == HookClientActionKind::Read)
        .filter(|effect| !effect.argv_prefix.is_empty())
        .map(|effect| effect.argv_prefix.clone())
        .collect::<BTreeSet<_>>();
    if read_prefixes.is_empty() {
        return Err("Hook config has no applicable shell registered-read prefix".to_owned());
    }
    let registered_extensions = config
        .language_providers
        .iter()
        .flat_map(|provider| provider.source_extensions.iter().cloned())
        .collect::<BTreeSet<_>>();
    let mut cases = Vec::new();
    let mut extension_index = 0usize;
    for provider in &config.language_providers {
        for extension in &provider.source_extensions {
            let path = format!(
                "generated/{}/witness{}",
                provider.language_id.replace(['/', '\\'], "-"),
                extension
            );
            for envelope_slot in 0..settings.direct_envelope_count {
                push_case_pair(
                    &mut cases,
                    settings,
                    HookPolicyCoverageCase {
                        polarity: HookPolicyCoveragePolarity::Black,
                        surface: HookPolicyCoverageSurface::Direct,
                        language_id: provider.language_id.clone(),
                        provider_id: provider.provider_id.clone(),
                        source_extension: extension.clone(),
                        path: path.clone(),
                        expected_rule_id: Some(direct_rule.id.clone()),
                        expected_reason_kind: "direct-source-read",
                        command_prefix: None,
                        envelope_slot,
                        wrapper_depth: 0,
                    },
                    &registered_extensions,
                );
            }
            for (command_index, prefix) in read_prefixes.iter().enumerate() {
                let winning_rule = shell_rules
                    .iter()
                    .filter(|rule| {
                        rule.match_config.effect_rules.iter().any(|effect| {
                            effect.effect == HookClientActionKind::Read
                                && effect.argv_prefix == *prefix
                        })
                    })
                    .max_by_key(|rule| rule.priority)
                    .ok_or_else(|| {
                        format!(
                            "no config rule owns generated shell prefix `{}`",
                            prefix.join(" ")
                        )
                    })?;
                let wrapper_depth = (extension_index + command_index)
                    % settings.max_wrapper_depth.saturating_add(1);
                push_case_pair(
                    &mut cases,
                    settings,
                    HookPolicyCoverageCase {
                        polarity: HookPolicyCoveragePolarity::Black,
                        surface: HookPolicyCoverageSurface::Shell,
                        language_id: provider.language_id.clone(),
                        provider_id: provider.provider_id.clone(),
                        source_extension: extension.clone(),
                        path: path.clone(),
                        expected_rule_id: Some(winning_rule.id.clone()),
                        expected_reason_kind: "bulk-source-dump",
                        command_prefix: Some(prefix.clone()),
                        envelope_slot: (extension_index + command_index)
                            % settings.shell_envelope_count,
                        wrapper_depth,
                    },
                    &registered_extensions,
                );
            }
            extension_index += 1;
        }
    }
    if cases.is_empty() {
        return Err("Hook configuration generated no policy coverage cases".to_owned());
    }
    Ok(cases)
}

fn is_shell_registered_read_rule(rule: &HookClientRuleConfig) -> bool {
    let match_config = &rule.match_config;
    let composed_shell_source = match_config
        .action_policy_all
        .iter()
        .any(|id| id == "raw-shell-read")
        && match_config
            .action_policy_all
            .iter()
            .any(|id| id == "registered-language-source");
    rule.enabled
        && matches!(rule.decision, HookClientConfigDecision::Deny)
        && (composed_shell_source || match_config.argv_registered_source_file)
        && (match_config.action_any.is_empty()
            || match_config
                .action_any
                .contains(&HookClientActionKind::Execute))
        && (match_config.effect_any.is_empty()
            || match_config
                .effect_any
                .contains(&HookClientActionKind::Read))
        && (match_config.subject_kind_any.is_empty()
            || match_config
                .subject_kind_any
                .contains(&HookClientActionSubjectKind::RegisteredLanguageSource))
        && (match_config.authority_any.is_empty()
            || match_config
                .authority_any
                .contains(&HookClientActionAuthority::RawShell))
}

fn push_case_pair(
    cases: &mut Vec<HookPolicyCoverageCase>,
    settings: HookPolicyCoverageSettings,
    black: HookPolicyCoverageCase,
    registered_extensions: &BTreeSet<String>,
) {
    let mut white = black.clone();
    white.polarity = HookPolicyCoveragePolarity::White;
    white.path = mutate_path_outside_registered_extensions(&white.path, registered_extensions);
    white.expected_rule_id = None;
    white.expected_reason_kind = "none";
    cases.push(black);
    if settings.include_negative_extension_mutation {
        cases.push(white);
    }
}

/// Mechanically mutate a source path into an extension class not owned by any
/// configured provider. This is shared by coverage planning and compiled
/// decision shards so tests cannot reserve their own magic negative suffix.
pub fn mutate_path_outside_registered_extensions(
    path: &str,
    registered_extensions: &BTreeSet<String>,
) -> String {
    let mut candidate = path.to_owned();
    loop {
        candidate.push('_');
        if registered_extensions
            .iter()
            .all(|extension| !candidate.ends_with(extension))
        {
            return candidate;
        }
    }
}

fn composed_rule<'a>(
    config: &'a HookClientConfigFile,
    action_policy: &str,
    source_policy: &str,
) -> Result<&'a HookClientRuleConfig, String> {
    config
        .rules
        .iter()
        .find(|rule| {
            rule.enabled
                && rule
                    .match_config
                    .action_policy_all
                    .iter()
                    .any(|id| id == action_policy)
                && rule
                    .match_config
                    .action_policy_all
                    .iter()
                    .any(|id| id == source_policy)
        })
        .ok_or_else(|| {
            format!(
                "Hook config has no enabled rule composing `{action_policy}` with `{source_policy}`"
            )
        })
}
