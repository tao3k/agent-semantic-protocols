//! Configuration-authoritative combinatorial Hook policy coverage planning.

use std::collections::BTreeSet;

use super::{
    HookClientActionKind, HookClientActionSubjectKind, HookClientConfigDecision,
    HookClientConfigFile, HookClientRuleConfig,
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
    let direct_rule = config
        .rules
        .iter()
        .find(|rule| {
            rule.enabled
                && rule
                    .actions
                    .contains(&crate::hook_client_config::routing::HookClientActionKind::Read)
                && !rule.profiles_list.is_empty()
        })
        .ok_or_else(|| {
            "Hook config has no enabled declarative Read rule with profilesList".to_owned()
        })?;
    let shell_rules = config
        .rules
        .iter()
        .filter(|rule| is_shell_registered_read_rule(rule))
        .collect::<Vec<_>>();
    let opaque_shell_stage = vec!["opaque-source-consumer".to_owned()];
    let registered_extensions = config
        .profiles
        .values()
        .flat_map(|profile| {
            profile
                .extension_any
                .iter()
                .map(|extension| format!(".{extension}"))
        })
        .collect::<BTreeSet<_>>();
    let mut cases = Vec::new();
    let mut extension_index = 0usize;
    for profile in config.profiles.values() {
        for bare_extension in &profile.extension_any {
            let extension = format!(".{bare_extension}");
            let path = format!(
                "generated/{}/witness{}",
                profile.language_id.replace(['/', '\\'], "-"),
                extension
            );
            for envelope_slot in 0..settings.direct_envelope_count {
                push_case_pair(
                    &mut cases,
                    settings,
                    HookPolicyCoverageCase {
                        polarity: HookPolicyCoveragePolarity::Black,
                        surface: HookPolicyCoverageSurface::Direct,
                        language_id: profile.language_id.clone(),
                        provider_id: profile.provider_id.clone(),
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
            for (command_index, prefix) in std::iter::once(&opaque_shell_stage).enumerate() {
                let winning_rule = shell_rules
                    .iter()
                    .max_by_key(|rule| rule.priority)
                    .ok_or_else(|| "no config rule owns opaque shell source access".to_owned())?;
                let wrapper_depth = (extension_index + command_index)
                    % settings.max_wrapper_depth.saturating_add(1);
                push_case_pair(
                    &mut cases,
                    settings,
                    HookPolicyCoverageCase {
                        polarity: HookPolicyCoveragePolarity::Black,
                        surface: HookPolicyCoverageSurface::Shell,
                        language_id: profile.language_id.clone(),
                        provider_id: profile.provider_id.clone(),
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
        .capability_policy_all
        .iter()
        .any(|id| id == "opaque-shell-source-access")
        && match_config
            .capability_policy_all
            .iter()
            .any(|id| id == "registered-language-source");
    rule.enabled
        && matches!(rule.decision, HookClientConfigDecision::Deny)
        && (composed_shell_source || match_config.argv_registered_source_file)
        && (match_config.action_any.is_empty()
            || match_config
                .action_any
                .contains(&HookClientActionKind::Execute))
        && (match_config.subject_kind_any.is_empty()
            || match_config
                .subject_kind_any
                .contains(&HookClientActionSubjectKind::RegisteredLanguageSource))
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
