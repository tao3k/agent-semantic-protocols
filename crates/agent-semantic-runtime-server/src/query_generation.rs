//! Immutable Ready-generation query executor owned by Runtime Server.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::watch;

use agent_semantic_client_db::runtime_resident_read::RuntimeResidentReadClient;

pub struct RuntimeQueryGeneration {
    generation_digest: String,
    generation_token: AtomicU64,
    resident: Option<Arc<RuntimeResidentReadClient>>,
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
    next_generation_token: Arc<AtomicU64>,
    publication_lock: Arc<std::sync::Mutex<()>>,
}

impl RuntimeQueryGenerationAuthority {
    pub fn new() -> Self {
        let (sender, _) = watch::channel(Arc::new(HashMap::new()));
        Self {
            sender,
            open_lanes: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            next_generation_token: Arc::new(AtomicU64::new(0)),
            publication_lock: Arc::new(std::sync::Mutex::new(())),
        }
    }

    pub fn subscribe(&self) -> watch::Receiver<Arc<HashMap<String, RuntimeQueryGenerationState>>> {
        self.sender.subscribe()
    }

    pub fn publish_ready(
        &self,
        workspace_identity: String,
        generation: Arc<RuntimeQueryGeneration>,
    ) -> Result<u64, String> {
        let _publication_guard = self
            .publication_lock
            .lock()
            .map_err(|_| "query generation publication lock poisoned".to_owned())?;
        if let Some(RuntimeQueryGenerationState::Ready(current)) =
            self.sender.borrow().get(&workspace_identity)
            && Arc::ptr_eq(current, &generation)
        {
            let current_token = current.generation_token();
            if current_token != 0 {
                return Ok(current_token);
            }
        }
        let generation_token = if generation.generation_token.load(Ordering::Acquire) == 0 {
            let next = self.next_generation_token.fetch_add(1, Ordering::AcqRel) + 1;
            match generation.generation_token.compare_exchange(
                0,
                next,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) | Err(_) => generation.generation_token.load(Ordering::Acquire),
            }
        } else {
            generation.generation_token.load(Ordering::Acquire)
        };
        let mut generations = self.sender.borrow().as_ref().clone();
        if let Some(RuntimeQueryGenerationState::Ready(current)) = generations.get(&workspace_identity)
            && generation_token <= current.generation_token()
        {
            return Err(format!(
                "state=stale-generation reasonKind=non-monotonic-query-generation-publication workspaceIdentity={workspace_identity} token={generation_token} currentToken={}",
                current.generation_token()
            ));
        }
        generations.insert(
            workspace_identity,
            RuntimeQueryGenerationState::Ready(generation),
        );
        self.sender.send_replace(Arc::new(generations));
        Ok(generation_token)
    }

    pub fn publish_failed(
        &self,
        workspace_identity: String,
        publication_token: u64,
        reason: impl Into<Arc<str>>,
    ) {
        let Ok(_publication_guard) = self.publication_lock.lock() else { return };
        if let Some(RuntimeQueryGenerationState::Ready(current)) =
            self.sender.borrow().get(&workspace_identity)
            && current.generation_token() >= publication_token
        {
            return;
        }
        let mut generations = self.sender.borrow().as_ref().clone();
        generations.insert(
            workspace_identity,
            RuntimeQueryGenerationState::Failed(reason.into()),
        );
        self.sender.send_replace(Arc::new(generations));
    }

    pub fn clear_workspace(&self, workspace_identity: &str) {
        let Ok(_publication_guard) = self.publication_lock.lock() else { return };
        let mut generations = self.sender.borrow().as_ref().clone();
        generations.remove(workspace_identity);
        self.sender.send_replace(Arc::new(generations));
    }

    pub fn clear_all(&self) {
        let Ok(_publication_guard) = self.publication_lock.lock() else { return };
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
                self.publish_ready(workspace_identity.to_owned(), Arc::clone(&generation))
                    .map(|_| generation)
            }
            Ok(generation) => {
                let error = format!(
                    "generation digest mismatch: expected={} actual={}",
                    expected_generation_digest,
                    generation.generation_digest()
                );
                self.publish_failed(
                    workspace_identity.to_owned(),
                    generation.generation_token(),
                    error.clone(),
                );
                Err(error)
            }
            Err(error) => {
                self.publish_failed(
                    workspace_identity.to_owned(),
                    0,
                    error.clone(),
                );
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

    fn test_generation(digest: &str) -> std::sync::Arc<super::RuntimeQueryGeneration> {
        std::sync::Arc::new(super::RuntimeQueryGeneration {
            generation_digest: digest.to_owned(),
            generation_token: std::sync::atomic::AtomicU64::new(0),
            resident: None,
        })
    }

    #[test]
    fn republishing_old_arc_cannot_mint_or_rollback() {
        let authority = RuntimeQueryGenerationAuthority::new();
        let old = test_generation("blake3-256:old");
        let newer = test_generation("blake3-256:newer");
        let old_token = authority
            .publish_ready("workspace-test".to_owned(), std::sync::Arc::clone(&old))
            .expect("first publication");
        let newer_token = authority
            .publish_ready("workspace-test".to_owned(), std::sync::Arc::clone(&newer))
            .expect("newer publication");
        assert!(newer_token > old_token);
        assert!(authority
            .publish_ready("workspace-test".to_owned(), old)
            .is_err());
        let current = authority.subscribe();
        let snapshot = current.borrow().clone();
        let super::RuntimeQueryGenerationState::Ready(current) =
            snapshot.get("workspace-test").expect("current")
        else { panic!("expected ready generation") };
        assert_eq!(current.generation_digest(), "blake3-256:newer");
        assert_eq!(current.generation_token(), newer_token);
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
            generation_token: AtomicU64::new(0),
            resident: Some(Arc::new(resident)),
        })
    }

    pub fn generation_digest(&self) -> &str {
        &self.generation_digest
    }

    pub fn generation_token(&self) -> u64 {
        self.generation_token.load(Ordering::Acquire)
    }

    pub fn resident(&self) -> &RuntimeResidentReadClient {
        self.resident
            .as_deref()
            .expect("ready query generation always owns a resident read client")
    }
}
