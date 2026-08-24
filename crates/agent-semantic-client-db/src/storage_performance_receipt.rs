//! Machine-readable latency and resource receipts for fixed storage scenarios.

use serde::{Deserialize, Serialize};

use crate::storage_contract::StorageSloMatrixReceiptSchemaId;

/// Schema identifier for fixed-scenario storage SLO evidence.
pub const STORAGE_SLO_MATRIX_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.client-db.storage-slo-matrix-receipt.v1";

/// Exact persisted byte count recorded by a storage receipt.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct StorageByteCount(u64);

impl StorageByteCount {
    /// Return the byte count.
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

impl From<u64> for StorageByteCount {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

/// Resident-set size measured in kibibytes.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct StorageKibibyteCount(u64);

impl StorageKibibyteCount {
    /// Return the kibibyte count.
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

impl From<u64> for StorageKibibyteCount {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// Sorted latency distribution measured in microseconds.
pub struct StorageLatencyDistributionMicros {
    pub sample_count: usize,
    pub p50: u64,
    pub p95: u64,
    pub p99: u64,
    pub max: u64,
}

impl StorageLatencyDistributionMicros {
    /// Build a percentile distribution from non-empty latency samples.
    pub fn from_samples(samples: &[u64]) -> Option<Self> {
        if samples.is_empty() {
            return None;
        }
        let mut sorted = samples.to_vec();
        sorted.sort_unstable();
        Some(Self {
            sample_count: sorted.len(),
            p50: percentile(&sorted, 50),
            p95: percentile(&sorted, 95),
            p99: percentile(&sorted, 99),
            max: *sorted.last().expect("non-empty latency samples"),
        })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// Combined latency, recovery, and footprint evidence for one storage SLO run.
pub struct StorageSloMatrixReceipt {
    schema_id: StorageSloMatrixReceiptSchemaId,
    long_ingestion_rows: usize,
    long_ingestion_batch_rows: usize,
    long_ingestion_latency_micros: StorageLatencyDistributionMicros,
    recovered_rows: usize,
    mixed_pressure_iterations: usize,
    mixed_pressure_latency_micros: StorageLatencyDistributionMicros,
    resident_set_kib: StorageKibibyteCount,
    database_bytes: StorageByteCount,
    wal_bytes: StorageByteCount,
    shm_bytes: StorageByteCount,
    passive_checkpoint: bool,
}

/// Long-ingestion scenario inputs and terminal latency evidence.
pub struct StorageLongIngestionReceipt {
    pub rows: usize,
    pub batch_rows: usize,
    pub latency_micros: StorageLatencyDistributionMicros,
    pub recovered_rows: usize,
}

/// Mixed read/write pressure scenario evidence.
pub struct StorageMixedPressureReceipt {
    pub iterations: usize,
    pub latency_micros: StorageLatencyDistributionMicros,
}

/// Typed resident and durable storage footprint evidence.
pub struct StorageFootprintReceipt {
    pub resident_set_kib: StorageKibibyteCount,
    pub database_bytes: StorageByteCount,
    pub wal_bytes: StorageByteCount,
    pub shm_bytes: StorageByteCount,
    pub passive_checkpoint: bool,
}

impl StorageSloMatrixReceipt {
    /// Compose the fixed storage SLO matrix from named scenario receipts.
    pub fn new(
        schema_id: StorageSloMatrixReceiptSchemaId,
        long_ingestion: StorageLongIngestionReceipt,
        mixed_pressure: StorageMixedPressureReceipt,
        footprint: StorageFootprintReceipt,
    ) -> Self {
        Self {
            schema_id,
            long_ingestion_rows: long_ingestion.rows,
            long_ingestion_batch_rows: long_ingestion.batch_rows,
            long_ingestion_latency_micros: long_ingestion.latency_micros,
            recovered_rows: long_ingestion.recovered_rows,
            mixed_pressure_iterations: mixed_pressure.iterations,
            mixed_pressure_latency_micros: mixed_pressure.latency_micros,
            resident_set_kib: footprint.resident_set_kib,
            database_bytes: footprint.database_bytes,
            wal_bytes: footprint.wal_bytes,
            shm_bytes: footprint.shm_bytes,
            passive_checkpoint: footprint.passive_checkpoint,
        }
    }
}

fn percentile(sorted: &[u64], percentile: usize) -> u64 {
    let index = ((sorted.len() - 1) * percentile).div_ceil(100);
    sorted[index]
}
