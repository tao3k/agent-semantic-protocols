//! Machine-readable latency and resource receipts for fixed storage scenarios.

use serde::{Deserialize, Serialize};

pub const STORAGE_SLO_MATRIX_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.client-db.storage-slo-matrix-receipt.v1";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageLatencyDistributionMicros {
    pub sample_count: usize,
    pub p50: u64,
    pub p95: u64,
    pub p99: u64,
    pub max: u64,
}

impl StorageLatencyDistributionMicros {
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
pub struct StorageSloMatrixReceipt {
    schema_id: StorageSloMatrixReceiptSchemaId,
    long_ingestion_rows: usize,
    long_ingestion_batch_rows: usize,
    long_ingestion_latency_micros: StorageLatencyDistributionMicros,
    recovered_rows: usize,
    mixed_pressure_iterations: usize,
    mixed_pressure_latency_micros: StorageLatencyDistributionMicros,
    resident_set_kib: u64,
    database_bytes: u64,
    wal_bytes: u64,
    shm_bytes: u64,
    passive_checkpoint: bool,
}

fn percentile(sorted: &[u64], percentile: usize) -> u64 {
    let index = ((sorted.len() - 1) * percentile).div_ceil(100);
    sorted[index]
}
