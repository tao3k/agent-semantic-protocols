use std::collections::{BTreeMap, BTreeSet};

use agent_semantic_config::HookClientConfigFile;
use serde::Serialize;
use serde_json::Value;

use crate::aot_evaluator::{HOOK_GENERATION_SCHEMA_ID, HOOK_GENERATION_SCHEMA_VERSION};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OwnedCompiledHookGeneration {
    schema_id: &'static str,
    schema_version: u32,
    generation_digest: String,
    rules: Vec<OwnedCompiledDecisionRule>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OwnedCompiledDecisionRule {
    id: String,
    matchers: Vec<String>,
    actions: Vec<String>,
    registered_extensions: Vec<String>,
    decision: String,
    reason_kind: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    profile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    route: Option<String>,
}

pub fn compile_aot_hook_generation(
    config: &HookClientConfigFile,
    generation_digest: impl Into<String>,
) -> Result<Vec<u8>, String> {
    let projection = serde_json::to_value(config)
        .map_err(|error| format!("failed to project canonical Hook config: {error}"))?;
    compile_aot_hook_generation_projection(&projection, generation_digest)
}

fn compile_aot_hook_generation_projection(
    projection: &Value,
    generation_digest: impl Into<String>,
) -> Result<Vec<u8>, String> {
    let profile_extensions = compile_profile_extensions(projection);
    let rules = projection
        .get("rules")
        .and_then(Value::as_array)
        .ok_or("canonical Hook config has no rules")?
        .iter()
        .map(|rule| compile_rule(rule, &profile_extensions))
        .collect::<Result<Vec<_>, _>>()?;
    serde_json::to_vec(&OwnedCompiledHookGeneration {
        schema_id: HOOK_GENERATION_SCHEMA_ID,
        schema_version: HOOK_GENERATION_SCHEMA_VERSION,
        generation_digest: generation_digest.into(),
        rules,
    })
    .map_err(|error| format!("failed to encode compiled HookGeneration: {error}"))
}

fn compile_profile_extensions(projection: &Value) -> BTreeMap<String, Vec<String>> {
    projection
        .get("profiles")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .map(|(name, profile)| {
            let mut extensions = profile
                .get("extensions")
                .or_else(|| profile.get("extensionList"))
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(|extension| extension.trim_start_matches('.').to_owned())
                .collect::<Vec<_>>();
            extensions.sort();
            extensions.dedup();
            (name.clone(), extensions)
        })
        .collect()
}

fn compile_rule(
    rule: &Value,
    profile_extensions: &BTreeMap<String, Vec<String>>,
) -> Result<OwnedCompiledDecisionRule, String> {
    let id = required_string(rule, "id")?;
    let matcher = required_string(rule, "matcher")?;
    let mut matchers = matcher
        .split('|')
        .map(str::trim)
        .filter(|matcher| !matcher.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    matchers.sort();
    matchers.dedup();
    if matchers.is_empty() {
        return Err(format!("Hook rule {id:?} has an empty matcher"));
    }

    let profiles = rule
        .get("profilesList")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    let mut registered_extensions = BTreeSet::new();
    for profile in &profiles {
        let extensions = profile_extensions
            .get(*profile)
            .ok_or_else(|| format!("Hook rule {id:?} references unknown profile {profile:?}"))?;
        registered_extensions.extend(extensions.iter().cloned());
    }

    let decision = rule
        .get("decision")
        .and_then(|decision| {
            decision
                .as_str()
                .or_else(|| decision.get("effect").and_then(Value::as_str))
        })
        .ok_or_else(|| format!("Hook rule {id:?} has no decision"))?
        .to_owned();
    Ok(OwnedCompiledDecisionRule {
        id,
        matchers,
        actions: rule
            .get("actions")
            .and_then(serde_json::Value::as_array)
            .map(|actions| {
                actions
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(|action| action.to_ascii_lowercase())
                    .collect()
            })
            .unwrap_or_default(),
        registered_extensions: registered_extensions.into_iter().collect(),
        decision,
        reason_kind: optional_string(rule, "reasonKind")
            .unwrap_or_else(|| "hook-policy-decision".to_owned()),
        message: optional_string(rule, "message").unwrap_or_default(),
        profile: profiles.first().map(|profile| (*profile).to_owned()),
        language: profiles.first().map(|profile| (*profile).to_owned()),
        route: optional_string(rule, "route"),
    })
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
