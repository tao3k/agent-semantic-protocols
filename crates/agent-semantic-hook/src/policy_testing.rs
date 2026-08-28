//! Host-envelope materialization for config-authoritative Hook coverage plans.

use agent_semantic_config::{
    HookClientConfigFile, HookPolicyCoverageSettings, HookPolicyCoverageSurface,
    derive_hook_policy_coverage_cases,
};
use serde_json::Value;

use crate::tool_action::shell_host_envelopes;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HookPolicyWitnessPolarity {
    Black,
    White,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HookPolicyCombinatorialStrategy {
    pub max_wrapper_depth: usize,
    pub include_negative_extension_mutation: bool,
}

#[derive(Clone, Debug)]
pub struct HookPolicyWitness {
    pub id: String,
    pub polarity: HookPolicyWitnessPolarity,
    pub expected_decision: crate::DecisionKind,
    pub language_id: String,
    pub provider_id: String,
    pub source_extension: String,
    pub path: String,
    pub tool_name: String,
    pub tool_input: Value,
    pub expected_rule_id: Option<String>,
    pub expected_reason_kind: String,
    pub expected_language_ids: Vec<String>,
    pub expected_routes: Vec<(String, String)>,
    pub envelope_axis: String,
    pub envelope_slot: usize,
    pub command_axis: Option<String>,
    pub command_prefix: Option<Vec<String>>,
    pub wrapper_depth: usize,
}

/// Materialize the config-generated abstract covering plan through the same
/// Host-envelope shapes consumed by production ToolAction normalization.
pub fn combinatorial_policy_witnesses(
    config: &HookClientConfigFile,
    strategy: HookPolicyCombinatorialStrategy,
) -> Result<Vec<HookPolicyWitness>, String> {
    let shell_envelopes = shell_host_envelopes("__ASP_COVERAGE_COMMAND__");
    let reference_config =
        crate::ClientHookConfig::compile_policy_coverage_reference(config.clone())?;
    let mut reference_runtime = crate::HookRuntime {
        project_root: ".".to_owned(),
        rankers: Vec::new(),
        providers: Vec::new(),
        policy_providers: Vec::new(),
    };
    reference_config.apply_language_provider_projection(&mut reference_runtime)?;
    derive_hook_policy_coverage_cases(
        config,
        HookPolicyCoverageSettings {
            max_wrapper_depth: strategy.max_wrapper_depth,
            include_negative_extension_mutation: strategy.include_negative_extension_mutation,
            shell_envelope_count: shell_envelopes.len(),
        },
    )?
    .into_iter()
    .map(|case| {
        let (tool_name, tool_input, envelope_axis, command_axis) = match case.surface {
            HookPolicyCoverageSurface::Shell => {
                let prefix = case.command_prefix.as_deref().ok_or_else(|| {
                    "config shell coverage case omitted command prefix".to_owned()
                })?;
                let command = wrapped_command(prefix, &case.path, case.wrapper_depth);
                let envelopes = shell_host_envelopes(&command);
                let (tool_name, tool_input) =
                    envelopes.get(case.envelope_slot).cloned().ok_or_else(|| {
                        "config coverage selected an invalid shell envelope".to_owned()
                    })?;
                (
                    tool_name,
                    tool_input,
                    format!("shell:{}", case.envelope_slot),
                    Some(prefix.join(" ")),
                )
            }
        };
        let payload = serde_json::json!({"tool_name": tool_name, "tool_input": tool_input});
        let reference_decision =
            crate::classify_hook_with_config(crate::HookClassificationRequest {
                registry: &reference_runtime,
                config: &reference_config,
                platform: "codex",
                event: "pre-tool",
                payload: &payload,
            });
        let reference_json = serde_json::to_value(&reference_decision)
            .map_err(|error| format!("serialize Hook coverage reference decision: {error}"))?;
        let polarity = match case.polarity {
            agent_semantic_config::HookPolicyCoveragePolarity::Black => {
                HookPolicyWitnessPolarity::Black
            }
            agent_semantic_config::HookPolicyCoveragePolarity::White => {
                HookPolicyWitnessPolarity::White
            }
        };
        let expected_rule_id = reference_json["fields"]["configRuleId"]
            .as_str()
            .map(str::to_owned);
        let expected_reason_kind = reference_json["reasonKind"]
            .as_str()
            .ok_or_else(|| "Hook coverage reference omitted reasonKind".to_owned())?
            .to_owned();
        let expected_language_ids = reference_json["languageIds"]
            .as_array()
            .ok_or_else(|| "Hook coverage reference omitted languageIds".to_owned())?
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect();
        let expected_routes = reference_json["routes"]
            .as_array()
            .ok_or_else(|| "Hook coverage reference omitted routes".to_owned())?
            .iter()
            .filter_map(|route| {
                Some((
                    route["providerId"].as_str()?.to_owned(),
                    route["languageId"].as_str()?.to_owned(),
                ))
            })
            .collect();
        Ok(HookPolicyWitness {
            id: format!(
                "{:?}:{:?}:{}:{}:{}:{}",
                case.polarity,
                case.surface,
                case.language_id,
                case.source_extension,
                case.envelope_slot,
                case.wrapper_depth
            ),
            polarity,
            expected_decision: reference_decision.decision,
            language_id: case.language_id,
            provider_id: case.provider_id,
            source_extension: case.source_extension,
            path: case.path,
            tool_name: payload["tool_name"]
                .as_str()
                .expect("typed tool name")
                .to_owned(),
            tool_input: payload["tool_input"].clone(),
            expected_rule_id,
            expected_reason_kind,
            expected_language_ids,
            expected_routes,
            envelope_axis,
            envelope_slot: case.envelope_slot,
            command_axis,
            command_prefix: case.command_prefix,
            wrapper_depth: case.wrapper_depth,
        })
    })
    .collect()
}

/// Generate config-owned black witnesses whose registered source is surrounded
/// by unrelated unregistered argv siblings. The command is assembled as words
/// before Host-envelope serialization, so quoting and wrapper structure remain
/// identical to the ordinary combinatorial witness.
pub fn combinatorial_positional_shell_witnesses(
    config: &HookClientConfigFile,
    strategy: HookPolicyCombinatorialStrategy,
) -> Result<Vec<HookPolicyWitness>, String> {
    let extensions = config
        .profiles
        .values()
        .flat_map(|profile| {
            profile
                .extension_any
                .iter()
                .map(|extension| format!(".{extension}"))
        })
        .collect::<std::collections::BTreeSet<_>>();
    combinatorial_policy_witnesses(config, strategy)?
        .into_iter()
        .filter(|witness| {
            witness.polarity == HookPolicyWitnessPolarity::Black && witness.command_prefix.is_some()
        })
        .map(|mut witness| {
            let auxiliary = agent_semantic_config::mutate_path_outside_registered_extensions(
                &witness.path,
                &extensions,
            );
            let command = wrapped_command_with_path_siblings(
                witness
                    .command_prefix
                    .as_deref()
                    .expect("filtered shell witness has command prefix"),
                &witness.path,
                witness.wrapper_depth,
                std::slice::from_ref(&auxiliary),
                std::slice::from_ref(&auxiliary),
            );
            let envelopes = shell_host_envelopes(&command);
            let (tool_name, tool_input) = envelopes
                .get(witness.envelope_slot)
                .cloned()
                .ok_or_else(|| "config coverage selected an invalid shell envelope".to_owned())?;
            witness.id.push_str(":positional");
            witness.tool_name = tool_name;
            witness.tool_input = tool_input;
            Ok(witness)
        })
        .collect()
}

fn wrapped_command(prefix: &[String], path: &str, wrapper_depth: usize) -> String {
    wrapped_command_with_path_siblings(prefix, path, wrapper_depth, &[], &[])
}

fn wrapped_command_with_path_siblings(
    prefix: &[String],
    path: &str,
    wrapper_depth: usize,
    before: &[String],
    after: &[String],
) -> String {
    let mut words = (0..wrapper_depth)
        .map(|depth| format!("generated-wrapper-{depth}"))
        .collect::<Vec<_>>();
    words.extend(prefix.iter().cloned());
    words.extend(before.iter().cloned());
    words.push(path.to_owned());
    words.extend(after.iter().cloned());
    words.join(" ")
}

#[cfg(test)]
#[path = "../tests/unit/policy_testing.rs"]
mod tests;
