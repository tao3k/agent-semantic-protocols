//! Storage-neutral production adapter for provider execution lifecycle evidence.
//!
//! Runtime identity remains owned by `agent-semantic-runtime`; persistence is
//! injected here, in the higher orchestration crate that already depends on
//! both runtime and client-db.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use agent_semantic_client_db::storage_contract::{
    AgentStorage, SESSION_EVENT_BATCH_SCHEMA_ID, SessionEvent, SessionEventBatch,
    SessionEventBatchWriteReceipt, StorageOptimizationProfile, StoragePartitionKey,
    StorageRetryPolicy, StorageTransactionMode,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use agent_semantic_client_db::ClientDbEngine;
use agent_semantic_client_db::turso_agent_storage::TursoMvccAgentStorage;
use agent_semantic_client_db::turso_mvcc_store::TursoMvccStoreConfig;

const PROVIDER_RUNTIME_EVENT_DB_FILE: &str = "agent-provider-runtime-events.turso";
static INVOCATION_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderRuntimeClientId(String);

impl ProviderRuntimeClientId {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl From<String> for ProviderRuntimeClientId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for ProviderRuntimeClientId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderRuntimeSessionId(String);

impl ProviderRuntimeSessionId {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl From<String> for ProviderRuntimeSessionId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for ProviderRuntimeSessionId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderRuntimeRootSessionId(String);

impl ProviderRuntimeRootSessionId {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl From<String> for ProviderRuntimeRootSessionId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for ProviderRuntimeRootSessionId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderRuntimeStorageContext {
    pub repo_id: String,
    pub workspace_id: String,
    pub scope_id: String,
    pub session_id: String,
    pub root_session_id: String,
    pub agent_id: String,
    pub invocation_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderExecutionStorageEvent {
    pub phase: String,
    pub provider_method: String,
    pub language_id: String,
    pub status_code: i32,
    pub stdout_digest: String,
    pub stderr_digest: String,
    pub receipt_present: bool,
    pub root_session_id: String,
}

impl ProviderExecutionStorageEvent {
    pub fn from_output(
        phase: impl Into<String>,
        provider_method: impl Into<String>,
        language_id: impl Into<String>,
        status_code: i32,
        stdout: &[u8],
        stderr: &[u8],
        receipt_present: bool,
        root_session_id: impl Into<String>,
    ) -> Self {
        Self {
            phase: phase.into(),
            provider_method: provider_method.into(),
            language_id: language_id.into(),
            status_code,
            stdout_digest: digest_bytes(stdout),
            stderr_digest: digest_bytes(stderr),
            receipt_present,
            root_session_id: root_session_id.into(),
        }
    }
}

#[derive(Clone)]
pub struct ProviderRuntimeStorageAdapter {
    storage: Arc<dyn AgentStorage>,
    optimization_profile: StorageOptimizationProfile,
    transaction_mode: StorageTransactionMode,
    retry_policy: StorageRetryPolicy,
}

#[derive(Clone)]
pub struct ProviderRuntimeStorageBinding {
    pub adapter: ProviderRuntimeStorageAdapter,
    pub context: ProviderRuntimeStorageContext,
    event_db_path: std::path::PathBuf,
}

impl ProviderRuntimeStorageBinding {
    /// Resolves the production storage binding for the current agent session.
    /// Manual CLI use without one unambiguous agent session remains supported
    /// and explicitly skips lifecycle persistence.
    pub fn from_current_runtime(project_root: impl AsRef<Path>) -> Result<Option<Self>, String> {
        let Some(runtime_session) = agent_semantic_runtime::current_agent_runtime_session() else {
            return Ok(None);
        };
        let registration =
            agent_semantic_runtime::agent_session_registration_identity((None, None).into())?;
        let engine = ClientDbEngine::resolve(project_root)?;
        let event_db_path = engine
            .db_path()
            .with_file_name(PROVIDER_RUNTIME_EVENT_DB_FILE);
        let mut config = TursoMvccStoreConfig::new(event_db_path.clone());
        config.connection_lanes = 4;
        config.passive_checkpoint = true;
        config.busy_timeout_ms = 250;
        config.retry_attempts = 8;
        config.max_batch_rows = 1_024;
        let storage = agent_semantic_runtime::runtime_block_on_current_thread(
            TursoMvccAgentStorage::open(config),
        )?
        .map_err(|error| format!("{:?}: {}", error.code, error.message))?;
        let adapter = ProviderRuntimeStorageAdapter::new(
            Arc::new(storage),
            StorageOptimizationProfile::MvccConcurrentPassiveCheckpoint,
        )?;
        let invocation_id = next_invocation_id(
            &registration.session_id,
            &runtime_session.client,
            current_time_millis(),
        );
        Ok(Some(Self {
            adapter,
            event_db_path,
            context: ProviderRuntimeStorageContext {
                repo_id: engine.repo_id().to_owned(),
                workspace_id: engine.workspace_id().to_owned(),
                scope_id: engine.scope_id().to_owned(),
                session_id: registration.session_id,
                root_session_id: registration.root_session_id,
                agent_id: runtime_session.client,
                invocation_id,
            },
        }))
    }

    /// Explicit identity constructor used by higher orchestration tests and
    /// embedders that already resolved runtime identity without environment
    /// probing.
    pub fn from_runtime_identity(
        project_root: impl AsRef<Path>,
        client: impl Into<ProviderRuntimeClientId>,
        session_id: impl Into<ProviderRuntimeSessionId>,
        root_session_id: impl Into<ProviderRuntimeRootSessionId>,
    ) -> Result<Self, String> {
        let client = client.into();
        let session_id = session_id.into();
        let root_session_id = root_session_id.into();
        let engine = ClientDbEngine::resolve(project_root)?;
        let event_db_path = engine
            .db_path()
            .with_file_name(PROVIDER_RUNTIME_EVENT_DB_FILE);
        let mut config = TursoMvccStoreConfig::new(event_db_path.clone());
        config.connection_lanes = 4;
        config.passive_checkpoint = true;
        config.busy_timeout_ms = 250;
        config.retry_attempts = 8;
        config.max_batch_rows = 1_024;
        let storage = agent_semantic_runtime::runtime_block_on_current_thread(
            TursoMvccAgentStorage::open(config),
        )?
        .map_err(|error| format!("{:?}: {}", error.code, error.message))?;
        let adapter = ProviderRuntimeStorageAdapter::new(
            Arc::new(storage),
            StorageOptimizationProfile::MvccConcurrentPassiveCheckpoint,
        )?;
        let invocation_id =
            next_invocation_id(session_id.as_str(), client.as_str(), current_time_millis());
        Ok(Self {
            adapter,
            event_db_path,
            context: ProviderRuntimeStorageContext {
                repo_id: engine.repo_id().to_owned(),
                workspace_id: engine.workspace_id().to_owned(),
                scope_id: engine.scope_id().to_owned(),
                session_id: session_id.into_string(),
                root_session_id: root_session_id.into_string(),
                agent_id: client.into_string(),
                invocation_id,
            },
        })
    }

    pub fn event_db_path(&self) -> &Path {
        &self.event_db_path
    }

    pub fn record_invocation_start(
        &self,
        provider_method: &str,
        language_id: &str,
    ) -> Result<SessionEventBatchWriteReceipt, String> {
        let created_at_ms = current_time_millis();
        let event = ProviderExecutionStorageEvent::from_output(
            "invocation-start",
            provider_method,
            language_id,
            0,
            &[],
            &[],
            false,
            self.context.root_session_id.clone(),
        );
        self.adapter
            .append_provider_execution(&self.context, &event, created_at_ms)
    }
}

impl ProviderRuntimeStorageAdapter {
    pub fn new(
        storage: Arc<dyn AgentStorage>,
        optimization_profile: StorageOptimizationProfile,
    ) -> Result<Self, String> {
        let transaction_mode = match optimization_profile {
            StorageOptimizationProfile::CompatibilityImmediate => StorageTransactionMode::Immediate,
            StorageOptimizationProfile::MvccConcurrent
            | StorageOptimizationProfile::MvccConcurrentPassiveCheckpoint => {
                StorageTransactionMode::Concurrent
            }
        };
        Ok(Self {
            storage,
            optimization_profile,
            transaction_mode,
            retry_policy: StorageRetryPolicy::default(),
        })
    }

    pub fn with_retry_policy(mut self, retry_policy: StorageRetryPolicy) -> Result<Self, String> {
        retry_policy
            .validate()
            .map_err(|error| format!("{:?}: {}", error.code, error.message))?;
        self.retry_policy = retry_policy;
        Ok(self)
    }

    pub async fn append_provider_execution_async(
        &self,
        context: &ProviderRuntimeStorageContext,
        event: &ProviderExecutionStorageEvent,
        created_at_ms: i64,
    ) -> Result<SessionEventBatchWriteReceipt, String> {
        let event_payload = serde_json::to_vec(event)
            .map_err(|error| format!("serialize provider execution event: {error}"))?;
        let identity_material = serde_json::to_vec(&(context, event, created_at_ms))
            .map_err(|error| format!("serialize provider execution identity: {error}"))?;
        let identity_digest = digest_bytes(&identity_material);
        let suffix = identity_digest
            .strip_prefix("sha256:")
            .unwrap_or(identity_digest.as_str());
        let event_id = format!("provider-execution-{}", &suffix[..24]);
        let batch_id = format!("provider-batch-{}", &suffix[..24]);
        let batch = SessionEventBatch {
            schema_id: SESSION_EVENT_BATCH_SCHEMA_ID.to_owned(),
            batch_id,
            partition: StoragePartitionKey {
                repo_id: context.repo_id.clone(),
                workspace_id: context.workspace_id.clone(),
                scope_id: context.scope_id.clone(),
                session_id: context.session_id.clone(),
                agent_id: context.agent_id.clone(),
            },
            optimization_profile: self.optimization_profile,
            transaction_mode: self.transaction_mode,
            retry_policy: self.retry_policy.clone(),
            events: vec![SessionEvent {
                event_id,
                turn_id: context.invocation_id.clone(),
                event_kind: format!("provider-execution:{}", event.phase),
                payload: event_payload,
                created_at_ms,
            }],
        };
        self.storage
            .append_session_events_atomically(&batch)
            .await
            .map_err(|error| format!("{:?}: {}", error.code, error.message))
    }

    pub fn append_provider_execution(
        &self,
        context: &ProviderRuntimeStorageContext,
        event: &ProviderExecutionStorageEvent,
        created_at_ms: i64,
    ) -> Result<SessionEventBatchWriteReceipt, String> {
        agent_semantic_runtime::runtime_block_on_current_thread(
            self.append_provider_execution_async(context, event, created_at_ms),
        )?
    }
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn current_time_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}

fn next_invocation_id(session_id: &str, client: &str, created_at_ms: i64) -> String {
    let sequence = INVOCATION_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let digest =
        digest_bytes(format!("{session_id}\0{client}\0{created_at_ms}\0{sequence}").as_bytes());
    format!("provider-invocation-{}", &digest[7..31])
}
