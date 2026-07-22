//! Hook client configuration interface.

mod agent_runtime;
mod document;
mod invocation;
mod routing;
mod validation;

pub use routing::HookClientStructuredFormat;
pub use routing::{HookClientStructuredFilterGrammar, HookClientStructuredProjectionMatchConfig};

pub use agent_runtime::{HookClientAgentsConfig, HookClientResidentAgentConfig};
pub use document::{
    AspProjectConfigFile, AspProjectHookConfig, CLIENT_HOOK_CONFIG_SCHEMA_ID,
    CLIENT_HOOK_CONFIG_SCHEMA_VERSION, HookClientAgentOrgArtifactsArchiveWarningConfig,
    HookClientAgentOrgArtifactsConfig, HookClientAgentSessionGuideConfig,
    HookClientAgentSessionMessagesConfig, HookClientConfigFile, HookClientRecoveryPromptConfig,
    default_hook_client_config_file, default_hook_client_config_template,
    hook_client_contract_fingerprint, load_asp_project_config_file, load_hook_client_config_file,
    merge_asp_project_hook_config, render_hook_client_message_template,
};
pub use invocation::{
    HookClientAuthorityProjection, HookClientCommandWrapper, HookClientEffectProjection,
    HookClientFlagPresence, HookClientInvocationShape, HookClientWrapperMatch,
};
pub use routing::{
    HookClientActionAuthority, HookClientActionKind, HookClientActionSubjectKind,
    HookClientConfigDecision, HookClientConfigReasonKind, HookClientConfigRouteKind,
    HookClientConfigStdinMode, HookClientDecisionMaterializer, HookClientLazyProviderPolicy,
    HookClientRuleConfig, HookClientRuleDispatchConfig, HookClientRuleDispatchTransport,
    HookClientRuleMatchConfig, HookClientRuleRouteConfig,
};
