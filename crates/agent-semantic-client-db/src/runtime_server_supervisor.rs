//! Protocol-neutral Runtime Server supervisor boundary.

use std::path::PathBuf;

pub struct SupervisorRequest {
    pub state_home: PathBuf,
    pub expected_executable: PathBuf,
    pub launch: agent_semantic_runtime::runtime_process_lifecycle::RuntimeProcessLaunchSpec,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SupervisorOutcome { AlreadyResident, SpawnAccepted, OwnerStale, Failed }

pub struct RuntimeServerSupervisor;

impl RuntimeServerSupervisor {
    pub async fn classify_owner(&self, request: &SupervisorRequest) -> Result<SupervisorOutcome, String> {
        let receipt = crate::runtime_server_lifecycle::read_owner_receipt(&request.state_home).await?;
        let coordinator = crate::runtime_server_lifecycle_coordinator::RuntimeServerLifecycleCoordinator::new(&request.state_home, &request.expected_executable);
        Ok(match receipt {
            Some(receipt) if coordinator.classify(Some(receipt.process_id)).await? == crate::runtime_server_lifecycle_coordinator::OwnerClassification::Live => SupervisorOutcome::AlreadyResident,
            Some(_) => SupervisorOutcome::OwnerStale,
            None => SupervisorOutcome::SpawnAccepted,
        })
    }
}
