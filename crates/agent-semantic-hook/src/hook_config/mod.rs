// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Optional client-side hook rules loaded on each hook invocation.

mod agent_org_config;
pub(crate) use agent_org_config::compile_agent_org_artifacts_config;
mod core;
mod core_load;

pub use core::ClientHookConfig;
pub use core::DurableHookConfigArtifact;
pub(crate) use core::HookPolicyCandidate;
pub use core::MaterializedDecisionShards;
pub use core_load::default_client_config_path;
pub use core_load::default_client_config_projection_digest;
pub use core_load::default_client_config_template;
pub use core_load::hook_runtime_artifact_fingerprint;
pub use core_load::load_client_config;
pub use core_load::load_client_config_for_matcher_publication;
pub use core_load::load_client_config_for_project;
pub use core_load::load_client_config_for_project_with_executable_capabilities;
pub use core_load::load_client_config_overlay_for_project;
pub use core_load::load_embedded_client_config_for_project;
