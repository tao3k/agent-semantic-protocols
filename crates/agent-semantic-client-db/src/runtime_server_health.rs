//! Typed cached-health service for the resident Runtime Server.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use serde::Serialize;

use crate::runtime_server_control::{
    RuntimeServerControlReceipt, RuntimeServerState, read_runtime_server_cached_health_status,
    runtime_server_runtime_base,
};
use crate::runtime_server_runtime::RuntimeServerClientExecutor;

const CACHED_HEALTH_SCHEMA_ID: &str = "agent.semantic-protocols.runtime-server-cached-health";
const SCHEMA_VERSION: &str = "1";
const STATUS_MEMORY_FILE: &str = "status.v1.memory";
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

/// Read cached health from the canonical Global Runtime Server status memory.
pub fn cached_runtime_server_health() -> Result<RuntimeServerCachedHealth, String> {
    let runtime_base = runtime_server_runtime_base();
    RuntimeServerClientExecutor::get()?.block_on(cached_runtime_server_health_at(&runtime_base))
}

/// Read cached health from an explicitly selected isolated Runtime Server base.
pub async fn cached_runtime_server_health_at(
    runtime_base: &Path,
) -> Result<RuntimeServerCachedHealth, String> {
    let started = Instant::now();
    let request_id = format!(
        "cached-health-{}",
        REQUEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    let resident = read_runtime_server_cached_health_status(
        &runtime_base.join(STATUS_MEMORY_FILE),
        request_id,
    )
    .await?;
    Ok(RuntimeServerCachedHealth {
        schema_id: CACHED_HEALTH_SCHEMA_ID,
        schema_version: SCHEMA_VERSION,
        elapsed_micros: u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX),
        resident,
    })
}
