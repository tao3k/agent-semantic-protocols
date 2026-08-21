//! Runtime Server lifecycle coordinator owned by client-db composition.

use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnerClassification { Missing, Live, Stale }

pub struct RuntimeServerLifecycleCoordinator {
    pub state_home: PathBuf,
    pub expected_executable: PathBuf,
}

impl RuntimeServerLifecycleCoordinator {
    pub fn new(state_home: impl Into<PathBuf>, expected_executable: impl Into<PathBuf>) -> Self {
        Self { state_home: state_home.into(), expected_executable: expected_executable.into() }
    }

    pub async fn classify(&self, process_id: Option<u32>) -> Result<OwnerClassification, String> {
        let Some(process_id) = process_id else { return Ok(OwnerClassification::Missing); };
        if !agent_semantic_runtime::runtime_process_lifecycle::process_id_is_alive(process_id).await {
            return Ok(OwnerClassification::Stale);
        }
        if !agent_semantic_runtime::runtime_process_lifecycle::process_executable_matches(process_id, &self.expected_executable).await? {
            return Err("runtime server owner executable identity mismatch".to_owned());
        }
        Ok(OwnerClassification::Live)
    }

    pub async fn terminate_verified(&self, process_id: u32, force: bool) -> Result<(), String> {
        if self.classify(Some(process_id)).await? != OwnerClassification::Live {
            return Err("runtime server owner is not a verified live process".to_owned());
        }
        if force {
            agent_semantic_runtime::runtime_process_lifecycle::force_terminate(process_id).await
        } else {
            agent_semantic_runtime::runtime_process_lifecycle::terminate(process_id).await
        }
    }
}

pub fn owner_receipt_path(state_home: &Path) -> PathBuf {
    state_home.join("runtime/server/owner-spawn.v1.json")
}
