//! Optional client-side hook rules loaded on each hook invocation.

mod agent_org_config;
pub(crate) use agent_org_config::compile_agent_org_artifacts_config;
mod core;
mod core_load;

pub(crate) use core::HookPolicyCandidate;
pub use core::{ClientHookConfig, DurableHookConfigArtifact, MaterializedDecisionShards};
pub use core_load::{
    default_client_config_path, default_client_config_projection_digest,
    default_client_config_template, hook_runtime_artifact_fingerprint, load_client_config,
    load_client_config_for_matcher_publication, load_client_config_for_project,
    load_client_config_for_project_with_executable_capabilities,
    load_client_config_overlay_for_project, load_embedded_client_config_for_project,
};
