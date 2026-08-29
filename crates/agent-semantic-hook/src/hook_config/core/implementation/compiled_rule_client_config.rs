//! Implements runtime accessors and classification for compiled hook config.

use std::{borrow::Cow, path::Path};

use super::{
    AgentOrgArtifactsArchiveWarning, AgentOrgArtifactsRecovery, ClientHookConfig, CompiledHookRule,
    DURABLE_HOOK_MATCHER_SCHEMA_ID, DURABLE_HOOK_MATCHER_SCHEMA_VERSION, DurableHookConfigArtifact,
    HookClientConfigFile, HookRuntime, ToolAction, compile_agent_org_artifacts_config,
};
use crate::hook_config::core::implementation::profile_provider_projection::{
    extend_profile_provider_projections, extend_registered_provider_route_projections,
};

impl Default for ClientHookConfig {
    fn default() -> Self {
        let config = crate::hook_config::core_load::default_client_config_file()
            .expect("embedded hook client config must remain valid");
        compile_config(config).expect("embedded hook client rules must compile")
    }
}

impl ClientHookConfig {
    fn classification_runtime(&self, runtime: &HookRuntime) -> HookRuntime {
        let mut classification_runtime = runtime.clone();
        let mut provider_projections = self.provider_projections.clone();
        crate::hook_config::core::implementation::profile_provider_projection::merge_provider_projection_facts(
            &mut provider_projections,
            &classification_runtime.policy_providers,
        );
        classification_runtime.policy_providers = provider_projections;
        classification_runtime
    }

    pub(crate) fn observed_action_receipt(
        &self,
        runtime: &HookRuntime,
        action: &ToolAction,
    ) -> (serde_json::Value, Vec<agent_semantic_config::LanguageId>) {
        let classification_runtime = self.classification_runtime(runtime);
        let agent_action = crate::action_ir::project_agent_action(
            &classification_runtime,
            action,
            Some(action.paths.as_slice()),
            None,
        );
        let mut language_ids = crate::collect_source_selector_matches(
            &classification_runtime,
            action.paths.iter().map(String::as_str),
            |_| true,
        )
        .into_iter()
        .map(|matched| matched.provider.language_id)
        .collect::<Vec<_>>();
        language_ids.sort();
        language_ids.dedup();
        (agent_action.receipt_value(), language_ids)
    }

    /// Compile a validated source configuration for the reference side of the
    /// config-derived black-box coverage framework.
    pub fn compile_policy_coverage_reference(config: HookClientConfigFile) -> Result<Self, String> {
        compile_config(config)
    }

    /// Validate configured language profiles against the active runtime identity.
    pub fn validate_language_provider_projection(
        &self,
        runtime: &HookRuntime,
    ) -> Result<(), String> {
        for provider in &runtime.providers {
            let projected = self
                .profiles
                .values()
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
            let _ = projected;
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
        let mut provider_projections = self.provider_projections.clone();
        crate::hook_config::core::implementation::profile_provider_projection::merge_provider_projection_facts(
            &mut provider_projections,
            &runtime.policy_providers,
        );
        runtime.policy_providers = provider_projections;
        for provider in &mut runtime.providers {
            let projected = self
                .profiles
                .values()
                .find(|candidate| {
                    candidate.language_id == provider.language_id.as_str()
                        && candidate.provider_id == provider.provider_id.as_str()
                })
                .expect("validated language provider projection");
            provider.source_extensions = projected
                .extension_any
                .iter()
                .map(|extension| format!(".{extension}"))
                .collect();
        }
        Ok(())
    }

    /// Move the immutable provider projection into a one-shot matcher runtime
    /// without cloning its route and extension vectors.
    pub fn move_language_provider_projection(
        &mut self,
        runtime: &mut HookRuntime,
    ) -> Result<(), String> {
        self.publish_policy_snapshot(runtime)?;
        runtime.policy_providers = std::mem::take(&mut self.provider_projections);
        for provider in &mut runtime.providers {
            let projected = self
                .profiles
                .values()
                .find(|candidate| {
                    candidate.language_id == provider.language_id.as_str()
                        && candidate.provider_id == provider.provider_id.as_str()
                })
                .expect("validated language provider projection");
            provider.source_extensions = projected
                .extension_any
                .iter()
                .map(|extension| format!(".{extension}"))
                .collect();
        }
        Ok(())
    }

    pub(crate) fn publish_policy_snapshot(&self, runtime: &HookRuntime) -> Result<String, String> {
        self.validate_language_provider_projection(runtime)?;
        let generation_digest = self.policy_generation_digest.clone();
        let _ = self.policy_receipt.set(super::HookPolicyReceipt {
            generation_digest: generation_digest.clone(),
            kernel_version: crate::protocol::HOOK_POLICY_KERNEL_VERSION,
        });
        Ok(generation_digest)
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

    pub(crate) fn attach_hook_policy_receipt(&self, decision: &mut crate::HookDecision) {
        let (generation_digest, kernel_version) = self.hook_policy_receipt().unwrap_or((
            self.policy_generation_digest.as_str(),
            crate::protocol::HOOK_POLICY_KERNEL_VERSION,
        ));
        decision.fields.insert(
            "hookPolicySnapshotDigest".to_owned(),
            serde_json::Value::String(generation_digest.to_owned()),
        );
        decision.fields.insert(
            "hookPolicyKernelVersion".to_owned(),
            serde_json::Value::String(kernel_version.to_owned()),
        );
        decision.fields.insert(
            "hookPolicySynchronousDependencies".to_owned(),
            serde_json::Value::Array(Vec::new()),
        );
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
        let classification_runtime = self.classification_runtime(runtime);
        let runtime = &classification_runtime;
        let mut command_tokens: Option<Option<Cow<'_, [String]>>> = None;
        for rule_index in self.candidate_rule_indices(platform, event) {
            let rule = &self.rules[*rule_index];
            let needs_command_tokens = rule.match_config.needs_command_tokens();
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
            let candidate = crate::hook_config::HookPolicyCandidate {
                priority: rule.priority,
                terminal: rule.terminal,
                decision: rule.decision(
                    runtime,
                    platform,
                    event,
                    action,
                    decision_paths,
                    structured_source_operands.as_deref(),
                ),
            };
            return Some(candidate);
        }
        None
    }
}

pub(in crate::hook_config) fn compile_config(
    config: HookClientConfigFile,
) -> Result<ClientHookConfig, String> {
    compile_config_with_executable_capabilities(config, None)
}

pub(in crate::hook_config) fn compile_config_with_executable_capabilities(
    mut config: HookClientConfigFile,
    executable_capabilities: Option<&std::collections::BTreeSet<String>>,
) -> Result<ClientHookConfig, String> {
    let default_config = agent_semantic_config::default_hook_client_config_file()?;
    for (profile_id, profile) in &default_config.profiles {
        config
            .profiles
            .entry(profile_id.clone())
            .or_insert_with(|| profile.clone());
    }
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
    let configured_command_set_ids = config
        .command_sets
        .iter()
        .map(|command_set| command_set.id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let mut command_sets = default_config
        .command_sets
        .into_iter()
        .filter(|command_set| !configured_command_set_ids.contains(command_set.id.as_str()))
        .collect::<Vec<_>>();
    command_sets.extend(config.command_sets);
    config.command_sets = command_sets;
    let configured_capability_policy_ids = config
        .capability_policies
        .iter()
        .map(|policy| policy.id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let mut capability_policies = default_config
        .capability_policies
        .into_iter()
        .filter(|policy| !configured_capability_policy_ids.contains(policy.id.as_str()))
        .collect::<Vec<_>>();
    capability_policies.extend(config.capability_policies);
    config.capability_policies = capability_policies;
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
    config.materialize_profile_rule_ir()?;
    compile_resolved_config(config, None, None, executable_capabilities)
}

fn compile_resolved_config(
    config: HookClientConfigFile,
    mut durable_matchers: Option<
        std::collections::BTreeMap<String, super::DurableRuleMatcherArtifact>,
    >,
    durable_policy: Option<(
        String,
        Vec<crate::protocol_activation::protocol_activation_manifest::HookProviderProjection>,
    )>,
    executable_capabilities: Option<&std::collections::BTreeSet<String>>,
) -> Result<ClientHookConfig, String> {
    let source_config = durable_matchers.is_none().then(|| config.clone());
    let contract_fingerprint = config.contract_fingerprint.clone();
    let profiles = config.profiles.clone();
    let agent_calling = config.agent_calling.clone();
    let wrapper_match = agent_semantic_config::WrapperMatchMode::Off;
    let mut rules = config
        .rules
        .into_iter()
        .filter(|rule| rule.enabled)
        .map(|rule| {
            let durable_matcher = durable_matchers
                .as_mut()
                .map(|matchers| {
                    matchers.remove(&rule.id).ok_or_else(|| {
                        format!(
                            "durable Hook matcher artifact omitted enabled rule `{}`",
                            rule.id
                        )
                    })
                })
                .transpose()?;
            CompiledHookRule::try_from_with_policy_and_matcher(
                rule,
                &config.command_profiles,
                &config.command_sets,
                &config.capability_policies,
                wrapper_match,
                &agent_calling,
                durable_matcher,
                executable_capabilities,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    if durable_matchers
        .as_ref()
        .is_some_and(|matchers| !matchers.is_empty())
    {
        return Err("durable Hook matcher artifact rule inventory mismatch".to_owned());
    }
    // `sort_by_key` is stable, so equal-priority rules keep config file order.
    rules.sort_by_key(|rule| std::cmp::Reverse(rule.priority));
    let rule_candidates = compile_rule_candidate_index(&rules);
    let (mut policy_generation_digest, mut provider_projections) =
        durable_policy.unwrap_or_else(|| ("profile-native-v1".to_owned(), Vec::new()));
    extend_profile_provider_projections(&config.profiles, &mut provider_projections);
    extend_registered_provider_route_projections(
        &config.provider_routes,
        &mut provider_projections,
    );
    let canonical_profiles = serde_json::to_vec(&(&config.profiles, &config.provider_routes))
        .map_err(|error| format!("serialize Hook provider policy projection: {error}"))?;
    let profile_aware_digest =
        agent_semantic_content_identity::exact_selector_merkle::canonical_content_digest(
            b"agent.semantic-protocols.hook-policy-profile-overlay.v1",
            &[
                policy_generation_digest.as_bytes(),
                canonical_profiles.as_slice(),
            ],
        );
    policy_generation_digest = format!("blake3-256:{}", profile_aware_digest.as_str());
    Ok(ClientHookConfig {
        source_config,
        rules,
        rule_candidates,
        policy_receipt: std::sync::OnceLock::new(),
        profiles,
        policy_generation_digest,
        provider_projections,
        contract_fingerprint,
        agent_org_artifacts: compile_agent_org_artifacts_config(config.agent_org_artifacts)?,
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
        let config = self
            .source_config
            .clone()
            .expect("only a source-compiled Hook config may publish a durable snapshot");
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
            policy_generation_digest: self.policy_generation_digest.clone(),
            provider_projections: self.provider_projections.clone(),
        }
    }

    /// Build policy-derived direct-read decision templates keyed by extension.
    /// The template is produced by the complete matcher; the hot loader only
    /// substitutes the normalized source path into that already-admitted
    /// decision and never re-implements rule selection.
    pub fn durable_direct_read_decision_shards(
        &self,
    ) -> Result<Vec<(String, String, Vec<u8>)>, String> {
        let source = self
            .source_config
            .as_ref()
            .ok_or_else(|| "only a source-compiled Hook config may publish shards".to_owned())?;
        let extensions = source
            .profiles
            .values()
            .flat_map(|profile| {
                profile
                    .extension_any
                    .iter()
                    .map(|extension| format!(".{extension}"))
            })
            .collect::<std::collections::BTreeSet<_>>();
        let target_paths = extensions
            .iter()
            .flat_map(|extension| {
                let registered_path = format!("__ASP_DIRECT_READ_PATH__{extension}");
                let negative_path =
                    agent_semantic_config::mutate_path_outside_registered_extensions(
                        &registered_path,
                        &extensions,
                    );
                [registered_path, negative_path]
            })
            .filter_map(|path| {
                let dot = path.rfind('.')?;
                Some((path[dot..].to_ascii_lowercase(), path))
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        let runtime = HookRuntime {
            project_root: ".".to_owned(),
            rankers: Vec::new(),
            providers: Vec::new(),
            policy_providers: self.provider_projections.clone(),
        };
        let mut shards = Vec::with_capacity(target_paths.len());
        for (extension, path) in target_paths {
            let action =
                crate::tool_action::ToolAction::normalized_direct_policy_action(path.clone());
            let mut decision = self
                .classify_candidate(&runtime, "codex", "pre-tool", &action)
                .map(|candidate| candidate.decision)
                .unwrap_or_else(|| {
                    crate::classifier::default_allow_for_normalized_action(
                        "codex", "pre-tool", &action,
                    )
                });
            self.attach_hook_policy_receipt(&mut decision);
            shards.push((extension, path, decision.to_compact_binary()?));
        }
        Ok(shards)
    }

    /// Compile wrapped registered-source winner tables from the complete
    /// configured policy graph.
    pub fn durable_shell_read_decision_shards(
        &self,
    ) -> Result<Vec<(String, String, Vec<u8>)>, String> {
        let source = self
            .source_config
            .as_ref()
            .ok_or_else(|| "only a source-compiled Hook config may publish shards".to_owned())?;
        // The shard proves that opaque shell stages carrying a registered source
        // operand are governed without pretending the executable implies Read.
        let prefixes =
            std::collections::BTreeSet::from([vec!["opaque-source-consumer".to_owned()]]);
        let extensions = source
            .profiles
            .values()
            .flat_map(|profile| {
                profile
                    .extension_any
                    .iter()
                    .map(|extension| format!(".{extension}"))
            })
            .collect::<std::collections::BTreeSet<_>>();
        let target_paths = extensions
            .iter()
            .flat_map(|extension| {
                let registered_path = format!("__ASP_SHELL_READ_PATH__{extension}");
                let negative_path =
                    agent_semantic_config::mutate_path_outside_registered_extensions(
                        &registered_path,
                        &extensions,
                    );
                [registered_path, negative_path]
            })
            .filter_map(|path| {
                let dot = path.rfind('.')?;
                Some((path[dot..].to_ascii_lowercase(), path))
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        let runtime = HookRuntime {
            project_root: ".".to_owned(),
            rankers: Vec::new(),
            providers: Vec::new(),
            policy_providers: self.provider_projections.clone(),
        };
        target_paths
            .into_iter()
            .map(|(extension, path)| {
                let entries = prefixes
                    .iter()
                    .map(|prefix| {
                        let command = format!("{} {path}", prefix.join(" "));
                        let action = crate::tool_action::ToolAction::normalized_shell_policy_action(
                            command,
                            path.clone(),
                        );
                        let mut decision = self
                            .classify_candidate(&runtime, "codex", "pre-tool", &action)
                            .map(|candidate| candidate.decision)
                            .unwrap_or_else(|| {
                                crate::classifier::default_allow_for_normalized_action(
                                    "codex", "pre-tool", &action,
                                )
                            });
                        self.attach_hook_policy_receipt(&mut decision);
                        (prefix.clone(), decision)
                    })
                    .collect::<Vec<_>>();
                let environment_entries =
                    self.durable_process_environment_decisions(&runtime, Some(&path))?;
                let bytes = crate::CommandDecisionShard::new_with_process_environment_assignments(
                    entries,
                    environment_entries,
                )?
                .to_binary_bytes()?;
                Ok((extension, path, bytes))
            })
            .collect()
    }

    /// Compile configured structured-document grammars and both of their
    /// complete-policy outcomes into a one-shot decision shard. Selection is
    /// still grammar-driven at runtime; rule ordering and messages are fixed
    /// here by the source-compiled policy graph.
    pub fn durable_structured_projection_decision_shard(&self) -> Result<Vec<u8>, String> {
        let runtime = HookRuntime {
            project_root: ".".to_owned(),
            rankers: Vec::new(),
            providers: Vec::new(),
            policy_providers: self.provider_projections.clone(),
        };
        let mut entries = Vec::new();
        for rule in &self.rules {
            let template_path = "__ASP_STRUCTURED_PROJECTION_PATH__.json";
            let template_action = crate::tool_action::ToolAction::normalized_shell_policy_action(
                format!("structured-projector '.field' {template_path}"),
                template_path.to_owned(),
            );
            let Some((projection, mut bounded_decision)) = rule
                .structured_projection_decision_template(&runtime, &template_action, template_path)
            else {
                continue;
            };
            let extension = match projection.document_format {
                agent_semantic_config::HookClientStructuredFormat::Json => "json",
                agent_semantic_config::HookClientStructuredFormat::Toml => "toml",
            };
            let placeholder = format!("__ASP_STRUCTURED_PROJECTION_PATH__.{extension}");
            bounded_decision.replace_template_marker(template_path, &placeholder);
            let rejected_action = crate::tool_action::ToolAction::normalized_shell_policy_action(
                format!("{} '..' {placeholder}", projection.binary),
                placeholder.clone(),
            );
            let mut rejected_decision = self
                .rules
                .iter()
                .filter_map(|rule| {
                    rule.structured_projection_rejection_template(
                        &runtime,
                        &rejected_action,
                        &projection.binary,
                        &placeholder,
                    )
                })
                .max_by_key(|(priority, _)| *priority)
                .map(|(_, decision)| decision)
                .ok_or_else(|| {
                    format!(
                        "structured projector `{}` has no configured rejection policy",
                        projection.binary
                    )
                })?;
            for decision in [&mut bounded_decision, &mut rejected_decision] {
                self.attach_hook_policy_receipt(decision);
            }
            entries.push((projection, placeholder, bounded_decision, rejected_decision));
        }
        crate::StructuredProjectionDecisionShard::new(entries)?.to_binary_bytes()
    }

    /// Compile the finite command-profile prefix space into winning decisions.
    /// The table is derived entirely from config and preserves wrapper matching.
    pub fn durable_command_profile_decision_shard(&self) -> Result<Vec<u8>, String> {
        let prefixes = self.durable_command_decision_prefixes()?;
        let runtime = HookRuntime {
            project_root: ".".to_owned(),
            rankers: Vec::new(),
            providers: Vec::new(),
            policy_providers: self.provider_projections.clone(),
        };
        let entries = prefixes
            .into_iter()
            .map(|prefix| {
                let action = crate::tool_action::ToolAction::normalized_shell_command_action(
                    prefix.join(" "),
                    "Bash".to_owned(),
                );
                let mut decision = self
                    .classify_candidate(&runtime, "codex", "pre-tool", &action)
                    .map(|candidate| candidate.decision)
                    .unwrap_or_else(|| {
                        crate::classifier::default_allow_for_normalized_action(
                            "codex", "pre-tool", &action,
                        )
                    });
                self.attach_hook_policy_receipt(&mut decision);
                (prefix, decision)
            })
            .collect();
        let environment_entries = self.durable_process_environment_decisions(&runtime, None)?;
        crate::CommandDecisionShard::new_with_process_environment_assignments(
            entries,
            environment_entries,
        )?
        .to_binary_bytes()
    }

    fn durable_command_decision_prefixes(
        &self,
    ) -> Result<std::collections::BTreeSet<Vec<String>>, String> {
        let source = self
            .source_config
            .as_ref()
            .ok_or_else(|| "only a source-compiled Hook config may publish shards".to_owned())?;
        let mut prefixes = source
            .command_profiles
            .iter()
            .flat_map(|profile| profile.categories.values())
            .flatten()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>();
        let registered_languages = source
            .profiles
            .values()
            .map(|profile| profile.language_id.clone())
            .collect::<Vec<_>>();
        for pattern in source
            .rules
            .iter()
            .filter(|rule| rule.enabled)
            .flat_map(|rule| {
                rule.match_config
                    .argv_pattern_any
                    .iter()
                    .chain(&rule.match_config.argv_prefix_any)
            })
        {
            if pattern.iter().any(|token| token == "<registered-language>") {
                for language in &registered_languages {
                    prefixes.insert(
                        pattern
                            .iter()
                            .map(|token| {
                                if token == "<registered-language>" {
                                    language.clone()
                                } else {
                                    token.clone()
                                }
                            })
                            .collect(),
                    );
                }
            } else {
                prefixes.insert(pattern.clone());
            }
        }
        Ok(prefixes)
    }

    /// Hydrate the compiled matcher without invoking any regex, glob, or Aho builder.
    pub fn from_durable_snapshot_config(
        artifact: DurableHookConfigArtifact,
    ) -> Result<Self, String> {
        let DurableHookConfigArtifact {
            schema_id,
            schema_version,
            mut config,
            rule_matchers,
            policy_generation_digest,
            provider_projections,
        } = artifact;
        if schema_id != DURABLE_HOOK_MATCHER_SCHEMA_ID
            || schema_version != DURABLE_HOOK_MATCHER_SCHEMA_VERSION
        {
            return Err("durable Hook matcher artifact contract mismatch".to_owned());
        }
        config.materialize_profile_rule_ir()?;
        compile_resolved_config(
            config,
            Some(rule_matchers),
            Some((policy_generation_digest, provider_projections)),
            None,
        )
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/hook_config_durable_artifact.rs"]
mod tests;
