#![deny(dead_code)]

//! Unified project identity, configuration, and local state layout for ASP.

pub mod codex_agent_projection;
mod codex_plugin_config;
mod hook_client_config;

pub use codex_plugin_config::codex_config_plugin_enabled;

pub use hook_client_config::HookClientDecisionMaterializer;
pub use hook_client_config::hook_client_contract_fingerprint;
mod layout;

pub use hook_client_config::HookClientStructuredFormat;
pub use hook_client_config::{
    AgentActionAuthorityRule, AgentActionEffectRule, AspProjectConfigFile, AspProjectHookConfig,
    CLIENT_HOOK_CONFIG_SCHEMA_ID, CLIENT_HOOK_CONFIG_SCHEMA_VERSION, HookClientActionAuthority,
    HookClientActionKind, HookClientActionSubjectKind,
    HookClientAgentOrgArtifactsArchiveWarningConfig, HookClientAgentOrgArtifactsConfig,
    HookClientAgentSessionGuideConfig, HookClientAgentSessionMessagesConfig,
    HookClientAgentsConfig, HookClientCommandWrapper, HookClientConfigDecision,
    HookClientConfigFile, HookClientConfigReasonKind, HookClientConfigRouteKind,
    HookClientConfigStdinMode, HookClientFlagPresence, HookClientInvocationShape,
    HookClientLazyProviderPolicy, HookClientRecoveryPromptConfig, HookClientResidentAgentConfig,
    HookClientRuleConfig, HookClientRuleDispatchConfig, HookClientRuleDispatchTransport,
    HookClientRuleMatchConfig, HookClientRuleRouteConfig, HookClientWrapperMatch,
    default_hook_client_config_file, default_hook_client_config_template,
    load_asp_project_config_file, load_hook_client_config_file, merge_asp_project_hook_config,
    render_hook_client_message_template,
};
pub use hook_client_config::{
    HookClientStructuredFilterGrammar, HookClientStructuredProjectionMatchConfig,
};
pub use layout::{
    PRJ_CACHE_HOME_ENV, ProjectCacheSource, ProjectEnvStatus, ProjectRuntimeEnv,
    ProjectRuntimeLayout, project_activation_path, project_cache_root, project_cache_root_with_env,
    project_client_cache_dir, project_hook_cache_dir, project_protocol_home,
    project_provider_lock_dir, project_runtime_bin_dir, project_runtime_layout,
    project_runtime_layout_with_env,
};
