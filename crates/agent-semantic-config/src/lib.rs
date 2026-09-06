#![deny(dead_code)]
// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only


//! Unified project identity, configuration, and local state layout for ASP.

pub mod agent_route_registry;
mod codex_collaboration;
mod codex_plugin_config;
mod codex_plugin_payload;
mod codex_threads;
pub mod embedded_agent_assets;
mod hook_client_config;
pub mod runtime_dev;

pub use codex_collaboration::CODEX_COLLABORATION_NAMESPACE;
pub use codex_collaboration::CODEX_COLLABORATION_TOOL_CALL_SCHEMA_ID;
pub use codex_collaboration::CODEX_COLLABORATION_TOOL_CALL_SCHEMA_VERSION;
pub use codex_collaboration::CodexCollaborationOperation;
pub use codex_collaboration::CodexCollaborationToolCall;
pub use codex_collaboration::CodexMultiAgentV2Interface;
pub use codex_collaboration::CollaborationAgentStatus;
pub use codex_collaboration::CollaborationAgentType;
pub use codex_collaboration::CollaborationDispatchAction;
pub use codex_collaboration::CollaborationDispatchState;
pub use codex_collaboration::CollaborationHostResultKind;
pub use codex_collaboration::CollaborationInterruptResult;
pub use codex_collaboration::CollaborationLifecycleTool;
pub use codex_collaboration::CollaborationLiveAgent;
pub use codex_collaboration::CollaborationLiveAgents;
pub use codex_collaboration::CollaborationRegistrationState;
pub use codex_collaboration::CollaborationSpawnResult;
pub use codex_collaboration::CollaborationWaitResult;
pub use codex_collaboration::ListAgentsInput;
pub use codex_collaboration::MessageAgentInput;
pub use codex_collaboration::SpawnAgentInput;
pub use codex_collaboration::TargetAgentInput;
pub use codex_collaboration::WaitAgentInput;
pub use codex_collaboration::state_after_interrupt;
pub use codex_plugin_config::codex_config_plugin_enabled;
pub use codex_plugin_payload::CODEX_PLUGIN_HOOKS_RELATIVE_PATH;
pub use codex_plugin_payload::CODEX_PLUGIN_LAUNCHER_RELATIVE_PATH;
pub use codex_plugin_payload::CODEX_PLUGIN_MANIFEST_RELATIVE_PATH;
pub use codex_plugin_payload::CodexPluginPayloadIdentity;
pub use codex_plugin_payload::CodexPluginPayloadInspection;
pub use codex_plugin_payload::CodexPluginPayloadState;
pub use codex_plugin_payload::codex_plugin_cache_root;
pub use codex_plugin_payload::inspect_codex_plugin_payload;
pub use codex_plugin_payload::load_codex_plugin_payload_identity;
pub use codex_threads::CODEX_THREAD_NAMESPACE;
pub use codex_threads::CODEX_THREAD_REFERENCE_SCHEMA_ID;
pub use codex_threads::CODEX_THREAD_REFERENCE_SCHEMA_VERSION;
pub use codex_threads::CODEX_THREAD_TOOL_CALL_SCHEMA_ID;
pub use codex_threads::CODEX_THREAD_TOOL_CALL_SCHEMA_VERSION;
pub use codex_threads::CodexHostId;
pub use codex_threads::CodexThreadId;
pub use codex_threads::CodexThreadOperation;
pub use codex_threads::CodexThreadReference;
pub use codex_threads::CodexThreadToolCall;
pub use codex_threads::IncludeThreadOutputs;
pub use codex_threads::ReadThreadInput;
pub use codex_threads::SendMessageToThreadInput;
pub use codex_threads::ThreadTargetInput;
pub use codex_threads::WaitThreadTarget;
pub use codex_threads::WaitThreadsInput;

pub use hook_client_config::hook_client_contract_fingerprint;

pub use hook_client_config::HookClientStructuredFormat;
mod semantic_identity;

pub use semantic_identity::LanguageId;
pub use semantic_identity::ProviderId;

pub use hook_client_config::AspProjectConfigFile;
pub use hook_client_config::AspProjectDiscoveryConfig;
pub use hook_client_config::AspProjectHookConfig;
pub use hook_client_config::CLIENT_HOOK_CONFIG_SCHEMA_ID;
pub use hook_client_config::CLIENT_HOOK_CONFIG_SCHEMA_VERSION;
pub use hook_client_config::HookClientActionKind;
pub use hook_client_config::HookClientActionSubjectKind;
pub use hook_client_config::HookClientAgentCallingConfig;
pub use hook_client_config::HookClientAgentOrgArtifactsArchiveWarningConfig;
pub use hook_client_config::HookClientAgentOrgArtifactsConfig;
pub use hook_client_config::HookClientAgentSelector;
pub use hook_client_config::HookClientCapabilityPolicyConfig;
pub use hook_client_config::HookClientCommandActionPatternConfig;
pub use hook_client_config::HookClientCommandCategory;
pub use hook_client_config::HookClientCommandProfileConfig;
pub use hook_client_config::HookClientCommandProfileId;
pub use hook_client_config::HookClientCommandProfileRef;
pub use hook_client_config::HookClientCommandSetConfig;
pub use hook_client_config::HookClientConfigDecision;
pub use hook_client_config::HookClientConfigFile;
pub use hook_client_config::HookClientConfigReasonKind;
pub use hook_client_config::HookClientConfigRouteKind;
pub use hook_client_config::HookClientConfigStdinMode;
pub use hook_client_config::HookClientHostInvocationKind;
pub use hook_client_config::HookClientLazyProviderPolicy;
pub use hook_client_config::HookClientMatcherPolicy;
pub use hook_client_config::HookClientProfileConfig;
pub use hook_client_config::HookClientProviderRouteIdentity;
pub use hook_client_config::HookClientRecoveryPromptConfig;
pub use hook_client_config::HookClientRuleConfig;
pub use hook_client_config::HookClientRuleDispatchConfig;
pub use hook_client_config::HookClientRuleDispatchTransport;
pub use hook_client_config::HookClientRuleMatchConfig;
pub use hook_client_config::HookClientRuleRouteConfig;
pub use hook_client_config::HookClientStructuredFilterGrammar;
pub use hook_client_config::HookClientStructuredProjectionMatchConfig;
pub use hook_client_config::HookPolicyCoverageCase;
pub use hook_client_config::HookPolicyCoverageLanguageId;
pub use hook_client_config::HookPolicyCoveragePath;
pub use hook_client_config::HookPolicyCoveragePolarity;
pub use hook_client_config::HookPolicyCoverageProviderId;
pub use hook_client_config::HookPolicyCoverageRuleId;
pub use hook_client_config::HookPolicyCoverageSettings;
pub use hook_client_config::HookPolicyCoverageSurface;
pub use hook_client_config::WrapperMatchMode;
pub use hook_client_config::default_hook_client_config_file;
pub use hook_client_config::default_hook_client_config_template;
pub use hook_client_config::derive_hook_policy_coverage_cases;
pub use hook_client_config::expand_command_profile_prefixes;
pub use hook_client_config::expand_command_set_prefixes;
pub use hook_client_config::load_asp_project_config_file;
pub use hook_client_config::load_hook_client_config_declared_contract_fingerprint;
pub use hook_client_config::load_hook_client_config_file;
pub use hook_client_config::load_hook_client_config_overlay_file;
pub use hook_client_config::merge_asp_project_hook_config;
pub use hook_client_config::mutate_path_outside_registered_extensions;
pub use hook_client_config::render_hook_client_message_template;
pub use hook_client_config::validate_codex_host_matcher_expression;
pub mod source_extension;
