#![deny(dead_code)]

//! Unified project identity, configuration, and local state layout for ASP.

pub mod agent_route_registry;
mod codex_collaboration;
mod codex_plugin_config;
mod codex_plugin_payload;
mod codex_threads;
pub mod embedded_agent_assets;
mod hook_client_config;
pub mod runtime_dev;

pub use codex_collaboration::{
    CODEX_COLLABORATION_NAMESPACE, CODEX_COLLABORATION_TOOL_CALL_SCHEMA_ID,
    CODEX_COLLABORATION_TOOL_CALL_SCHEMA_VERSION, CodexCollaborationOperation,
    CodexCollaborationToolCall, CodexMultiAgentV2Interface, CollaborationAgentStatus,
    CollaborationDispatchAction, CollaborationDispatchState, CollaborationHostResultKind,
    CollaborationInterruptResult, CollaborationLifecycleTool, CollaborationLiveAgent,
    CollaborationLiveAgents, CollaborationRegistrationState, CollaborationSpawnResult,
    CollaborationWaitResult, ListAgentsInput, MessageAgentInput, SpawnAgentInput, TargetAgentInput,
    WaitAgentInput, state_after_interrupt,
};
pub use codex_plugin_config::codex_config_plugin_enabled;
pub use codex_plugin_payload::{
    CODEX_PLUGIN_HOOKS_RELATIVE_PATH, CODEX_PLUGIN_LAUNCHER_RELATIVE_PATH,
    CODEX_PLUGIN_MANIFEST_RELATIVE_PATH, CodexPluginPayloadIdentity, CodexPluginPayloadInspection,
    CodexPluginPayloadState, codex_plugin_cache_root, inspect_codex_plugin_payload,
    load_codex_plugin_payload_identity,
};
pub use codex_threads::{
    CODEX_THREAD_NAMESPACE, CODEX_THREAD_REFERENCE_SCHEMA_ID,
    CODEX_THREAD_REFERENCE_SCHEMA_VERSION, CODEX_THREAD_TOOL_CALL_SCHEMA_ID,
    CODEX_THREAD_TOOL_CALL_SCHEMA_VERSION, CodexThreadOperation, CodexThreadReference,
    CodexThreadToolCall, ReadThreadInput, SendMessageToThreadInput, ThreadTargetInput,
    WaitThreadTarget, WaitThreadsInput,
};

pub use hook_client_config::hook_client_contract_fingerprint;

pub use hook_client_config::HookClientStructuredFormat;
mod semantic_identity;

pub use semantic_identity::{LanguageId, ProviderId};

pub use hook_client_config::{
    AspProjectConfigFile, AspProjectDiscoveryConfig, AspProjectHookConfig,
    CLIENT_HOOK_CONFIG_SCHEMA_ID, CLIENT_HOOK_CONFIG_SCHEMA_VERSION, HookClientActionKind,
    HookClientActionSubjectKind, HookClientAgentCallingConfig,
    HookClientAgentOrgArtifactsArchiveWarningConfig, HookClientAgentOrgArtifactsConfig,
    HookClientAgentSelector, HookClientCapabilityPolicyConfig, HookClientCommandProfileConfig,
    HookClientCommandProfileRef, HookClientCommandSetConfig, HookClientConfigDecision,
    HookClientConfigFile, HookClientConfigReasonKind, HookClientConfigRouteKind,
    HookClientConfigStdinMode, HookClientHostInvocationKind, HookClientLazyProviderPolicy,
    HookClientMatcherPolicy, HookClientProfileConfig, HookClientProviderRouteIdentity,
    HookClientRecoveryPromptConfig, HookClientRuleConfig, HookClientRuleDispatchConfig,
    HookClientRuleDispatchTransport, HookClientRuleMatchConfig, HookClientRuleRouteConfig,
    HookPolicyCoverageCase, HookPolicyCoveragePolarity, HookPolicyCoverageSettings,
    HookPolicyCoverageSurface, WrapperMatchMode, default_hook_client_config_file,
    default_hook_client_config_template, derive_hook_policy_coverage_cases,
    expand_command_profile_prefixes, expand_command_set_prefixes, load_asp_project_config_file,
    load_hook_client_config_declared_contract_fingerprint, load_hook_client_config_file,
    load_hook_client_config_overlay_file, merge_asp_project_hook_config,
    mutate_path_outside_registered_extensions, render_hook_client_message_template,
    validate_codex_host_matcher_expression,
};
pub use hook_client_config::{
    HookClientStructuredFilterGrammar, HookClientStructuredProjectionMatchConfig,
};
pub mod source_extension;
