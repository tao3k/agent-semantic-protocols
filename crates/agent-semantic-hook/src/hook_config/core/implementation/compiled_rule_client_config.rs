//! Implements runtime accessors and classification for compiled hook config.

use std::{borrow::Cow, path::Path};

use super::{
    AgentOrgArtifactsArchiveWarning, AgentOrgArtifactsRecovery, AspSessionPolicy, ClientHookConfig,
    CompiledHookRule, CompiledRecoveryPromptConfig, HookClientConfigFile, HookDecision,
    HookRuntime, ToolAction, compile_agent_org_artifacts_config, merge_agent_session_messages,
};

impl Default for ClientHookConfig {
    fn default() -> Self {
        let config = agent_semantic_config::default_hook_client_config_file()
            .expect("embedded hook client config must remain valid");
        compile_config(config).expect("embedded hook client rules must compile")
    }
}

impl ClientHookConfig {
    /// Return the agent-facing session message templates.
    pub fn agent_session_messages(
        &self,
    ) -> &agent_semantic_config::HookClientAgentSessionMessagesConfig {
        &self.agent_session_messages
    }

    /// Return the resident child session name used for ASP exploration.
    pub fn resident_asp_explore_child_name(&self) -> &str {
        self.asp_session_policy.resident_child_name()
    }

    /// Return the configured Codex agent name used for ASP exploration.
    pub fn resident_asp_explore_codex_agent_name(&self) -> &str {
        self.asp_session_policy.resident_codex_agent_name()
    }
}

impl ClientHookConfig {
    pub fn contract_fingerprint(&self) -> Option<&str> {
        self.contract_fingerprint.as_deref()
    }

    pub(crate) fn semantic_ast_patch_enabled(&self) -> bool {
        !self.semantic_ast_patch_disabled
    }

    /// Return ASP session routing policy compiled from hook config.
    pub fn asp_session_policy(&self) -> &AspSessionPolicy {
        &self.asp_session_policy
    }

    pub(crate) fn recovery_prompt(&self) -> &CompiledRecoveryPromptConfig {
        &self.recovery_prompt
    }

    pub(crate) fn agent_org_artifacts_recovery(
        &self,
        project_root: impl AsRef<Path>,
        session_id: Option<&str>,
    ) -> Option<AgentOrgArtifactsRecovery> {
        self.agent_org_artifacts
            .recovery(project_root.as_ref(), session_id)
    }

    pub(crate) fn agent_org_artifacts_archive_warning(
        &self,
        project_root: impl AsRef<Path>,
    ) -> Option<AgentOrgArtifactsArchiveWarning> {
        self.agent_org_artifacts
            .archive_warning(project_root.as_ref())
    }

    pub(crate) fn classify(
        &self,
        runtime: &HookRuntime,
        platform: &str,
        event: &str,
        action: &ToolAction,
    ) -> Option<HookDecision> {
        let mut command_tokens: Option<Option<Cow<'_, [String]>>> = None;
        for rule in &self.rules {
            let needs_command_tokens = rule.match_config.needs_command_tokens()
                || matches!(
                    rule.decision_materializer,
                    Some(
                        agent_semantic_config::HookClientDecisionMaterializer::AgentSearchJson
                            | agent_semantic_config::HookClientDecisionMaterializer::SourceAccess
                    )
                );
            let command_token_slice = if needs_command_tokens {
                command_tokens
                    .get_or_insert_with(|| action.command_tokens())
                    .as_deref()
            } else {
                None
            };
            if !rule.matches_before_paths(runtime, platform, event, action, command_token_slice) {
                continue;
            }
            let match_paths = action.paths.as_slice();
            if !rule.matches_after_paths(runtime, match_paths) {
                continue;
            }
            let argv_source_paths = if rule.match_config.needs_argv_source_match() {
                let Some(paths) = rule
                    .match_config
                    .matching_argv_source_paths(runtime, command_token_slice)
                else {
                    continue;
                };
                Some(paths)
            } else {
                None
            };
            let decision_paths = if rule.needs_decision_paths() {
                if let Some(paths) = argv_source_paths.as_deref()
                    && !rule.match_config.needs_path_match()
                {
                    paths
                } else {
                    action.paths.as_slice()
                }
            } else {
                action.paths.as_slice()
            };
            if let Some(materializer) = rule.decision_materializer {
                let decision = match materializer {
                    agent_semantic_config::HookClientDecisionMaterializer::AgentSearchJson => {
                        command_token_slice.and_then(|tokens| {
                            crate::classifier::materialize_agent_search_json_decision(
                                runtime, platform, event, action, tokens,
                            )
                        })
                    }
                    agent_semantic_config::HookClientDecisionMaterializer::ApplyPatch => {
                        crate::classifier::materialize_apply_patch_decision(
                            runtime,
                            platform,
                            event,
                            action,
                            self.semantic_ast_patch_enabled(),
                        )
                    }
                    agent_semantic_config::HookClientDecisionMaterializer::SourceAccess => {
                        let agent_action =
                            rule.match_config.agent_action.derive_agent_action_for_rule(
                                runtime,
                                action,
                                Some(action.paths.as_slice()),
                            );
                        crate::classifier::materialize_source_access_decision(
                            runtime,
                            platform,
                            event,
                            action,
                            agent_action.as_ref(),
                            command_token_slice,
                            self.semantic_ast_patch_enabled(),
                            self.recovery_prompt(),
                        )
                    }
                };
                if let Some(mut decision) = decision {
                    if let Some(agent_action) = rule.agent_action_receipt(
                        runtime,
                        action,
                        decision.subject.paths.as_slice(),
                    ) {
                        decision
                            .fields
                            .insert("agentAction".to_string(), agent_action);
                    }
                    decision.fields.insert(
                        "configRuleId".to_string(),
                        serde_json::Value::String(rule.id.clone()),
                    );
                    if let Some(intent) = rule.intent.as_ref() {
                        decision.fields.insert(
                            "intent".to_string(),
                            serde_json::Value::String(intent.clone()),
                        );
                    }
                    decision
                        .fields
                        .extend(rule.fields.iter().map(|(key, value)| {
                            (key.clone(), serde_json::Value::String(value.clone()))
                        }));
                    return Some(decision);
                }
                continue;
            }
            return Some(rule.decision(runtime, platform, event, action, decision_paths));
        }
        None
    }
}

pub(in crate::hook_config) fn compile_config(
    config: HookClientConfigFile,
) -> Result<ClientHookConfig, String> {
    let contract_fingerprint = config.contract_fingerprint.clone();
    let default_config = agent_semantic_config::default_hook_client_config_file()?;
    let default_agent_session_messages = default_config.agent_session_messages;
    let agent_session_messages = merge_agent_session_messages(
        config.agent_session_messages,
        default_agent_session_messages,
    );
    let semantic_ast_patch_enabled = config
        .experimental
        .get("semanticAstPatch")
        .and_then(|feature| feature.get("enabled"))
        .copied()
        .unwrap_or(true);
    let configured_rule_ids = config
        .rules
        .iter()
        .map(|rule| rule.id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let mut rule_configs = default_config
        .rules
        .into_iter()
        .filter(|rule| !configured_rule_ids.contains(rule.id.as_str()))
        .collect::<Vec<_>>();
    rule_configs.extend(config.rules);
    let agents = config.agents;
    let mut rules = rule_configs
        .into_iter()
        .filter(|rule| rule.enabled)
        .map(|rule| CompiledHookRule::try_from_with_agents(rule, &agents.resident_agents))
        .collect::<Result<Vec<_>, _>>()?;
    // `sort_by_key` is stable, so equal-priority rules keep config file order.
    rules.sort_by_key(|rule| std::cmp::Reverse(rule.priority));
    Ok(ClientHookConfig {
        rules,
        contract_fingerprint,
        semantic_ast_patch_disabled: !semantic_ast_patch_enabled,
        agent_org_artifacts: compile_agent_org_artifacts_config(config.agent_org_artifacts)?,
        recovery_prompt: config.recovery_prompt.into(),
        agent_session_messages,
        asp_session_policy: AspSessionPolicy::try_from(agents)?,
    })
}
