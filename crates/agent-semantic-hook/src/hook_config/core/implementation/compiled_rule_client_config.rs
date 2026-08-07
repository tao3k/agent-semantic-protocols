//! Implements runtime accessors and classification for compiled hook config.

use std::{borrow::Cow, path::Path};

use super::{
    AgentOrgArtifactsArchiveWarning, AgentOrgArtifactsRecovery, AspSessionPolicy, ClientHookConfig,
    CompiledHookRule, CompiledRecoveryPromptConfig, DURABLE_HOOK_MATCHER_SCHEMA_ID,
    DURABLE_HOOK_MATCHER_SCHEMA_VERSION, DurableHookConfigArtifact, HookClientConfigFile,
    HookRuntime, ToolAction, compile_agent_org_artifacts_config, merge_agent_session_messages,
};

impl Default for ClientHookConfig {
    fn default() -> Self {
        let config = crate::hook_config::core_load::default_client_config_file()
            .expect("embedded hook client config must remain valid");
        compile_config(config).expect("embedded hook client rules must compile")
    }
}

impl ClientHookConfig {
    /// Validate the managed language-provider projection against the active
    /// manifest-bound runtime before it can participate in source matching.
    pub fn validate_language_provider_projection(
        &self,
        runtime: &HookRuntime,
    ) -> Result<(), String> {
        for provider in &runtime.providers {
            let projected = self
                .language_providers
                .iter()
                .find(|candidate| {
                    candidate.language_id == provider.language_id.as_str()
                        && candidate.provider_id == provider.provider_id.as_str()
                })
                .ok_or_else(|| {
                    format!(
                        "managed hook config omitted language provider projection `{}/{}`",
                        provider.language_id, provider.provider_id
                    )
                })?;
            if projected.manifest_digest != provider.manifest_digest {
                return Err(format!(
                    "managed hook config provider manifest drift for `{}/{}`: configured {}, active {}",
                    provider.language_id,
                    provider.provider_id,
                    projected.manifest_digest,
                    provider.manifest_digest
                ));
            }
        }
        Ok(())
    }

    /// Apply the validated TOML projection to the matcher runtime. Source
    /// classification below this boundary consumes config, not provider JSON.
    pub fn apply_language_provider_projection(
        &self,
        runtime: &mut HookRuntime,
    ) -> Result<(), String> {
        self.publish_policy_snapshot(runtime)?;
        for provider in &mut runtime.providers {
            let projected = self
                .language_providers
                .iter()
                .find(|candidate| {
                    candidate.language_id == provider.language_id.as_str()
                        && candidate.provider_id == provider.provider_id.as_str()
                })
                .expect("validated language provider projection");
            provider.source_extensions = projected.source_extensions.clone();
        }
        Ok(())
    }

    pub(crate) fn publish_policy_snapshot(&self, runtime: &HookRuntime) -> Result<String, String> {
        self.validate_language_provider_projection(runtime)?;
        let snapshot = crate::hook_policy_kernel::publish_language_provider_snapshot(
            &runtime.project_root,
            &self.language_providers,
        )?;
        let _ = self.policy_receipt.set(super::HookPolicyReceipt {
            generation_digest: snapshot.generation_digest.clone(),
            kernel_version: snapshot.kernel_version,
        });
        Ok(snapshot.generation_digest.clone())
    }

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

    /// Returns the configured rule identifiers in classification order.
    ///
    /// This is intentionally read-only: contract tests and diagnostics can
    /// prove that every production rule has an executable match-policy case
    /// without exposing or duplicating the compiled matcher implementation.
    pub fn rule_ids(&self) -> impl ExactSizeIterator<Item = &str> {
        self.rules.iter().map(|rule| rule.id.as_str())
    }

    /// Returns the number of compiled match-policy rules.
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    pub(crate) fn hook_policy_receipt(&self) -> Option<(&str, &'static str)> {
        self.policy_receipt
            .get()
            .map(|receipt| (receipt.generation_digest.as_str(), receipt.kernel_version))
    }

    fn candidate_rule_indices(&self, platform: &str, event: &str) -> &[usize] {
        let canonical_event = super::canonical_event(event);
        let event_candidates = self
            .rule_candidates
            .get(canonical_event.as_str())
            .or_else(|| self.rule_candidates.get("*"))
            .expect("compiled rule index always contains the wildcard event");
        let canonical_platform = platform.to_ascii_lowercase();
        event_candidates
            .get(canonical_platform.as_str())
            .or_else(|| event_candidates.get("*"))
            .expect("compiled rule index always contains the wildcard platform")
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

    pub(crate) fn classify_candidate(
        &self,
        runtime: &HookRuntime,
        platform: &str,
        event: &str,
        action: &ToolAction,
    ) -> Option<crate::hook_config::HookPolicyCandidate> {
        let mut command_tokens: Option<Option<Cow<'_, [String]>>> = None;
        for rule_index in self.candidate_rule_indices(platform, event) {
            let rule = &self.rules[*rule_index];
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
            let structured_source_operands = match rule
                .match_config
                .structured_projection_source_operands(action)
            {
                Ok(source_operands) => source_operands,
                Err(()) => continue,
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
                                structured_source_operands.as_deref(),
                            );
                        crate::classifier::materialize_source_access_decision(
                            runtime,
                            platform,
                            event,
                            action,
                            agent_action.as_ref(),
                            self.semantic_ast_patch_enabled(),
                            self.recovery_prompt(),
                        )
                    }
                };
                if let Some(mut decision) = decision {
                    if let Some(message) = rule.message.as_ref() {
                        decision.message = message.clone();
                    }
                    if let Some(agent_action) = rule.agent_action_receipt(
                        runtime,
                        action,
                        decision.subject.paths.as_slice(),
                        structured_source_operands.as_deref(),
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
                    return Some(crate::hook_config::HookPolicyCandidate {
                        priority: rule.priority,
                        decision,
                    });
                }
                continue;
            }
            return Some(crate::hook_config::HookPolicyCandidate {
                priority: rule.priority,
                decision: rule.decision(
                    runtime,
                    platform,
                    event,
                    action,
                    decision_paths,
                    structured_source_operands.as_deref(),
                ),
            });
        }
        None
    }
}

fn merge_agents(
    configured: agent_semantic_config::HookClientAgentsConfig,
    defaults: agent_semantic_config::HookClientAgentsConfig,
) -> agent_semantic_config::HookClientAgentsConfig {
    let configured_names = configured
        .resident_agents
        .iter()
        .map(|agent| agent.name.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let mut resident_agents = defaults
        .resident_agents
        .into_iter()
        .filter(|agent| !configured_names.contains(agent.name.as_str()))
        .collect::<Vec<_>>();
    resident_agents.extend(configured.resident_agents);

    let mut placeholders = defaults.placeholders;
    placeholders.extend(configured.placeholders);
    agent_semantic_config::HookClientAgentsConfig {
        placeholders,
        resident_agents,
    }
}

pub(in crate::hook_config) fn compile_config(
    mut config: HookClientConfigFile,
) -> Result<ClientHookConfig, String> {
    let default_config = agent_semantic_config::default_hook_client_config_file()?;
    config.agent_session_messages = merge_agent_session_messages(
        config.agent_session_messages,
        default_config.agent_session_messages,
    );
    let configured_profile_ids = config
        .command_profiles
        .iter()
        .map(|profile| profile.id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let mut command_profiles = default_config
        .command_profiles
        .into_iter()
        .filter(|profile| !configured_profile_ids.contains(profile.id.as_str()))
        .collect::<Vec<_>>();
    command_profiles.extend(config.command_profiles);
    config.command_profiles = command_profiles;
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
    config.rules = rule_configs;
    config.agents = merge_agents(config.agents, default_config.agents);
    compile_resolved_config(config, None)
}

fn compile_resolved_config(
    config: HookClientConfigFile,
    durable_matchers: Option<
        &std::collections::BTreeMap<String, super::DurableRuleMatcherArtifact>,
    >,
) -> Result<ClientHookConfig, String> {
    let source_config = config.clone();
    let contract_fingerprint = config.contract_fingerprint.clone();
    let language_providers = config.language_providers.clone();
    let wrapper_match = config.wrapper_match;
    let agent_session_messages = config.agent_session_messages.clone();
    let semantic_ast_patch_enabled = config
        .experimental
        .get("semanticAstPatch")
        .and_then(|feature| feature.get("enabled"))
        .copied()
        .unwrap_or(true);
    let agents = config.agents.clone();
    let mut rules = config
        .rules
        .into_iter()
        .filter(|rule| rule.enabled)
        .map(|rule| {
            let durable_matcher = durable_matchers
                .map(|matchers| {
                    matchers.get(&rule.id).cloned().ok_or_else(|| {
                        format!(
                            "durable Hook matcher artifact omitted enabled rule `{}`",
                            rule.id
                        )
                    })
                })
                .transpose()?;
            CompiledHookRule::try_from_with_agents_and_matcher(
                rule,
                &agents,
                &config.command_profiles,
                wrapper_match,
                durable_matcher,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    // `sort_by_key` is stable, so equal-priority rules keep config file order.
    rules.sort_by_key(|rule| std::cmp::Reverse(rule.priority));
    let rule_candidates = compile_rule_candidate_index(&rules);
    Ok(ClientHookConfig {
        source_config,
        rules,
        rule_candidates,
        policy_receipt: std::sync::OnceLock::new(),
        language_providers,
        contract_fingerprint,
        semantic_ast_patch_disabled: !semantic_ast_patch_enabled,
        agent_org_artifacts: compile_agent_org_artifacts_config(config.agent_org_artifacts)?,
        recovery_prompt: config.recovery_prompt.into(),
        agent_session_messages,
        asp_session_policy: AspSessionPolicy::try_from(agents)?,
    })
}

fn compile_rule_candidate_index(rules: &[CompiledHookRule]) -> super::RuleCandidateIndex {
    let mut event_keys = std::collections::BTreeSet::from(["*".to_owned()]);
    let mut platform_keys = std::collections::BTreeSet::from(["*".to_owned()]);
    for rule in rules {
        if let Some(event) = rule.canonical_event_key() {
            event_keys.insert(event);
        }
        if let Some(platform) = rule.canonical_platform_key() {
            platform_keys.insert(platform);
        }
    }

    event_keys
        .into_iter()
        .map(|event| {
            let platform_candidates = platform_keys
                .iter()
                .map(|platform| {
                    let indices = rules
                        .iter()
                        .enumerate()
                        .filter_map(|(index, rule)| {
                            let event_matches = match rule.canonical_event_key() {
                                Some(rule_event) => event != "*" && rule_event == event,
                                None => true,
                            };
                            let platform_matches = match rule.canonical_platform_key() {
                                Some(rule_platform) => {
                                    platform != "*" && rule_platform == *platform
                                }
                                None => true,
                            };
                            (event_matches && platform_matches).then_some(index)
                        })
                        .collect();
                    (platform.clone(), indices)
                })
                .collect();
            (event, platform_candidates)
        })
        .collect()
}

impl ClientHookConfig {
    /// Return the normalized config and already-compiled matcher automata as one typed artifact.
    pub fn durable_snapshot_config(&self) -> DurableHookConfigArtifact {
        let config = self.source_config.clone();
        let rule_matchers = self
            .rules
            .iter()
            .map(|rule| (rule.id.clone(), rule.durable_matcher_artifact()))
            .collect();
        DurableHookConfigArtifact {
            schema_id: DURABLE_HOOK_MATCHER_SCHEMA_ID.to_owned(),
            schema_version: DURABLE_HOOK_MATCHER_SCHEMA_VERSION.to_owned(),
            config,
            rule_matchers,
        }
    }

    /// Hydrate the compiled matcher without invoking any regex, glob, or Aho builder.
    pub fn from_durable_snapshot_config(
        artifact: DurableHookConfigArtifact,
    ) -> Result<Self, String> {
        let DurableHookConfigArtifact {
            schema_id,
            schema_version,
            config,
            rule_matchers,
        } = artifact;
        if schema_id != DURABLE_HOOK_MATCHER_SCHEMA_ID
            || schema_version != DURABLE_HOOK_MATCHER_SCHEMA_VERSION
        {
            return Err("durable Hook matcher artifact contract mismatch".to_owned());
        }
        let enabled_rule_ids = config
            .rules
            .iter()
            .filter(|rule| rule.enabled)
            .map(|rule| rule.id.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        if enabled_rule_ids.len() != rule_matchers.len()
            || rule_matchers
                .keys()
                .any(|rule_id| !enabled_rule_ids.contains(rule_id.as_str()))
        {
            return Err("durable Hook matcher artifact rule inventory mismatch".to_owned());
        }
        compile_resolved_config(config, Some(&rule_matchers))
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/hook_config_durable_artifact.rs"]
mod tests;
