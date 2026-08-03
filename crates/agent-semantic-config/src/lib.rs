#![deny(dead_code)]

//! Unified project identity, configuration, and local state layout for ASP.

pub mod codex_agent_projection;
mod codex_plugin_config;
mod hook_client_config;
pub mod runtime_dev;
pub mod subagent_manager;

pub use codex_plugin_config::codex_config_plugin_enabled;

pub use hook_client_config::HookClientDecisionMaterializer;
pub use hook_client_config::hook_client_contract_fingerprint;

pub use hook_client_config::HookClientStructuredFormat;
mod semantic_identity;

pub use semantic_identity::{LanguageId, ProviderId};

pub use hook_client_config::{
    AgentActionAuthorityRule, AgentActionEffectRule, AspProjectConfigFile,
    AspProjectDiscoveryConfig, AspProjectHookConfig, CLIENT_HOOK_CONFIG_SCHEMA_ID,
    CLIENT_HOOK_CONFIG_SCHEMA_VERSION, HookClientActionAuthority, HookClientActionKind,
    HookClientActionSubjectKind, HookClientAgentOrgArtifactsArchiveWarningConfig,
    HookClientAgentOrgArtifactsConfig, HookClientAgentPlaceholder,
    HookClientAgentSessionGuideConfig, HookClientAgentSessionMessagesConfig,
    HookClientAgentsConfig, HookClientCommandProfileConfig, HookClientCommandProfileRef,
    HookClientConfigDecision, HookClientConfigFile, HookClientConfigReasonKind,
    HookClientConfigRouteKind, HookClientConfigStdinMode, HookClientLanguageProviderConfig,
    HookClientLazyProviderPolicy, HookClientRecoveryPromptConfig, HookClientResidentAgentConfig,
    HookClientRuleConfig, HookClientRuleDispatchConfig, HookClientRuleDispatchTransport,
    HookClientRuleMatchConfig, HookClientRuleRouteConfig, WrapperMatchMode,
    default_hook_client_config_file, default_hook_client_config_template,
    expand_command_profile_prefixes, load_asp_project_config_file,
    load_hook_client_config_declared_contract_fingerprint, load_hook_client_config_file,
    load_hook_client_config_overlay_file, merge_asp_project_hook_config,
    render_hook_client_message_template,
};
pub use hook_client_config::{
    HookClientStructuredFilterGrammar, HookClientStructuredProjectionMatchConfig,
};
pub mod source_extension;
