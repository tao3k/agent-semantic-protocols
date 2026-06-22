#![deny(dead_code)]

//! Unified project identity, configuration, and local state layout for ASP.

mod hook_client_config;
mod layout;

pub use hook_client_config::{
    AspProjectConfigFile, AspProjectHookConfig, CLIENT_HOOK_CONFIG_SCHEMA_ID,
    CLIENT_HOOK_CONFIG_SCHEMA_VERSION, HookClientAgentOrgArtifactsArchiveWarningConfig,
    HookClientAgentOrgArtifactsConfig, HookClientConfigDecision, HookClientConfigFile,
    HookClientConfigReasonKind, HookClientConfigRouteKind, HookClientConfigStdinMode,
    HookClientRuleConfig, HookClientRuleMatchConfig, HookClientRuleRouteConfig,
    default_hook_client_config_path, default_hook_client_config_template,
    default_hook_client_config_template_for_source_extensions, load_asp_project_config_file,
    load_hook_client_config_file,
};
pub use layout::{
    PRJ_CACHE_HOME_ENV, ProjectCacheSource, ProjectEnvStatus, ProjectRuntimeEnv,
    ProjectRuntimeLayout, project_activation_path, project_artifacts_dir, project_cache_root,
    project_cache_root_with_env, project_client_cache_dir, project_hook_cache_dir,
    project_hook_state_dir, project_protocol_home, project_provider_lock_dir,
    project_runtime_bin_dir, project_runtime_layout, project_runtime_layout_with_env,
};
