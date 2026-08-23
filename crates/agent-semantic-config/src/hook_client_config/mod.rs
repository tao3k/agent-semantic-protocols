//! Hook client configuration interface.

mod document;
mod policy_coverage;
mod profiles;
mod routing;
mod validation;

pub use routing::HookClientStructuredFormat;
pub use routing::{HookClientStructuredFilterGrammar, HookClientStructuredProjectionMatchConfig};

pub use document::{
    AspProjectConfigFile, AspProjectDiscoveryConfig, AspProjectHookConfig,
    CLIENT_HOOK_CONFIG_SCHEMA_ID, CLIENT_HOOK_CONFIG_SCHEMA_VERSION,
    HookClientAgentOrgArtifactsArchiveWarningConfig, HookClientAgentOrgArtifactsConfig,
    HookClientConfigFile, HookClientProfileConfig, HookClientRecoveryPromptConfig,
    WrapperMatchMode, default_hook_client_config_file, default_hook_client_config_template,
    hook_client_contract_fingerprint, load_asp_project_config_file,
    load_hook_client_config_declared_contract_fingerprint, load_hook_client_config_file,
    load_hook_client_config_overlay_file, merge_asp_project_hook_config,
    render_hook_client_message_template,
};
pub use policy_coverage::{
    HookPolicyCoverageCase, HookPolicyCoveragePolarity, HookPolicyCoverageSettings,
    HookPolicyCoverageSurface, derive_hook_policy_coverage_cases,
    mutate_path_outside_registered_extensions,
};
pub use profiles::{
    HookClientCommandProfileConfig, HookClientCommandProfileRef, expand_command_profile_prefixes,
};
pub use routing::{
    HookClientActionKind, HookClientActionSubjectKind, HookClientAgentRoleSelector,
    HookClientCapabilityPolicyConfig, HookClientConfigDecision, HookClientConfigReasonKind,
    HookClientConfigRouteKind, HookClientConfigStdinMode, HookClientLazyProviderPolicy,
    HookClientMatcherPolicy, HookClientRuleConfig, HookClientRuleDispatchConfig,
    HookClientRuleDispatchTransport, HookClientRuleMatchConfig, HookClientRuleRouteConfig,
};
