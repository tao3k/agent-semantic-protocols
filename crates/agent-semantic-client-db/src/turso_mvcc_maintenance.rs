//! Observable maintenance and recovery receipts for the Turso 0.7 MVCC profile.

use std::path::Path;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::turso_mvcc_store::TursoMvccStore;

pub const TURSO_MVCC_MAINTENANCE_RECEIPT_SCHEMA_ID: &str = "asp.turso-mvcc-maintenance-receipt.v1";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TursoMvccMaintenanceReceipt {
    pub schema_id: String,
    pub backend: String,
    pub backend_version: String,
    pub passive_checkpoint: bool,
    pub cache_flush_count: usize,
    pub database_bytes: u64,
    pub wal_bytes: u64,
    pub shared_memory_bytes: u64,
    pub total_file_bytes: u64,
    pub elapsed_micros: u64,
    pub checkpoint_counter_observable: bool,
}

impl TursoMvccStore {
    /// Flushes dirty connection caches and records durable file-size evidence.
    ///
    /// Turso 0.7 exposes `cacheflush`, while the experimental passive-checkpoint
    /// profile does not expose a public completed-checkpoint counter. The receipt
    /// keeps that observability gap explicit instead of fabricating a count.
    pub async fn flush_and_measure(&self) -> Result<TursoMvccMaintenanceReceipt, String> {
        let started = Instant::now();
        let mut cache_flush_count = 0_usize;
        for lane in &self.inner.lanes {
            let connection = lane.lock().await;
            connection.cacheflush().map_err(|error| error.to_string())?;
            cache_flush_count += 1;
        }

        let database_bytes = file_bytes(&self.inner.path);
        let wal_bytes = suffixed_file_bytes(&self.inner.path, "-wal")
            .saturating_add(suffixed_file_bytes(&self.inner.path, ".wal"));
        let shared_memory_bytes = suffixed_file_bytes(&self.inner.path, "-shm")
            .saturating_add(suffixed_file_bytes(&self.inner.path, ".shm"));
        let total_file_bytes = database_bytes
            .saturating_add(wal_bytes)
            .saturating_add(shared_memory_bytes);

        Ok(TursoMvccMaintenanceReceipt {
            schema_id: TURSO_MVCC_MAINTENANCE_RECEIPT_SCHEMA_ID.to_owned(),
            backend: "turso".to_owned(),
            backend_version: "0.7.0".to_owned(),
            passive_checkpoint: self.inner.passive_checkpoint,
            cache_flush_count,
            database_bytes,
            wal_bytes,
            shared_memory_bytes,
            total_file_bytes,
            elapsed_micros: started.elapsed().as_micros() as u64,
            checkpoint_counter_observable: false,
        })
    }
}

fn file_bytes(path: &Path) -> u64 {
    std::fs::metadata(path)
        .map(|metadata| metadata.len())
        .unwrap_or(0)
}

fn suffixed_file_bytes(path: &Path, suffix: &str) -> u64 {
    let candidate = std::path::PathBuf::from(format!("{}{suffix}", path.display()));
    file_bytes(&candidate)
}
