// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use crate::runtime_server_admission::WorkspaceGenerationBuildFailure;
use std::future::Future;

#[derive(Clone, Copy)]
pub(crate) enum Stage {
    WorkspaceBootstrap,
    DurableRestore,
    SourceBuilder,
    SourceIndexCommit,
    CanonicalGenerationPublication,
}

impl Stage {
    fn label(self) -> &'static str {
        match self {
            Self::WorkspaceBootstrap => "workspace-bootstrap",
            Self::DurableRestore => "durable-restore",
            Self::SourceBuilder => "source-builder",
            Self::SourceIndexCommit => "source-index-commit",
            Self::CanonicalGenerationPublication => "canonical-generation-publication",
        }
    }
}

pub(crate) async fn await_stage<T, F>(
    workspace_identity: &str,
    operation_id: &str,
    stage: Stage,
    future: F,
) -> Result<T, WorkspaceGenerationBuildFailure>
where
    F: Future<Output = Result<T, WorkspaceGenerationBuildFailure>>,
{
    let started = std::time::Instant::now();
    let result = future.await;
    let mut observation = agent_semantic_runtime_observability::RuntimePerformanceObservation::new(
        "workspace-generation-admission",
        stage.label(),
        u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX),
        0,
        if result.is_ok() { "observed" } else { "failed" },
    )
    .with_operation_id(operation_id.to_owned());
    observation.workspace_identity = Some(workspace_identity.to_owned());
    if let Err(error) = &result {
        observation.failure_reason = Some(format!("{:?}", error.stage).to_lowercase());
    }
    let _ = agent_semantic_runtime_observability::try_record_to_active_runtime(observation);
    result
}

pub(crate) fn restored_generation_covers_demand(
    memory_registry: &crate::runtime_server_workspace::RuntimeServerWorkspaceRegistry,
    workspace_identity: &str,
    project_root: &std::path::Path,
    target_paths: &std::collections::BTreeSet<std::path::PathBuf>,
    provider_target: Option<&crate::runtime_server_admission::WorkspaceGenerationProviderTarget>,
) -> bool {
    let Ok(resident) = memory_registry.resident_read_client(workspace_identity, project_root)
    else {
        return false;
    };
    target_paths.iter().all(|path| {
        let owner_path = path
            .strip_prefix(project_root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        resident
            .owner_snapshot(&owner_path)
            .ok()
            .flatten()
            .is_some_and(|owner| {
                provider_target.is_none_or(|target| {
                    owner.authority.as_ref().is_some_and(|authority| {
                        authority.language_id.as_str() == target.language_id
                            && target.provider_id.as_ref().is_none_or(|provider_id| {
                                authority.provider_id.as_str() == provider_id
                            })
                    })
                })
            })
    })
}
