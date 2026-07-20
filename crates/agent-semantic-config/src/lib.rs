#![deny(dead_code)]

//! Unified project identity, configuration, and local state layout for ASP.

pub mod codex_agent_projection;
mod codex_plugin_config;
mod hook_client_config;

pub use codex_plugin_config::codex_config_plugin_enabled;

pub use hook_client_config::HookClientDecisionMaterializer;
pub use hook_client_config::hook_client_contract_fingerprint;
mod layout;

pub use hook_client_config::{
    AspCommandIntent, AspCommandIntentMatch, AspCommandRouteId, AspProjectConfigFile,
    AspProjectHookConfig, CLIENT_HOOK_CONFIG_SCHEMA_ID, CLIENT_HOOK_CONFIG_SCHEMA_VERSION,
    HookClientAgentOrgArtifactsArchiveWarningConfig, HookClientAgentOrgArtifactsConfig,
    HookClientAgentSessionGuideConfig, HookClientAgentSessionMessagesConfig,
    HookClientAgentsConfig, HookClientAspCommandIntentPolicyConfig,
    HookClientAspControlPlaneIntentConfig, HookClientAspExactEvidenceIntentConfig,
    HookClientAspInvalidEvidenceIntentConfig, HookClientAspReasoningIntentConfig,
    HookClientConfigDecision, HookClientConfigFile, HookClientConfigReasonKind,
    HookClientConfigRouteKind, HookClientConfigStdinMode, HookClientExecutionLaneConfig,
    HookClientExecutionLanesConfig, HookClientExecutionTransport, HookClientRecoveryPromptConfig,
    HookClientResidentAgentConfig, HookClientRuleConfig, HookClientRuleMatchConfig,
    HookClientRuleRouteConfig, StructuralSelector, classify_asp_language_command,
    default_hook_client_config_file, default_hook_client_config_template,
    default_hook_client_config_template_for_source_extensions, load_asp_project_config_file,
    load_hook_client_config_file, render_hook_client_message_template,
};
pub use layout::{
    PRJ_CACHE_HOME_ENV, ProjectCacheSource, ProjectEnvStatus, ProjectRuntimeEnv,
    ProjectRuntimeLayout, project_activation_path, project_cache_root, project_cache_root_with_env,
    project_client_cache_dir, project_hook_cache_dir, project_protocol_home,
    project_provider_lock_dir, project_runtime_bin_dir, project_runtime_layout,
    project_runtime_layout_with_env,
};
