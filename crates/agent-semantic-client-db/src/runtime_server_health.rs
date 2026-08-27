//! Typed cached-health service for the resident Runtime Server.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use serde::Serialize;

use crate::runtime_server_control::{
    RuntimeServerControlReceipt, RuntimeServerState, read_runtime_server_cached_health_status,
};

const CACHED_HEALTH_SCHEMA_ID: &str = "agent.semantic-protocols.runtime-server-cached-health";
const SCHEMA_VERSION: &str = "1";
static REQUEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// Coherent resident health projected from the stable status-memory authority.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeServerCachedHealth {
    pub schema_id: &'static str,
    pub schema_version: &'static str,
    pub elapsed_micros: u64,
    pub resident: RuntimeServerControlReceipt,
}

impl RuntimeServerCachedHealth {
    /// Return whether the resident snapshot reports the healthy state.
    pub fn is_healthy(&self) -> bool {
        self.resident.state == RuntimeServerState::Healthy
    }
}

/// Read cached health from the exact generation-bound endpoint publication.
pub async fn cached_runtime_server_health(
    endpoint: &crate::runtime_server_control::RuntimeServerEndpoint,
) -> Result<RuntimeServerCachedHealth, String> {
    endpoint.validate()?;
    cached_runtime_server_health_from_status_memory(Path::new(&endpoint.status_memory_path)).await
}

pub async fn cached_runtime_server_health_for_state_home(
    state_home: &Path,
) -> Result<RuntimeServerCachedHealth, String> {
    let endpoint =
        crate::runtime_server_control::read_runtime_server_supervisor_endpoint(state_home)
            .await?
            .ok_or_else(|| "Runtime Server endpoint is unavailable".to_owned())?;
    cached_runtime_server_health(&endpoint).await
}

async fn cached_runtime_server_health_from_status_memory(
    status_memory_path: &Path,
) -> Result<RuntimeServerCachedHealth, String> {
    let started = Instant::now();
    let request_id = format!(
        "cached-health-{}",
        REQUEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    let resident = read_runtime_server_cached_health_status(status_memory_path, request_id).await?;
    Ok(RuntimeServerCachedHealth {
        schema_id: CACHED_HEALTH_SCHEMA_ID,
        schema_version: SCHEMA_VERSION,
        elapsed_micros: u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX),
        resident,
    })
}
