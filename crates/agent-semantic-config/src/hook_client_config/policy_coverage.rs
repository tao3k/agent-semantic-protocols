// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Configuration-authoritative combinatorial Hook policy coverage planning.

use std::collections::BTreeSet;

use super::HookClientActionKind;
use super::HookClientActionSubjectKind;
use super::HookClientConfigDecision;
use super::HookClientConfigFile;
use super::HookClientRuleConfig;

macro_rules! coverage_text_type {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
        pub struct $name(String);

        impl $name {
            /// Returns the exact configuration-derived text.
            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// Consumes the identity and returns its owned wire text.
            pub fn into_string(self) -> String {
                self.0
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(&self.0)
            }
        }

        impl std::ops::Deref for $name {
            type Target = str;

            fn deref(&self) -> &Self::Target {
                self.as_str()
            }
        }
    };
}

coverage_text_type!(
    HookPolicyCoverageLanguageId,
    "Language identity carried by a generated Hook policy witness."
);
coverage_text_type!(
    HookPolicyCoverageProviderId,
    "Provider identity carried by a generated Hook policy witness."
);
coverage_text_type!(
    HookPolicyCoveragePath,
    "Source path carried by a generated Hook policy witness."
);
coverage_text_type!(
    HookPolicyCoverageRuleId,
    "Winning declarative rule identity for a generated Hook policy witness."
);

/// Expected decision side for a generated policy coverage case.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HookPolicyCoveragePolarity {
    /// A registered source access expected to match a deny route.
    Black,
    /// A mechanically mutated non-source access expected to remain allowed.
    White,
}

/// Host surface exercised by a generated policy coverage case.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HookPolicyCoverageSurface {
    /// A shell program delivered through the Host `Bash` matcher.
    Shell,
}

/// Bounds controlling deterministic Hook policy coverage generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HookPolicyCoverageSettings {
    /// Greatest wrapper nesting depth generated for a case.
    pub max_wrapper_depth: usize,
    /// Whether every positive source case receives a negative extension pair.
    pub include_negative_extension_mutation: bool,
    /// Count of available Host shell envelope shapes.
    pub shell_envelope_count: usize,
}

/// One configuration-derived Hook policy coverage witness.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HookPolicyCoverageCase {
    /// Expected decision side for the witness.
    pub polarity: HookPolicyCoveragePolarity,
    /// Host surface carrying the witness.
    pub surface: HookPolicyCoverageSurface,
    /// Language profile that owns the source extension.
    pub language_id: HookPolicyCoverageLanguageId,
    /// Provider route declared by the language profile.
    pub provider_id: HookPolicyCoverageProviderId,
    /// Registered source extension, including its leading dot.
    pub source_extension: String,
    /// Generated witness path.
    pub path: HookPolicyCoveragePath,
    /// Rule expected to win, or `None` for an allowed negative case.
    pub expected_rule_id: Option<HookPolicyCoverageRuleId>,
    /// Typed decision reason expected from the Hook engine.
    pub expected_reason_kind: &'static str,
    /// Optional opaque command prefix used by the shell probe.
    pub command_prefix: Option<Vec<String>>,
    /// Index of the Host envelope shape used for this witness.
    pub envelope_slot: usize,
    /// Number of wrapper layers around the effective command.
    pub wrapper_depth: usize,
}

/// Derive the abstract covering plan from configuration only. Host-specific
/// JSON envelopes are deliberately absent from this authority layer.
pub fn derive_hook_policy_coverage_cases(
    config: &HookClientConfigFile,
    settings: HookPolicyCoverageSettings,
) -> Result<Vec<HookPolicyCoverageCase>, String> {
    if settings.shell_envelope_count == 0 {
        return Err("Hook policy coverage requires non-empty Host envelope projections".to_owned());
    }
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
                        language_id: profile.language_id.clone().into(),
                        provider_id: profile.provider_id.clone().into(),
                        source_extension: extension.clone(),
                        path: path.clone().into(),
                        expected_rule_id: Some(winning_rule.id.clone().into()),
                        expected_reason_kind: "registered-source-route-required",
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
    rule.enabled
        && matches!(rule.decision, HookClientConfigDecision::Deny)
        && rule.actions.contains(&HookClientActionKind::Read)
        && !rule.profiles_list.is_empty()
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
    white.path =
        mutate_path_outside_registered_extensions(&white.path, registered_extensions).into();
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
