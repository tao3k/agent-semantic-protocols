//! Atomic publication and exact-identity admission for resident query generations.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use tokio::sync::watch;
use tokio_stream::Stream;

use crate::query_generation::RuntimeSearchGenerationBuilder;
use crate::runtime_query_generation::RuntimeQueryGeneration;
use crate::runtime_query_generation_key::{RuntimeProjectWorkspaceKey, validate_ready_identity};
use agent_semantic_search::{
    RuntimeSearchDerivedAttachmentEvent, RuntimeSearchDerivedAttachmentSnapshot,
};

#[derive(Clone)]
pub enum RuntimeQueryGenerationState {
    Ready(Arc<RuntimeQueryGeneration>),
    Failed {
        expected_generation_digest: Arc<str>,
        reason: Arc<str>,
    },
}

#[derive(Clone)]
pub struct RuntimeQueryGenerationAuthority {
    sender: watch::Sender<Arc<HashMap<RuntimeProjectWorkspaceKey, RuntimeQueryGenerationState>>>,
    open_lanes:
        Arc<tokio::sync::Mutex<HashMap<RuntimeProjectWorkspaceKey, Arc<tokio::sync::Mutex<()>>>>>,
    next_generation_token: Arc<AtomicU64>,
    publication_lock: Arc<std::sync::Mutex<()>>,
    builder: Arc<RuntimeSearchGenerationBuilder>,
}

impl RuntimeQueryGenerationAuthority {
    pub fn new() -> Self {
        Self::new_in_task_scope(
            agent_semantic_client_db::runtime_server_runtime::RuntimeServerTaskScope::new(
                "runtime-query-generation",
            ),
            agent_semantic_client_db::runtime_server_runtime::RuntimeServerResourceSupervisor::for_current_daemon(),
        )
        .expect("a new Runtime query generation task scope must admit its builder")
    }

    pub fn new_in_task_scope(
        task_scope: agent_semantic_client_db::runtime_server_runtime::RuntimeServerTaskScope,
        resource_supervisor: agent_semantic_client_db::runtime_server_runtime::RuntimeServerResourceSupervisor,
    ) -> Result<Self, String> {
        Self::new_in_task_scope_with_calibration_store(task_scope, resource_supervisor, None)
    }

    pub fn new_in_task_scope_with_calibration_store(
        task_scope: agent_semantic_client_db::runtime_server_runtime::RuntimeServerTaskScope,
        resource_supervisor: agent_semantic_client_db::runtime_server_runtime::RuntimeServerResourceSupervisor,
        calibration_store_path: Option<std::path::PathBuf>,
    ) -> Result<Self, String> {
        let (sender, _) = watch::channel(Arc::new(HashMap::new()));
        Ok(Self {
            sender,
            open_lanes: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            next_generation_token: Arc::new(AtomicU64::new(0)),
            publication_lock: Arc::new(std::sync::Mutex::new(())),
            builder: Arc::new(RuntimeSearchGenerationBuilder::new_with_calibration_store(
                task_scope,
                resource_supervisor,
                calibration_store_path,
            )?),
        })
    }

    pub fn subscribe(
        &self,
    ) -> watch::Receiver<Arc<HashMap<RuntimeProjectWorkspaceKey, RuntimeQueryGenerationState>>>
    {
        self.sender.subscribe()
    }

    pub fn subscribe_derived_attachment_events(
        &self,
    ) -> Pin<Box<dyn Stream<Item = Result<RuntimeSearchDerivedAttachmentEvent, String>> + Send>>
    {
        self.builder.subscribe_attachment_events()
    }

    pub fn derived_attachment_snapshot(&self) -> Arc<RuntimeSearchDerivedAttachmentSnapshot> {
        self.builder.derived_attachment_snapshot()
    }

    pub fn publish_ready(
        &self,
        key: RuntimeProjectWorkspaceKey,
        generation: Arc<RuntimeQueryGeneration>,
    ) -> Result<u64, String> {
        let _publication_guard = self
            .publication_lock
            .lock()
            .map_err(|_| "query generation publication lock poisoned".to_owned())?;
        if let Some(RuntimeQueryGenerationState::Ready(current)) = self.sender.borrow().get(&key)
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
        if let Some(RuntimeQueryGenerationState::Ready(current)) = generations.get(&key)
            && generation_token <= current.generation_token()
        {
            return Err(format!(
                "state=stale-generation reasonKind=non-monotonic-query-generation-publication projectId={} workspaceId={} token={generation_token} currentToken={}",
                key.project_id().as_str(),
                key.workspace_id().as_str(),
                current.generation_token()
            ));
        }
        generations.insert(key, RuntimeQueryGenerationState::Ready(generation));
        self.sender.send_replace(Arc::new(generations));
        Ok(generation_token)
    }

    pub fn publish_failed(
        &self,
        key: RuntimeProjectWorkspaceKey,
        publication_token: u64,
        expected_generation_digest: impl Into<Arc<str>>,
        reason: impl Into<Arc<str>>,
    ) {
        let Ok(_publication_guard) = self.publication_lock.lock() else {
            return;
        };
        if let Some(RuntimeQueryGenerationState::Ready(current)) = self.sender.borrow().get(&key)
            && current.generation_token() >= publication_token
        {
            return;
        }
        let mut generations = self.sender.borrow().as_ref().clone();
        generations.insert(
            key,
            RuntimeQueryGenerationState::Failed {
                expected_generation_digest: expected_generation_digest.into(),
                reason: reason.into(),
            },
        );
        self.sender.send_replace(Arc::new(generations));
    }

    pub fn clear_workspace(&self, key: &RuntimeProjectWorkspaceKey) {
        let Ok(_publication_guard) = self.publication_lock.lock() else {
            return;
        };
        let mut generations = self.sender.borrow().as_ref().clone();
        generations.remove(key);
        self.sender.send_replace(Arc::new(generations));
    }

    pub fn require_workspace_absent(&self, key: &RuntimeProjectWorkspaceKey) -> Result<(), String> {
        let _publication_guard = self
            .publication_lock
            .lock()
            .map_err(|_| "query generation publication lock poisoned".to_owned())?;
        if self.sender.borrow().contains_key(key) {
            return Err(format!(
                "state=cache-state-conflict reasonKind=cold-build-workspace-already-published projectId={} workspaceId={}",
                key.project_id().as_str(),
                key.workspace_id().as_str()
            ));
        }
        Ok(())
    }

    pub fn verify_ready_exact(
        &self,
        key: &RuntimeProjectWorkspaceKey,
        expected_generation_digest: &str,
        expected_root_digest: &str,
    ) -> Result<(), String> {
        let _publication_guard = self
            .publication_lock
            .lock()
            .map_err(|_| "query generation publication lock poisoned".to_owned())?;
        let generations = self.sender.borrow();
        let Some(RuntimeQueryGenerationState::Ready(generation)) = generations.get(key) else {
            return Err(format!(
                "state=query-not-ready reasonKind=resident-generation-missing projectId={} workspaceId={}",
                key.project_id().as_str(),
                key.workspace_id().as_str()
            ));
        };
        validate_ready_identity(
            key,
            generation,
            expected_generation_digest,
            expected_root_digest,
        )
    }

    pub fn evict_ready_exact(
        &self,
        key: &RuntimeProjectWorkspaceKey,
        expected_generation_digest: &str,
        expected_root_digest: &str,
    ) -> Result<bool, String> {
        let _publication_guard = self
            .publication_lock
            .lock()
            .map_err(|_| "query generation publication lock poisoned".to_owned())?;
        let mut generations = self.sender.borrow().as_ref().clone();
        let Some(RuntimeQueryGenerationState::Ready(generation)) = generations.get(key) else {
            return Err(format!(
                "state=query-not-ready reasonKind=resident-generation-missing projectId={} workspaceId={}",
                key.project_id().as_str(),
                key.workspace_id().as_str()
            ));
        };
        validate_ready_identity(
            key,
            generation,
            expected_generation_digest,
            expected_root_digest,
        )?;
        generations.remove(key);
        self.sender.send_replace(Arc::new(generations));
        Ok(true)
    }

    pub fn clear_all(&self) {
        let Ok(_publication_guard) = self.publication_lock.lock() else {
            return;
        };
        self.sender.send_replace(Arc::new(HashMap::new()));
    }

    pub async fn shutdown(&self) -> Result<(), String> {
        self.clear_all();
        self.builder.shutdown().await
    }

    pub async fn ensure_ready(
        &self,
        key: &RuntimeProjectWorkspaceKey,
        pointer_path: &std::path::Path,
        project_root: &std::path::Path,
        expected_generation_digest: &str,
    ) -> Result<Arc<RuntimeQueryGeneration>, String> {
        if let Some(RuntimeQueryGenerationState::Ready(generation)) = self.sender.borrow().get(key)
            && generation.generation_digest() == expected_generation_digest
        {
            return Ok(Arc::clone(generation));
        }
        let lane = {
            let mut lanes = self.open_lanes.lock().await;
            Arc::clone(
                lanes
                    .entry(key.clone())
                    .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(()))),
            )
        };
        let _guard = lane.lock().await;
        if let Some(RuntimeQueryGenerationState::Ready(generation)) = self.sender.borrow().get(key)
            && generation.generation_digest() == expected_generation_digest
        {
            return Ok(Arc::clone(generation));
        }
        let previous_generation = match self.sender.borrow().get(key) {
            Some(RuntimeQueryGenerationState::Ready(generation)) => Some(Arc::clone(generation)),
            _ => None,
        };
        match RuntimeQueryGeneration::open(pointer_path, project_root).await {
            Ok(generation) if generation.generation_digest() == expected_generation_digest => {
                let generation = Arc::new(generation);
                self.publish_ready(key.clone(), Arc::clone(&generation))?;
                if let Err(error) = self.builder.schedule(
                    key,
                    project_root,
                    Arc::clone(&generation),
                    previous_generation.as_deref(),
                ) {
                    generation.resident().fail_derived_attachments(&error);
                }
                Ok(generation)
            }
            Ok(generation) => {
                let error = format!(
                    "generation digest mismatch: expected={} actual={}",
                    expected_generation_digest,
                    generation.generation_digest()
                );
                self.publish_failed(
                    key.clone(),
                    generation.generation_token(),
                    expected_generation_digest.to_owned(),
                    error.clone(),
                );
                Err(error)
            }
            Err(error) => {
                self.publish_failed(
                    key.clone(),
                    0,
                    expected_generation_digest.to_owned(),
                    error.clone(),
                );
                Err(error)
            }
        }
    }

    pub async fn ensure_ready_resident(
        &self,
        key: &RuntimeProjectWorkspaceKey,
        project_root: &std::path::Path,
        resident: agent_semantic_client_db::runtime_resident_read::RuntimeResidentReadClient,
        expected_generation_digest: &str,
    ) -> Result<Arc<RuntimeQueryGeneration>, String> {
        if let Some(RuntimeQueryGenerationState::Ready(generation)) = self.sender.borrow().get(key)
            && generation.generation_digest() == expected_generation_digest
        {
            return Ok(Arc::clone(generation));
        }
        let lane = {
            let mut lanes = self.open_lanes.lock().await;
            Arc::clone(
                lanes
                    .entry(key.clone())
                    .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(()))),
            )
        };
        let _guard = lane.lock().await;
        if let Some(RuntimeQueryGenerationState::Ready(generation)) = self.sender.borrow().get(key)
            && generation.generation_digest() == expected_generation_digest
        {
            return Ok(Arc::clone(generation));
        }
        let previous_generation = match self.sender.borrow().get(key) {
            Some(RuntimeQueryGenerationState::Ready(generation)) => Some(Arc::clone(generation)),
            _ => None,
        };
        let generation = Arc::new(RuntimeQueryGeneration::from_resident(resident)?);
        if generation.generation_digest() != expected_generation_digest {
            return Err(format!(
                "resident generation digest mismatch: expected={} actual={}",
                expected_generation_digest,
                generation.generation_digest()
            ));
        }
        self.publish_ready(key.clone(), Arc::clone(&generation))?;
        if let Err(error) = self.builder.schedule(
            key,
            project_root,
            Arc::clone(&generation),
            previous_generation.as_deref(),
        ) {
            generation.resident().fail_derived_attachments(&error);
        }
        Ok(generation)
    }
}
