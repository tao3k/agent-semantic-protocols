//! Hook client configuration interface.

mod agent_runtime;
mod asp_command_intent;
mod document;
mod intent_policy;
mod routing;
pub use asp_command_intent::{
    AspCommandIntent, AspCommandIntentMatch, AspCommandRouteId, StructuralSelector,
    classify_asp_language_command,
};
mod validation;

pub use agent_runtime::{
    HookClientAgentsConfig, HookClientExecutionLaneConfig, HookClientExecutionLanesConfig,
    HookClientExecutionTransport, HookClientResidentAgentConfig,
};
pub use document::{
    AspProjectConfigFile, AspProjectHookConfig, CLIENT_HOOK_CONFIG_SCHEMA_ID,
    CLIENT_HOOK_CONFIG_SCHEMA_VERSION, HookClientAgentOrgArtifactsArchiveWarningConfig,
    HookClientAgentOrgArtifactsConfig, HookClientAgentSessionGuideConfig,
    HookClientAgentSessionMessagesConfig, HookClientConfigFile, HookClientRecoveryPromptConfig,
    default_hook_client_config_file, default_hook_client_config_template,
    default_hook_client_config_template_for_source_extensions, hook_client_contract_fingerprint,
    load_asp_project_config_file, load_hook_client_config_file,
    render_hook_client_message_template,
};
pub use intent_policy::{
    HookClientAspCommandIntentPolicyConfig, HookClientAspControlPlaneIntentConfig,
    HookClientAspExactEvidenceIntentConfig, HookClientAspInvalidEvidenceIntentConfig,
    HookClientAspReasoningIntentConfig,
};
pub use routing::{
    HookClientConfigDecision, HookClientConfigReasonKind, HookClientConfigRouteKind,
    HookClientConfigStdinMode, HookClientDecisionMaterializer, HookClientRuleConfig,
    HookClientRuleMatchConfig, HookClientRuleRouteConfig,
};
