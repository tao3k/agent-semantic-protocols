//! Immutable Ready-generation query executor owned by Runtime Server.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::watch;

use agent_semantic_client_db::runtime_resident_read::RuntimeResidentReadClient;

pub struct RuntimeQueryGeneration {
    generation_digest: String,
    resident: Arc<RuntimeResidentReadClient>,
}

#[derive(Clone)]
pub enum RuntimeQueryGenerationState {
    Ready(Arc<RuntimeQueryGeneration>),
    Failed(Arc<str>),
}

#[derive(Clone)]
pub struct RuntimeQueryGenerationAuthority {
    sender: watch::Sender<Arc<HashMap<String, RuntimeQueryGenerationState>>>,
    open_lanes: Arc<tokio::sync::Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>>,
}

impl RuntimeQueryGenerationAuthority {
    pub fn new() -> Self {
        let (sender, _) = watch::channel(Arc::new(HashMap::new()));
        Self {
            sender,
            open_lanes: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        }
    }

    pub fn subscribe(&self) -> watch::Receiver<Arc<HashMap<String, RuntimeQueryGenerationState>>> {
        self.sender.subscribe()
    }

    pub fn publish_ready(
        &self,
        workspace_identity: String,
        generation: Arc<RuntimeQueryGeneration>,
    ) {
        let mut generations = self.sender.borrow().as_ref().clone();
        generations.insert(
            workspace_identity,
            RuntimeQueryGenerationState::Ready(generation),
        );
        self.sender.send_replace(Arc::new(generations));
    }

    pub fn publish_failed(&self, workspace_identity: String, reason: impl Into<Arc<str>>) {
        let mut generations = self.sender.borrow().as_ref().clone();
        generations.insert(
            workspace_identity,
            RuntimeQueryGenerationState::Failed(reason.into()),
        );
        self.sender.send_replace(Arc::new(generations));
    }

    pub fn clear_workspace(&self, workspace_identity: &str) {
        let mut generations = self.sender.borrow().as_ref().clone();
        generations.remove(workspace_identity);
        self.sender.send_replace(Arc::new(generations));
    }

    pub fn clear_all(&self) {
        self.sender.send_replace(Arc::new(HashMap::new()));
    }

    pub async fn ensure_ready(
        &self,
        workspace_identity: &str,
        pointer_path: &std::path::Path,
        project_root: &std::path::Path,
        expected_generation_digest: &str,
    ) -> Result<Arc<RuntimeQueryGeneration>, String> {
        if let Some(RuntimeQueryGenerationState::Ready(generation)) =
            self.sender.borrow().get(workspace_identity)
            && generation.generation_digest() == expected_generation_digest
        {
            return Ok(Arc::clone(generation));
        }
        let lane = {
            let mut lanes = self.open_lanes.lock().await;
            Arc::clone(
                lanes
                    .entry(workspace_identity.to_owned())
                    .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(()))),
            )
        };
        let _guard = lane.lock().await;
        if let Some(RuntimeQueryGenerationState::Ready(generation)) =
            self.sender.borrow().get(workspace_identity)
            && generation.generation_digest() == expected_generation_digest
        {
            return Ok(Arc::clone(generation));
        }
        match RuntimeQueryGeneration::open(pointer_path, project_root).await {
            Ok(generation) if generation.generation_digest() == expected_generation_digest => {
                let generation = Arc::new(generation);
                self.publish_ready(workspace_identity.to_owned(), Arc::clone(&generation));
                Ok(generation)
            }
            Ok(generation) => {
                let error = format!(
                    "generation digest mismatch: expected={} actual={}",
                    expected_generation_digest,
                    generation.generation_digest()
                );
                self.publish_failed(workspace_identity.to_owned(), error.clone());
                Err(error)
            }
            Err(error) => {
                self.publish_failed(workspace_identity.to_owned(), error.clone());
                Err(error)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{RuntimeQueryGenerationAuthority, RuntimeQueryGenerationState};

    #[test]
    fn authority_starts_empty_and_can_clear_all() {
        let authority = RuntimeQueryGenerationAuthority::new();
        let receiver = authority.subscribe();
        assert!(receiver.borrow().is_empty());
        authority.clear_all();
        assert!(receiver.borrow().is_empty());
    }

    #[tokio::test]
    async fn failed_open_publishes_a_typed_workspace_state() {
        let authority = RuntimeQueryGenerationAuthority::new();
        let receiver = authority.subscribe();
        let missing = std::path::Path::new("/definitely-missing-asp-generation/pointer");

        let result = authority
            .ensure_ready(
                "workspace-test",
                missing,
                std::path::Path::new("/definitely-missing-asp-generation/project"),
                "blake3-256:expected",
            )
            .await;
        let Err(error) = result else {
            panic!("missing generation must fail");
        };

        assert!(!error.is_empty());
        assert!(matches!(
            receiver.borrow().get("workspace-test"),
            Some(RuntimeQueryGenerationState::Failed(_))
        ));
    }
}

impl RuntimeQueryGeneration {
    pub async fn open(
        pointer_path: &std::path::Path,
        project_root: &std::path::Path,
    ) -> Result<Self, String> {
        let resident = RuntimeResidentReadClient::open(pointer_path, project_root).await?;
        let generation_digest = resident.generation_digest();
        Ok(Self {
            generation_digest,
            resident: Arc::new(resident),
        })
    }

    pub fn generation_digest(&self) -> &str {
        &self.generation_digest
    }

    pub fn resident(&self) -> &RuntimeResidentReadClient {
        &self.resident
    }
}
