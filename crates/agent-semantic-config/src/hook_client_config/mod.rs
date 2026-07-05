//! Hook client configuration interface.

mod model;
mod validation;

pub use model::{
    AspProjectConfigFile, AspProjectHookConfig, CLIENT_HOOK_CONFIG_SCHEMA_ID,
    CLIENT_HOOK_CONFIG_SCHEMA_VERSION, HookClientAgentOrgArtifactsArchiveWarningConfig,
    HookClientAgentOrgArtifactsConfig, HookClientAgentSessionGuideConfig,
    HookClientAgentSessionMessagesConfig, HookClientAgentsConfig, HookClientConfigDecision,
    HookClientConfigFile, HookClientConfigReasonKind, HookClientConfigRouteKind,
    HookClientConfigStdinMode, HookClientRecoveryPromptConfig, HookClientResidentAgentConfig,
    HookClientRuleConfig, HookClientRuleMatchConfig, HookClientRuleRouteConfig,
    default_hook_client_config_file, default_hook_client_config_template,
    default_hook_client_config_template_for_source_extensions, load_asp_project_config_file,
    load_hook_client_config_file, render_hook_client_message_template,
};
