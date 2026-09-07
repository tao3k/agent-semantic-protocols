// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Validation rules for hook client config files.

use std::collections::BTreeMap;
use std::collections::HashSet;

use super::HookClientCommandProfileConfig;
use super::HookClientCommandSetConfig;
use super::document::CLIENT_HOOK_CONFIG_SCHEMA_ID;
use super::document::CLIENT_HOOK_CONFIG_SCHEMA_VERSION;
use super::document::HOOK_PROTOCOL_ID;
use super::document::HOOK_PROTOCOL_VERSION;
use super::document::HookClientAgentCallingConfig;
use super::document::HookClientAgentOrgArtifactsArchiveWarningConfig;
use super::document::HookClientAgentOrgArtifactsConfig;
use super::document::HookClientConfigFile;
use super::expand_command_profile_prefixes;
use super::expand_command_set_prefixes;
use super::routing::HookClientRuleConfig;
use super::routing::HookClientRuleMatchConfig;
use super::routing::HookClientRuleRouteConfig;

pub(super) fn validate_config(config: &HookClientConfigFile) -> Result<(), String> {
    validate_protocol(config)?;
    validate_codex_host_matchers(config)?;
    validate_optional_non_empty(
        "contractFingerprint",
        config.contract_fingerprint.as_deref(),
    )?;
    validate_agent_org_artifacts(config.agent_org_artifacts.as_ref())?;
    validate_agent_calling(&config.agent_calling)?;
    validate_profiles(&config.profiles)?;
    validate_provider_routes(&config.provider_routes)?;
    validate_rule_profile_references(&config.rules, &config.profiles, &config.provider_routes)?;
    validate_command_profiles(&config.command_profiles)?;
    validate_command_sets(&config.command_sets)?;
    validate_command_action_patterns(&config.command_action_patterns)?;
    validate_capability_policies(&config.capability_policies)?;
    validate_rule_dispatches(&config.rules)?;
    validate_unique_rule_ids(&config.rules)?;
    validate_rule_schema_shape(
        &config.rules,
        &config.command_profiles,
        &config.command_sets,
        &config.capability_policies,
    )
}

fn validate_command_action_patterns(
    families: &[super::document::HookClientCommandActionPatternConfig],
) -> Result<(), String> {
    let mut unique = HashSet::new();
    for (family_index, family) in families.iter().enumerate() {
        if !matches!(
            family.action,
            super::routing::HookClientActionKind::Read
                | super::routing::HookClientActionKind::Search
        ) {
            return Err(format!(
                "commandActionPatterns[{family_index}].action must be read or search"
            ));
        }
        if family.argv_pattern_any.is_empty() {
            return Err(format!(
                "commandActionPatterns[{family_index}].argvPatternAny must not be empty"
            ));
        }
        for (pattern_index, pattern) in family.argv_pattern_any.iter().enumerate() {
            if pattern.is_empty() {
                return Err(format!(
                    "commandActionPatterns[{family_index}].argvPatternAny[{pattern_index}] must contain an executable basename"
                ));
            }
            validate_non_empty_values("commandActionPatterns[].argvPatternAny[]", pattern)?;
            let executable = &pattern[0];
            if executable.contains('/') || executable == "." || executable == ".." {
                return Err(format!(
                    "commandActionPatterns[{family_index}].argvPatternAny[{pattern_index}][0] must be an executable basename"
                ));
            }
            for token_glob in pattern.iter().skip(1) {
                globset::Glob::new(token_glob).map_err(|error| {
                format!(
                    "commandActionPatterns[{family_index}].argvPatternAny[{pattern_index}] contains invalid argv glob `{token_glob}`: {error}"
                )
            })?;
            }
            if !unique.insert((family.action, pattern)) {
                return Err(format!(
                    "commandActionPatterns contains duplicate action/pattern {:?}/{pattern:?}",
                    family.action
                ));
            }
        }
    }
    Ok(())
}

fn validate_codex_host_matchers(config: &HookClientConfigFile) -> Result<(), String> {
    for rule in &config.rules {
        let Some(matcher) = rule.matcher.as_deref() else {
            continue;
        };
        validate_codex_host_matcher_expression(matcher)
            .map_err(|error| format!("rule `{}` {error}", rule.id))?;
    }
    Ok(())
}

/// Validates the bounded Codex matcher-alias grammar accepted by Hook config.
pub fn validate_codex_host_matcher_expression(matcher: &str) -> Result<(), String> {
    if matcher.is_empty() || matcher == "*" {
        return Ok(());
    }
    if matcher.split('|').all(|alias| {
        !alias.is_empty()
            && !alias.chars().any(|character| {
                matches!(
                    character,
                    '^' | '$' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '\\'
                )
            })
    }) {
        return Ok(());
    }
    Err(format!(
        "uses unsupported Host matcher `{matcher}`; ASP config accepts only exact aliases separated by `|`"
    ))
}

fn validate_agent_calling(config: &HookClientAgentCallingConfig) -> Result<(), String> {
    let validate_pattern = |field: &str, pattern: &str| {
        validate_non_empty(field, pattern)?;
        let placeholder_count =
            pattern.matches("{name}").count() + pattern.matches("{name-kebab}").count();
        if placeholder_count != 1 {
            return Err(format!(
                "{field} must contain exactly one `{{name}}` or `{{name-kebab}}` placeholder"
            ));
        }
        Ok(())
    };
    validate_pattern("agentCalling.defaultPattern", &config.default_pattern)?;
    for (platform, pattern) in &config.platform_patterns {
        validate_identifier("agentCalling.platformPatterns platform", platform)?;
        validate_pattern(
            &format!("agentCalling.platformPatterns.{platform}"),
            pattern,
        )?;
    }
    Ok(())
}

fn validate_provider_routes(
    routes: &[super::document::HookClientProviderRouteIdentity],
) -> Result<(), String> {
    let mut identities = HashSet::new();
    let mut languages = HashSet::new();
    for route in routes {
        validate_non_empty("providerRoutes[].languageId", &route.language_id)?;
        validate_non_empty("providerRoutes[].providerId", &route.provider_id)?;
        if !identities.insert((route.language_id.as_str(), route.provider_id.as_str())) {
            return Err(format!(
                "duplicate provider route identity `{}/{}`",
                route.language_id, route.provider_id
            ));
        }
        if !languages.insert(route.language_id.as_str()) {
            return Err(format!(
                "provider facade language `{}` resolves to more than one provider identity",
                route.language_id
            ));
        }
    }
    Ok(())
}

fn validate_rule_profile_references(
    rules: &[HookClientRuleConfig],
    profiles: &BTreeMap<String, super::document::HookClientProfileConfig>,
    provider_routes: &[super::document::HookClientProviderRouteIdentity],
) -> Result<(), String> {
    for rule in rules {
        validate_rule_profiles(rule, profiles)?;
        validate_lazy_provider_profiles(rule, profiles, provider_routes)?;
        validate_rule_matcher_policies(rule)?;
    }
    Ok(())
}

fn validate_lazy_provider_profiles(
    rule: &HookClientRuleConfig,
    profiles: &BTreeMap<String, super::document::HookClientProfileConfig>,
    provider_routes: &[super::document::HookClientProviderRouteIdentity],
) -> Result<(), String> {
    if !rule.dispatch.as_ref().is_some_and(|dispatch| {
        matches!(
            dispatch.lazy_provider,
            Some(super::routing::HookClientLazyProviderPolicy::MatchedLanguage)
        )
    }) {
        return Ok(());
    }
    for profile_id in &rule.profiles_list {
        let profile = profiles
            .get(profile_id)
            .expect("profile existence is validated before provider reachability");
        if !provider_routes.iter().any(|route| {
            route.language_id == profile.language_id && route.provider_id == profile.provider_id
        }) {
            return Err(format!(
                "rule {} profilesList profile {profile_id:?} selects unregistered lazy provider {}/{}",
                rule.id, profile.language_id, profile.provider_id
            ));
        }
    }
    Ok(())
}

fn validate_rule_profiles<'a>(
    rule: &HookClientRuleConfig,
    profiles: &'a BTreeMap<String, super::document::HookClientProfileConfig>,
) -> Result<(), String> {
    let mut profile_ids = HashSet::new();
    let mut extension_targets = BTreeMap::<String, (&'a str, &'a str)>::new();
    for profile_id in &rule.profiles_list {
        if !profile_ids.insert(profile_id) {
            return Err(format!(
                "rule {} profilesList contains duplicate profile {profile_id:?}",
                rule.id
            ));
        }
        let profile = profiles.get(profile_id).ok_or_else(|| {
            format!(
                "rule {} profilesList references unknown profile {profile_id:?}",
                rule.id
            )
        })?;
        validate_profile_extension_targets(rule, profile, &mut extension_targets)?;
    }
    Ok(())
}

fn validate_profile_extension_targets<'a>(
    rule: &HookClientRuleConfig,
    profile: &'a super::document::HookClientProfileConfig,
    targets: &mut BTreeMap<String, (&'a str, &'a str)>,
) -> Result<(), String> {
    for extension in &profile.extension_any {
        let extension = extension.trim().to_ascii_lowercase();
        match targets.get(&extension) {
            Some((language_id, provider_id))
                if *language_id != profile.language_id || *provider_id != profile.provider_id =>
            {
                return Err(format!(
                    "rule {} profilesList maps extension {extension:?} to both {language_id}/{provider_id} and {}/{}",
                    rule.id, profile.language_id, profile.provider_id
                ));
            }
            Some(_) => {}
            None => {
                targets.insert(extension, (&profile.language_id, &profile.provider_id));
            }
        }
    }
    Ok(())
}

fn validate_rule_matcher_policies(rule: &HookClientRuleConfig) -> Result<(), String> {
    let mut matcher_policies = HashSet::new();
    for matcher_policy in &rule.matcher_policies {
        if !matcher_policies.insert(matcher_policy) {
            return Err(format!(
                "rule {} matcherPolicies contains duplicate policy {matcher_policy:?}",
                rule.id
            ));
        }
    }
    Ok(())
}

fn validate_capability_policies(
    policies: &[super::routing::HookClientCapabilityPolicyConfig],
) -> Result<(), String> {
    let mut ids = HashSet::new();
    for policy in policies {
        validate_identifier("capabilityPolicies[].id", &policy.id)?;
        if !ids.insert(policy.id.as_str()) {
            return Err(format!("duplicate capability policy id `{}`", policy.id));
        }
        if policy.host_invocation_any.is_empty()
            && policy.semantic_capability_any.is_empty()
            && policy.subject_kind_any.is_empty()
        {
            return Err(format!(
                "capability policy `{}` must declare at least one typed predicate axis",
                policy.id
            ));
        }
    }
    Ok(())
}

fn validate_profiles(
    profiles: &BTreeMap<String, super::document::HookClientProfileConfig>,
) -> Result<(), String> {
    for (profile_id, profile) in profiles {
        validate_non_empty(
            &format!("profiles.{profile_id}.languageId"),
            &profile.language_id,
        )?;
        validate_non_empty(
            &format!("profiles.{profile_id}.providerId"),
            &profile.provider_id,
        )?;
        if profile.extension_any.is_empty() {
            return Err(format!(
                "profiles.{profile_id}.extensionAny must contain at least one extension"
            ));
        }
        let mut extensions = HashSet::new();
        for extension in &profile.extension_any {
            let canonical = extension.trim().to_ascii_lowercase();
            if canonical.is_empty()
                || canonical.starts_with('.')
                || !canonical.chars().all(|ch| ch.is_ascii_alphanumeric())
            {
                return Err(format!(
                    "profiles.{profile_id}.extensionAny entries must be bare alphanumeric extensions, got {extension:?}"
                ));
            }
            if !extensions.insert(canonical) {
                return Err(format!(
                    "profiles.{profile_id}.extensionAny contains duplicate extension {extension:?}"
                ));
            }
        }
        let mut source_roots = HashSet::new();
        for source_root in &profile.source_root_any {
            let canonical = source_root.trim().trim_end_matches('/');
            let valid = !canonical.is_empty()
                && !canonical.starts_with('/')
                && !canonical.contains('\\')
                && !canonical.contains("://")
                && !canonical.contains(['*', '?', '[', ']', '{', '}'])
                && canonical.split('/').all(|component| {
                    !component.is_empty()
                        && component != ".."
                        && component
                            .chars()
                            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
                });
            if !valid {
                return Err(format!(
                    "profiles.{profile_id}.sourceRootAny entries must be normalized relative source roots, got {source_root:?}"
                ));
            }
            if !source_roots.insert(canonical.to_ascii_lowercase()) {
                return Err(format!(
                    "profiles.{profile_id}.sourceRootAny contains duplicate source root {source_root:?}"
                ));
            }
        }
        // Profiles form the declarative language catalog. Provider installation is
        // optional, so an unavailable profile remains valid and is inactive until
        // its matching language/provider descriptor is registered.
    }
    Ok(())
}

fn validate_rule_dispatches(rules: &[HookClientRuleConfig]) -> Result<(), String> {
    for rule in rules.iter().filter(|rule| rule.enabled) {
        let Some(dispatch) = rule.dispatch.as_ref() else {
            continue;
        };
        let prefix = format!("rules[{}].dispatch", rule.id);
        validate_identifier(&format!("{prefix}.agent"), dispatch.agent.as_str())?;
        validate_non_empty(
            &format!("{prefix}.receiptKind"),
            dispatch.receipt_kind.as_str(),
        )?;
    }
    Ok(())
}

fn validate_command_profiles(configs: &[HookClientCommandProfileConfig]) -> Result<(), String> {
    let mut ids = HashSet::new();
    for config in configs {
        validate_identifier("commandProfiles[].id", &config.id)?;
        if !ids.insert(config.id.as_str()) {
            return Err(format!("duplicate command profile id `{}`", config.id));
        }
        if config.language_ids.is_empty() {
            return Err(format!(
                "commandProfiles[{}].languageIds must not be empty",
                config.id
            ));
        }
        validate_unique_values("commandProfiles[].languageIds", &config.language_ids)?;
        validate_identifiers("commandProfiles[].languageIds[]", &config.language_ids)?;
        if config.categories.is_empty() {
            return Err(format!(
                "commandProfiles[{}].categories must not be empty",
                config.id
            ));
        }
        for (category, prefixes) in &config.categories {
            validate_identifier("commandProfiles[].categories key", category)?;
            if prefixes.is_empty() {
                return Err(format!(
                    "commandProfiles[{}].categories.{category} must not be empty",
                    config.id
                ));
            }
            validate_argv_prefix_patterns("commandProfiles[].categories argv prefixes", prefixes)?;
        }
    }
    Ok(())
}

fn validate_command_sets(configs: &[HookClientCommandSetConfig]) -> Result<(), String> {
    let mut ids = HashSet::new();
    for config in configs {
        validate_identifier("commandSets[].id", &config.id)?;
        if !ids.insert(config.id.as_str()) {
            return Err(format!("duplicate command set id `{}`", config.id));
        }
        if config.argv_prefix_any.is_empty() {
            return Err(format!(
                "commandSets[{}].argvPrefixAny must not be empty",
                config.id
            ));
        }
        validate_argv_prefix_patterns("commandSets[].argvPrefixAny", &config.argv_prefix_any)?;
    }
    Ok(())
}

fn validate_protocol(config: &HookClientConfigFile) -> Result<(), String> {
    expect_optional_field(
        "schemaId",
        config.schema_id.as_deref(),
        CLIENT_HOOK_CONFIG_SCHEMA_ID,
    )?;
    expect_optional_field(
        "schemaVersion",
        config.schema_version.as_deref(),
        CLIENT_HOOK_CONFIG_SCHEMA_VERSION,
    )?;
    expect_optional_field(
        "protocolId",
        config.protocol_id.as_deref(),
        HOOK_PROTOCOL_ID,
    )?;
    expect_optional_field(
        "protocolVersion",
        config.protocol_version.as_deref(),
        HOOK_PROTOCOL_VERSION,
    )?;
    Ok(())
}

fn validate_unique_rule_ids(rules: &[HookClientRuleConfig]) -> Result<(), String> {
    let mut seen = HashSet::new();
    for rule in rules {
        if !seen.insert(rule.id.as_str()) {
            return Err(format!("duplicate client hook rule id `{}`", rule.id));
        }
    }
    Ok(())
}

fn validate_rule_schema_shape(
    rules: &[HookClientRuleConfig],
    profiles: &[HookClientCommandProfileConfig],
    command_sets: &[HookClientCommandSetConfig],
    capability_policies: &[super::routing::HookClientCapabilityPolicyConfig],
) -> Result<(), String> {
    let capability_policy_ids = capability_policies
        .iter()
        .map(|policy| policy.id.as_str())
        .collect::<HashSet<_>>();
    for rule in rules {
        validate_identifier("rules[].id", &rule.id)?;
        validate_optional_non_empty("rules[].message", rule.message.as_deref())?;
        validate_optional_event(rule.event.as_deref())?;
        validate_optional_platform(rule.platform.as_deref())?;
        validate_unique_values("rules[].languageIds", &rule.language_ids)?;
        validate_identifiers("rules[].languageIds[]", &rule.language_ids)?;
        validate_match_schema_shape(
            &rule.match_config,
            profiles,
            command_sets,
            &capability_policy_ids,
        )?;
        if rule.terminal
            && !matches!(
                rule.decision,
                super::routing::HookClientConfigDecision::Allow
            )
        {
            return Err(format!(
                "terminal hook rule `{}` must use decision=allow",
                rule.id
            ));
        }
        if !rule
            .match_config
            .process_environment_assignment_any
            .is_empty()
            && !rule.terminal
        {
            return Err(format!(
                "hook rule `{}` using processEnvironmentAssignmentAny must be terminal",
                rule.id
            ));
        }
        for route in &rule.routes {
            validate_route_schema_shape(route)?;
        }
    }
    Ok(())
}

fn validate_match_schema_shape(
    match_config: &HookClientRuleMatchConfig,
    profiles: &[HookClientCommandProfileConfig],
    command_sets: &[HookClientCommandSetConfig],
    capability_policy_ids: &HashSet<&str>,
) -> Result<(), String> {
    for (axis, references) in [
        ("capabilityPolicyAll", &match_config.capability_policy_all),
        ("capabilityPolicyAny", &match_config.capability_policy_any),
        ("capabilityPolicyNone", &match_config.capability_policy_none),
    ] {
        validate_non_empty_values(&format!("rules[].match.{axis}[]"), references)?;
        validate_unique_values(&format!("rules[].match.{axis}"), references)?;
        for reference in references {
            if capability_policy_ids.get(reference.as_str()).is_none() {
                return Err(format!(
                    "rules[].match.{axis} references unknown capability policy `{reference}`"
                ));
            }
        }
    }
    let mut profile_references = HashSet::new();
    for reference in &match_config.command_profile_any {
        validate_identifier(
            "rules[].match.commandProfileAny[].profile",
            reference.profile.as_str(),
        )?;
        validate_identifier(
            "rules[].match.commandProfileAny[].category",
            reference.category.as_str(),
        )?;
        if !profile_references.insert((reference.profile.as_str(), reference.category.as_str())) {
            return Err(format!(
                "duplicate command profile reference `{}:{}`",
                reference.profile, reference.category
            ));
        }
    }
    expand_command_profile_prefixes(&match_config.command_profile_any, profiles)?;
    validate_non_empty_values(
        "rules[].match.commandSetAny[]",
        &match_config.command_set_any,
    )?;
    validate_unique_values("rules[].match.commandSetAny", &match_config.command_set_any)?;
    expand_command_set_prefixes(&match_config.command_set_any, command_sets)?;
    validate_optional_non_empty("rules[].match.tool", match_config.tool.as_deref())?;
    validate_non_empty_values("rules[].match.toolAny[]", &match_config.tool_any)?;
    validate_non_empty_values("rules[].match.commandAny[]", &match_config.command_any)?;
    validate_argv_prefix_patterns("rules[].match.argvPrefixAny", &match_config.argv_prefix_any)?;
    validate_non_empty_values("rules[].match.argvTokenAll[]", &match_config.argv_token_all)?;
    validate_unique_values("rules[].match.argvTokenAll", &match_config.argv_token_all)?;
    validate_environment_assignments(
        "rules[].match.processEnvironmentAssignmentAny",
        &match_config.process_environment_assignment_any,
    )?;
    validate_argv_pattern_bindings(&match_config.argv_pattern_any)?;
    validate_non_empty_values(
        "rules[].match.commandContainsAny[]",
        &match_config.command_contains_any,
    )?;
    validate_non_empty_values("rules[].match.pathAny[]", &match_config.path_any)?;
    validate_non_empty_values("rules[].match.pathGlobAny[]", &match_config.path_glob_any)?;
    validate_non_empty_values(
        "rules[].match.argvSourceAny[]",
        &match_config.argv_source_any,
    )?;
    validate_non_empty_values(
        "rules[].match.argvSourceGlobAny[]",
        &match_config.argv_source_glob_any,
    )?;
    validate_non_empty_values(
        "rules[].match.argvSourceExcludeFlagAny[]",
        &match_config.argv_source_exclude_flag_any,
    )?;
    if let Some(projection) = match_config.structured_projection.as_ref() {
        if !match_config.argv_workspace_regular_file {
            return Err(
                "rules[].match.structuredProjection requires argvWorkspaceRegularFile=true"
                    .to_string(),
            );
        }
        validate_required_binary_name(
            "rules[].match.structuredProjection.binary",
            &projection.binary,
        )?;
        validate_non_empty_values(
            "rules[].match.structuredProjection.optionalSubcommandAny[]",
            &projection.optional_subcommand_any,
        )?;
        validate_unique_values(
            "rules[].match.structuredProjection.optionalSubcommandAny",
            &projection.optional_subcommand_any,
        )?;
        validate_non_empty_values(
            "rules[].match.structuredProjection.optionAny[]",
            &projection.option_any,
        )?;
        validate_unique_values(
            "rules[].match.structuredProjection.optionAny",
            &projection.option_any,
        )?;
        for option in &projection.option_any {
            if !option.starts_with('-') {
                return Err(format!(
                    "rules[].match.structuredProjection.optionAny value `{option}` must start with `-`"
                ));
            }
        }
        let value_free_options = projection
            .option_any
            .iter()
            .map(String::as_str)
            .collect::<HashSet<_>>();
        for (option, arity) in &projection.option_value_arity {
            if !option.starts_with('-') || *arity == 0 {
                return Err(format!(
                    "rules[].match.structuredProjection.optionValueArity `{option}` must start with `-` and have positive arity"
                ));
            }
            if value_free_options.get(option.as_str()).is_some() {
                return Err(format!(
                    "rules[].match.structuredProjection option `{option}` cannot be both value-free and value-owning"
                ));
            }
        }
    }
    Ok(())
}

fn validate_argv_pattern_bindings(patterns: &[Vec<String>]) -> Result<(), String> {
    validate_argv_prefix_patterns("rules[].match.argvPatternAny", patterns)?;
    for pattern in patterns {
        validate_argv_pattern_binding(pattern)?;
    }
    Ok(())
}

fn validate_argv_pattern_binding(pattern: &[String]) -> Result<(), String> {
    let mut bindings = 0usize;
    let mut has_unknown_binding = false;
    for token in pattern {
        if token == "<registered-language>" {
            bindings += 1;
        } else if token.starts_with('<') && token.ends_with('>') {
            has_unknown_binding = true;
        }
    }
    if bindings > 1 {
        return Err(
            "rules[].match.argvPatternAny[] may contain at most one `<registered-language>` binding"
                .to_string(),
        );
    }
    if has_unknown_binding {
        return Err(
            "rules[].match.argvPatternAny[] contains an unknown schema binding".to_string(),
        );
    }
    Ok(())
}

fn validate_route_schema_shape(route: &HookClientRuleRouteConfig) -> Result<(), String> {
    validate_identifier("rules[].routes[].providerId", &route.provider_id)?;
    if let Some(language_id) = &route.language_id {
        validate_identifier("rules[].routes[].languageId", language_id)?;
    }
    if let Some(binary) = &route.binary {
        validate_binary_name("rules[].routes[].binary", binary)?;
    }
    if route.argv.is_empty() {
        return Err("rules[].routes[].argv must contain at least one item".to_string());
    }
    Ok(())
}

fn validate_agent_org_artifacts(
    config: Option<&HookClientAgentOrgArtifactsConfig>,
) -> Result<(), String> {
    let Some(config) = config else {
        return Ok(());
    };
    if config.inactive_after_minutes == 0 {
        return Err("agentOrgArtifacts.inactiveAfterMinutes must be greater than 0".to_string());
    }
    validate_non_empty("agentOrgArtifacts.artifactsPath", &config.artifacts_path)?;
    validate_non_empty("agentOrgArtifacts.entrySkillPath", &config.entry_skill_path)?;
    validate_agent_org_artifacts_archive_warning(&config.archive_warning)?;
    Ok(())
}

fn validate_agent_org_artifacts_archive_warning(
    config: &HookClientAgentOrgArtifactsArchiveWarningConfig,
) -> Result<(), String> {
    if config.active_org_file_threshold == 0 {
        return Err(
            "agentOrgArtifacts.archiveWarning.activeOrgFileThreshold must be greater than 0"
                .to_string(),
        );
    }
    if config.max_reported_files == 0 {
        return Err(
            "agentOrgArtifacts.archiveWarning.maxReportedFiles must be greater than 0".to_string(),
        );
    }
    validate_non_empty(
        "agentOrgArtifacts.archiveWarning.archivesDir",
        &config.archives_dir,
    )?;
    Ok(())
}

fn validate_identifiers(field: &str, values: &[String]) -> Result<(), String> {
    for value in values {
        validate_identifier(field, value)?;
    }
    Ok(())
}

fn validate_identifier(field: &str, value: &str) -> Result<(), String> {
    let mut bytes = value.bytes();
    if !matches!(bytes.next(), Some(b'a'..=b'z')) {
        return Err(format!("invalid {field} `{value}`"));
    }
    if bytes.all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-')) {
        Ok(())
    } else {
        Err(format!("invalid {field} `{value}`"))
    }
}

fn validate_optional_non_empty(field: &str, value: Option<&str>) -> Result<(), String> {
    if matches!(value, Some("")) {
        Err(format!("{field} must not be empty"))
    } else {
        Ok(())
    }
}

fn validate_non_empty_values(field: &str, values: &[String]) -> Result<(), String> {
    for value in values {
        if value.is_empty() {
            return Err(format!("{field} must not be empty"));
        }
    }
    Ok(())
}

fn validate_environment_assignments(field: &str, values: &[String]) -> Result<(), String> {
    for value in values {
        let Some((name, _)) = value.split_once('=') else {
            return Err(format!("{field} value `{value}` must be NAME=VALUE"));
        };
        let mut characters = name.chars();
        let valid_name = characters
            .next()
            .is_some_and(|character| character == '_' || character.is_ascii_alphabetic())
            && characters.all(|character| character == '_' || character.is_ascii_alphanumeric());
        if !valid_name {
            return Err(format!(
                "{field} value `{value}` has an invalid environment name"
            ));
        }
    }
    validate_unique_values(field, values)
}

fn validate_argv_prefix_patterns(field: &str, patterns: &[Vec<String>]) -> Result<(), String> {
    for (index, pattern) in patterns.iter().enumerate() {
        if pattern.is_empty() {
            return Err(format!("{field}[{index}] must not be empty"));
        }
        validate_non_empty_values(&format!("{field}[{index}][]"), pattern)?;
    }
    Ok(())
}

fn validate_non_empty(field: &str, value: &str) -> Result<(), String> {
    if value.is_empty() {
        Err(format!("{field} must not be empty"))
    } else {
        Ok(())
    }
}

fn validate_unique_values(field: &str, values: &[String]) -> Result<(), String> {
    let mut seen = HashSet::new();
    for value in values {
        if !seen.insert(value.as_str()) {
            return Err(format!("duplicate {field} `{value}`"));
        }
    }
    Ok(())
}

fn validate_optional_event(value: Option<&str>) -> Result<(), String> {
    let Some(value) = value else {
        return Ok(());
    };
    match value {
        "pre-tool" | "permission-request" | "post-tool" | "user-prompt" | "session-start"
        | "subagent-start" | "subagent-stop" | "stop" => Ok(()),
        _ => Err(format!("unsupported event `{value}`")),
    }
}

fn validate_optional_platform(value: Option<&str>) -> Result<(), String> {
    let Some(value) = value else {
        return Ok(());
    };
    match value {
        "codex" | "claude" | "unknown" => Ok(()),
        _ => Err(format!("unsupported platform `{value}`")),
    }
}

fn validate_binary_name(field: &str, value: &str) -> Result<(), String> {
    if !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
    {
        Ok(())
    } else {
        Err(format!("invalid {field} `{value}`"))
    }
}

fn validate_required_binary_name(field: &str, value: &str) -> Result<(), String> {
    let mut bytes = value.bytes();
    if !matches!(bytes.next(), Some(byte) if byte.is_ascii_alphanumeric()) {
        return Err(format!("invalid {field} `{value}`"));
    }
    if bytes.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-')) {
        Ok(())
    } else {
        Err(format!("invalid {field} `{value}`"))
    }
}

fn expect_optional_field(field: &str, actual: Option<&str>, expected: &str) -> Result<(), String> {
    if actual.is_some_and(|actual| actual != expected) {
        return Err(format!("expected {field}={expected}"));
    }
    Ok(())
}
