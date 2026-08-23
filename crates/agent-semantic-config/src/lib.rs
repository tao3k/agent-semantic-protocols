#![deny(dead_code)]

//! Unified project identity, configuration, and local state layout for ASP.

pub mod agent_route_registry;
mod codex_plugin_config;
pub mod embedded_agent_assets;
mod hook_client_config;
pub mod runtime_dev;

pub use codex_plugin_config::codex_config_plugin_enabled;

pub use hook_client_config::hook_client_contract_fingerprint;

pub use hook_client_config::HookClientStructuredFormat;
mod semantic_identity;

pub use semantic_identity::{LanguageId, ProviderId};

pub use hook_client_config::{
    AspProjectConfigFile, AspProjectDiscoveryConfig, AspProjectHookConfig,
    CLIENT_HOOK_CONFIG_SCHEMA_ID, CLIENT_HOOK_CONFIG_SCHEMA_VERSION, HookClientActionKind,
    HookClientActionSubjectKind, HookClientAgentOrgArtifactsArchiveWarningConfig,
    HookClientAgentOrgArtifactsConfig, HookClientAgentRoleSelector,
    HookClientCapabilityPolicyConfig, HookClientCommandProfileConfig, HookClientCommandProfileRef,
    HookClientConfigDecision, HookClientConfigFile, HookClientConfigReasonKind,
    HookClientConfigRouteKind, HookClientConfigStdinMode, HookClientLazyProviderPolicy,
    HookClientMatcherPolicy, HookClientProfileConfig, HookClientRecoveryPromptConfig,
    HookClientRuleConfig, HookClientRuleDispatchConfig, HookClientRuleDispatchTransport,
    HookClientRuleMatchConfig, HookClientRuleRouteConfig, HookPolicyCoverageCase,
    HookPolicyCoveragePolarity, HookPolicyCoverageSettings, HookPolicyCoverageSurface,
    WrapperMatchMode, default_hook_client_config_file, default_hook_client_config_template,
    derive_hook_policy_coverage_cases, expand_command_profile_prefixes,
    load_asp_project_config_file, load_hook_client_config_declared_contract_fingerprint,
    load_hook_client_config_file, load_hook_client_config_overlay_file,
    merge_asp_project_hook_config, mutate_path_outside_registered_extensions,
    render_hook_client_message_template,
};
pub use hook_client_config::{
    HookClientStructuredFilterGrammar, HookClientStructuredProjectionMatchConfig,
};
pub mod source_extension;
