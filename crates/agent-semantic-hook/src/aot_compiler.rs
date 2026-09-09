// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::collections::BTreeMap;

use agent_semantic_config::HookClientConfigFile;
use serde::Serialize;
use serde_json::Value;

use crate::aot_evaluator::HOOK_POLICY_BUNDLE_SCHEMA_ID;
use crate::aot_evaluator::HOOK_POLICY_BUNDLE_SCHEMA_VERSION;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OwnedCompiledHookPolicyBundle {
    schema_id: &'static str,
    schema_version: u32,
    generation_digest: String,
    command_action_patterns: Vec<OwnedCommandActionPattern>,
    registered_languages: Vec<String>,
    agent_calling_pattern: String,
    rules: Vec<OwnedCompiledDecisionRule>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OwnedCommandActionPattern {
    action: String,
    argv_pattern_any: Vec<Vec<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OwnedCompiledDecisionRule {
    id: String,
    priority: i64,
    matchers: Vec<String>,
    wrapped_command: bool,
    actions: Vec<String>,
    registered_extensions: Vec<String>,
    decision: String,
    intent: String,
    reason_kind: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    profile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    route: Option<String>,
    argv_prefix_any: Vec<Vec<String>>,
    command_contains_any: Vec<String>,
    argv_token_all: Vec<String>,
    argv_source_glob_any: Vec<String>,
    command_any: Vec<String>,
    process_environment_assignment_any: Vec<String>,
    path_glob_any: Vec<String>,
    argv_workspace_regular_file: bool,
    argv_structured_document_file: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    structured_projection: Option<Value>,
}

#[derive(Clone, Debug)]
struct CompiledProfile {
    language_id: String,
    extensions: Vec<String>,
}

pub fn compile_aot_hook_policy_bundle(
    config: &HookClientConfigFile,
    generation_digest: impl Into<String>,
) -> Result<Vec<u8>, String> {
    let projection = serde_json::to_value(config)
        .map_err(|error| format!("failed to project canonical Hook config: {error}"))?;
    compile_aot_hook_policy_bundle_projection(&projection, generation_digest)
}

/// Compile the matcher, policy and Agent registry embedded in this executable.
///
/// The canonical Runtime Hook binary is the only executable authority for Hook
/// evaluation. The returned digest therefore identifies the embedded policy
/// inputs; there is no separately published evaluator binary or
/// standalone stable-path Hook executable.
pub fn compile_embedded_hook_policy_bundle() -> Result<Vec<u8>, String> {
    let generation_digest = embedded_hook_policy_content_digest()?;
    let config = agent_semantic_config::default_hook_client_config_file()
        .map_err(|error| format!("load embedded Hook config: {error}"))?;
    compile_aot_hook_policy_bundle(&config, generation_digest)
}

/// Compile the serving Hook policy from the immutable system defaults plus an
/// optional user-level State Home overlay.
///
/// The system template is always the base policy.  When present,
/// `$ASP_STATE_HOME/control/config/hook-client.toml` is a declarative overlay, not a second
/// executable authority and not a best-effort hint: invalid user policy fails
/// closed before a Host action can be admitted.  The normal `asp-hook` path
/// calls this function directly.
pub fn compile_serving_hook_policy_bundle() -> Result<Vec<u8>, String> {
    compile_serving_hook_policy_bundle_at(
        crate::hook_config_global::default_global_client_config_path().as_deref(),
    )
}

/// Compile the serving Hook policy from an explicit State Home config path.
///
/// Control-plane acceptance uses this read-only entry point so it can measure
/// the same overlay that the dedicated Hook process will serve without
/// mutating process-global environment variables.
pub fn compile_serving_hook_policy_bundle_at(
    path: Option<&std::path::Path>,
) -> Result<Vec<u8>, String> {
    let Some(path) = path else {
        return compile_embedded_hook_policy_bundle();
    };
    if !path.exists() {
        return compile_embedded_hook_policy_bundle();
    }
    if !path.is_file() {
        return Err(format!(
            "Hook user config is not a regular file: {}",
            path.display()
        ));
    }
    let source = std::fs::read(&path)
        .map_err(|error| format!("read Hook user config {}: {error}", path.display()))?;
    let config = agent_semantic_config::load_hook_client_config_overlay_file(&path)
        .map_err(|error| format!("load Hook user config {}: {error}", path.display()))?;
    let mut identity = blake3::Hasher::new();
    identity.update(b"agent-semantic-hook-serving-policy.v1\0");
    identity.update(embedded_hook_policy_content_digest()?.as_bytes());
    identity.update(b"\0");
    identity.update(&source);
    compile_aot_hook_policy_bundle(
        &config,
        format!("blake3-256:{}", identity.finalize().to_hex()),
    )
}

/// Project the identity of the immutable Hook inputs linked into this build.
///
/// This is deliberately separate from AOT compilation: publication probes
/// must not parse Config or construct matcher rules merely to identify the
/// candidate executable.
pub fn embedded_hook_policy_content_digest() -> Result<String, String> {
    let config_source = agent_semantic_config::default_hook_client_config_template();
    let registry = agent_semantic_config::embedded_agent_assets::embedded_agent_assets()
        .iter()
        .find(|asset| asset.file_name == "config.toml")
        .ok_or("embedded Hook Agent registry is missing config.toml")?
        .contents;
    let mut identity = blake3::Hasher::new();
    identity.update(b"agent-semantic-hook-embedded-policy\0");
    identity.update(config_source.as_bytes());
    identity.update(b"\0");
    identity.update(registry);
    Ok(format!("blake3-256:{}", identity.finalize().to_hex()))
}

fn compile_aot_hook_policy_bundle_projection(
    projection: &Value,
    generation_digest: impl Into<String>,
) -> Result<Vec<u8>, String> {
    let profiles = compile_profiles(projection)?;
    let command_profiles = compile_command_profile_patterns(projection)?;
    let command_sets = compile_command_sets(projection)?;
    let agent_calling_pattern = projection
        .get("agentCalling")
        .and_then(|calling| {
            calling
                .get("platformPatterns")
                .and_then(|patterns| patterns.get("codex"))
                .and_then(Value::as_str)
                .or_else(|| calling.get("defaultPattern").and_then(Value::as_str))
        })
        .ok_or("canonical Hook config has no Codex/default Agent calling pattern")?
        .to_owned();
    let rule_values = projection
        .get("rules")
        .and_then(Value::as_array)
        .ok_or("canonical Hook config has no rules")?;
    let mut rules = Vec::new();
    for rule in rule_values {
        rules.extend(compile_rules(
            rule,
            &profiles,
            &command_profiles,
            &command_sets,
        )?);
    }
    rules.sort_by(|left, right| {
        right
            .priority
            .cmp(&left.priority)
            .then_with(|| left.id.cmp(&right.id))
            .then_with(|| left.profile.cmp(&right.profile))
    });
    let mut registered_languages = profiles
        .values()
        .map(|profile| profile.language_id.clone())
        .collect::<Vec<_>>();
    registered_languages.sort();
    registered_languages.dedup();
    serde_json::to_vec(&OwnedCompiledHookPolicyBundle {
        schema_id: HOOK_POLICY_BUNDLE_SCHEMA_ID,
        schema_version: HOOK_POLICY_BUNDLE_SCHEMA_VERSION,
        generation_digest: generation_digest.into(),
        command_action_patterns: compile_command_action_patterns(projection)?,
        registered_languages,
        agent_calling_pattern,
        rules,
    })
    .map_err(|error| format!("failed to encode compiled HookPolicyBundle: {error}"))
}

fn compile_command_action_patterns(
    projection: &Value,
) -> Result<Vec<OwnedCommandActionPattern>, String> {
    let mut families = projection
        .get("commandActionPatterns")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|family| {
            let action = family
                .get("action")
                .and_then(Value::as_str)
                .filter(|action| matches!(*action, "read" | "search"))
                .ok_or("commandActionPatterns action must be read or search")?
                .to_owned();
            let mut argv_pattern_any = family
                .get("argvPatternAny")
                .and_then(Value::as_array)
                .ok_or("commandActionPatterns entries must contain argvPatternAny")?
                .iter()
                .map(|pattern| {
                    pattern
                        .as_array()
                        .ok_or("commandActionPatterns argvPatternAny entries must be argv arrays")?
                        .iter()
                        .map(|token| {
                            token
                                .as_str()
                                .filter(|token| !token.is_empty())
                                .map(str::to_owned)
                                .ok_or("commandActionPatterns tokens must be non-empty strings")
                        })
                        .collect::<Result<Vec<_>, _>>()
                })
                .collect::<Result<Vec<_>, _>>()?;
            if argv_pattern_any.iter().any(Vec::is_empty) || argv_pattern_any.is_empty() {
                return Err(
                    "commandActionPatterns argvPatternAny entries must not be empty".to_owned(),
                );
            }
            argv_pattern_any.sort();
            argv_pattern_any.dedup();
            Ok(OwnedCommandActionPattern {
                action,
                argv_pattern_any,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    families.sort_by(|left, right| left.action.cmp(&right.action));
    Ok(families)
}

fn compile_profiles(projection: &Value) -> Result<BTreeMap<String, CompiledProfile>, String> {
    projection
        .get("profiles")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .map(|(name, profile)| {
            let language_id = profile
                .get("languageId")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("Hook profile {name:?} has no languageId"))?
                .to_owned();
            let mut extensions = profile
                .get("extensionAny")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(|extension| extension.trim_start_matches('.').to_owned())
                .collect::<Vec<_>>();
            extensions.sort();
            extensions.dedup();
            Ok((
                name.clone(),
                CompiledProfile {
                    language_id,
                    extensions,
                },
            ))
        })
        .collect()
}

fn compile_rules(
    rule: &Value,
    profile_catalog: &BTreeMap<String, CompiledProfile>,
    command_profiles: &BTreeMap<(String, String), Vec<Vec<String>>>,
    command_sets: &BTreeMap<String, Vec<Vec<String>>>,
) -> Result<Vec<OwnedCompiledDecisionRule>, String> {
    let id = required_string(rule, "id")?;
    let mut matchers = optional_string(rule, "matcher")
        .into_iter()
        .flat_map(|matcher| {
            matcher
                .split('|')
                .map(str::trim)
                .filter(|matcher| !matcher.is_empty())
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let actions: Vec<String> = rule
        .get("actions")
        .and_then(serde_json::Value::as_array)
        .map(|actions| {
            actions
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(|action| action.to_ascii_lowercase())
                .collect()
        })
        .unwrap_or_default();
    let wrapped_command = actions.iter().any(|action| action == "read")
        || rule
            .get("matcherPolicies")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .any(|policy| policy == "wrapped_command");
    // A matcher-less shell rule is a legacy shorthand for Bash admission.
    // Do not add Bash to an explicitly native `Read` rule: Host action
    // boundaries are part of the policy identity.
    if wrapped_command && matchers.is_empty() {
        matchers.push("Bash".to_owned());
    }
    matchers.sort();
    matchers.dedup();
    if matchers.is_empty() {
        return Err(format!("Hook rule {id:?} has no Host matcher projection"));
    }

    let compiled_match = compile_rule_match(rule, command_profiles, command_sets)?;
    let priority = rule.get("priority").and_then(Value::as_i64).unwrap_or(0);

    let profile_ids = rule
        .get("profilesList")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    let decision = rule
        .get("decision")
        .and_then(|decision| {
            decision
                .as_str()
                .or_else(|| decision.get("effect").and_then(Value::as_str))
        })
        .ok_or_else(|| format!("Hook rule {id:?} has no decision"))?
        .to_owned();
    let route = rule
        .get("dispatch")
        .and_then(|dispatch| dispatch.get("agent"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let intent = optional_string(rule, "intent").unwrap_or_else(|| {
        if actions.iter().any(|action| action == "read") {
            "source-read".to_owned()
        } else {
            "host-tool".to_owned()
        }
    });
    let reason_kind =
        optional_string(rule, "reasonKind").unwrap_or_else(|| "hook-policy-decision".to_owned());
    let message = optional_string(rule, "message").unwrap_or_default();

    if profile_ids.is_empty() {
        let registered_extensions = if actions.iter().any(|action| action == "read") {
            extensions_from_globs(&compiled_match.path_glob_any)
        } else {
            Vec::new()
        };
        return Ok(vec![OwnedCompiledDecisionRule {
            id,
            priority,
            matchers,
            wrapped_command,
            actions,
            registered_extensions,
            decision,
            intent,
            reason_kind,
            message,
            profile: None,
            language: None,
            route,
            argv_prefix_any: compiled_match.argv_prefix_any,
            command_contains_any: compiled_match.command_contains_any,
            argv_token_all: compiled_match.argv_token_all,
            argv_source_glob_any: compiled_match.argv_source_glob_any,
            command_any: compiled_match.command_any,
            process_environment_assignment_any: compiled_match.process_environment_assignment_any,
            path_glob_any: compiled_match.path_glob_any,
            argv_workspace_regular_file: compiled_match.argv_workspace_regular_file,
            argv_structured_document_file: compiled_match.argv_structured_document_file,
            structured_projection: compiled_match.structured_projection,
        }]);
    }

    let mut compiled = Vec::with_capacity(profile_ids.len());
    for profile_id in profile_ids {
        let profile = profile_catalog
            .get(profile_id)
            .ok_or_else(|| format!("Hook rule {id:?} references unknown profile {profile_id:?}"))?;
        compiled.push(OwnedCompiledDecisionRule {
            id: id.clone(),
            priority,
            matchers: matchers.clone(),
            wrapped_command,
            actions: actions.clone(),
            registered_extensions: profile.extensions.clone(),
            decision: decision.clone(),
            intent: intent.clone(),
            reason_kind: reason_kind.clone(),
            message: message.clone(),
            profile: Some(profile_id.to_owned()),
            language: Some(profile.language_id.clone()),
            route: route.clone(),
            argv_prefix_any: compiled_match.argv_prefix_any.clone(),
            command_contains_any: compiled_match.command_contains_any.clone(),
            argv_token_all: compiled_match.argv_token_all.clone(),
            argv_source_glob_any: compiled_match.argv_source_glob_any.clone(),
            command_any: compiled_match.command_any.clone(),
            process_environment_assignment_any: compiled_match
                .process_environment_assignment_any
                .clone(),
            path_glob_any: compiled_match.path_glob_any.clone(),
            argv_workspace_regular_file: compiled_match.argv_workspace_regular_file,
            argv_structured_document_file: compiled_match.argv_structured_document_file,
            structured_projection: compiled_match.structured_projection.clone(),
        });
    }
    Ok(compiled)
}

fn extensions_from_globs(patterns: &[String]) -> Vec<String> {
    let mut extensions = patterns
        .iter()
        .filter_map(|pattern| pattern.rsplit_once("*.").map(|(_, extension)| extension))
        .filter(|extension| !extension.is_empty() && !extension.contains(['*', '/', '\\']))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    extensions.sort();
    extensions.dedup();
    extensions
}

#[derive(Clone, Default)]
struct CompiledRuleMatch {
    argv_prefix_any: Vec<Vec<String>>,
    command_contains_any: Vec<String>,
    argv_token_all: Vec<String>,
    argv_source_glob_any: Vec<String>,
    command_any: Vec<String>,
    process_environment_assignment_any: Vec<String>,
    path_glob_any: Vec<String>,
    argv_workspace_regular_file: bool,
    argv_structured_document_file: bool,
    structured_projection: Option<Value>,
}

fn compile_rule_match(
    rule: &Value,
    command_profiles: &BTreeMap<(String, String), Vec<Vec<String>>>,
    command_sets: &BTreeMap<String, Vec<Vec<String>>>,
) -> Result<CompiledRuleMatch, String> {
    let Some(rule_match) = rule.get("match") else {
        return Ok(CompiledRuleMatch::default());
    };
    let mut compiled = CompiledRuleMatch {
        argv_prefix_any: string_matrix(rule_match, "argvPatternAny")?,
        command_contains_any: string_list(rule_match, "commandContainsAny")?,
        argv_token_all: string_list(rule_match, "argvTokenAll")?,
        argv_source_glob_any: string_list(rule_match, "argvSourceGlobAny")?,
        command_any: string_list(rule_match, "commandAny")?,
        process_environment_assignment_any: string_list(
            rule_match,
            "processEnvironmentAssignmentAny",
        )?,
        path_glob_any: string_list(rule_match, "pathGlobAny")?,
        argv_workspace_regular_file: rule_match
            .get("argvWorkspaceRegularFile")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        argv_structured_document_file: rule_match
            .get("argvStructuredDocumentFile")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        structured_projection: rule_match.get("structuredProjection").cloned(),
    };
    for reference in rule_match
        .get("commandProfileAny")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let profile = required_string(reference, "profile")?;
        let category = required_string(reference, "category")?;
        let patterns = command_profiles
            .get(&(profile.clone(), category.clone()))
            .ok_or_else(|| {
                format!("Hook rule references unknown command profile {profile:?}/{category:?}")
            })?;
        compiled.argv_prefix_any.extend(patterns.iter().cloned());
    }
    for command_set in string_list(rule_match, "commandSetAny")? {
        let patterns = command_sets
            .get(&command_set)
            .ok_or_else(|| format!("Hook rule references unknown command set {command_set:?}"))?;
        compiled.argv_prefix_any.extend(patterns.iter().cloned());
    }
    compiled.argv_prefix_any.sort();
    compiled.argv_prefix_any.dedup();
    Ok(compiled)
}

fn compile_command_profile_patterns(
    projection: &Value,
) -> Result<BTreeMap<(String, String), Vec<Vec<String>>>, String> {
    let mut catalog = BTreeMap::new();
    for profile in projection
        .get("commandProfiles")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let id = required_string(profile, "id")?;
        for (category, patterns) in profile
            .get("categories")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
        {
            catalog.insert(
                (id.clone(), category.clone()),
                string_matrix_value(patterns, "command profile category")?,
            );
        }
    }
    Ok(catalog)
}

fn compile_command_sets(projection: &Value) -> Result<BTreeMap<String, Vec<Vec<String>>>, String> {
    projection
        .get("commandSets")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|set| {
            Ok((
                required_string(set, "id")?,
                string_matrix(set, "argvPrefixAny")?,
            ))
        })
        .collect()
}

fn string_list(value: &Value, field: &str) -> Result<Vec<String>, String> {
    value
        .get(field)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|item| {
            item.as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("{field} entries must be strings"))
        })
        .collect()
}

fn string_matrix(value: &Value, field: &str) -> Result<Vec<Vec<String>>, String> {
    match value.get(field) {
        Some(matrix) => string_matrix_value(matrix, field),
        None => Ok(Vec::new()),
    }
}

fn string_matrix_value(value: &Value, label: &str) -> Result<Vec<Vec<String>>, String> {
    value
        .as_array()
        .ok_or_else(|| format!("{label} must be an array"))?
        .iter()
        .map(|row| {
            row.as_array()
                .ok_or_else(|| format!("{label} entries must be argv arrays"))?
                .iter()
                .map(|token| {
                    token
                        .as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| format!("{label} argv tokens must be strings"))
                })
                .collect()
        })
        .collect()
}

fn required_string(value: &Value, field: &str) -> Result<String, String> {
    optional_string(value, field).ok_or_else(|| format!("missing required field {field:?}"))
}

fn optional_string(value: &Value, field: &str) -> Option<String> {
    value.get(field).and_then(Value::as_str).map(str::to_owned)
}

#[cfg(test)]
#[path = "../tests/unit/aot_compiler.rs"]
mod tests;
