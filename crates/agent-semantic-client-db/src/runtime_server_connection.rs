// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::future::Future;

pub const RUNTIME_SERVER_CONNECTION_IO_BUDGET: std::time::Duration =
    std::time::Duration::from_millis(25);

pub async fn within_connection_io_budget<T>(
    surface: &'static str,
    operation: impl Future<Output = Result<T, String>>,
) -> Result<T, String> {
    tokio::time::timeout(RUNTIME_SERVER_CONNECTION_IO_BUDGET, operation)
        .await
        .map_err(|_| {
            format!(
                "Runtime Server {surface} exceeded the {}ms connection I/O budget",
                RUNTIME_SERVER_CONNECTION_IO_BUDGET.as_millis()
            )
        })?
}

/// Adaptive upper bound for accepted-but-not-reaped resident connections.
pub fn runtime_server_connection_limit(worker_count: usize) -> usize {
    worker_count.saturating_mul(16).clamp(32, 512)
}

#[derive(Clone)]
pub struct RuntimeServerConnectionSupervisor {
    surface: &'static str,
    limit: usize,
    active: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    high_watermark: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    rejected: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeServerConnectionSnapshot {
    pub active: usize,
    pub limit: usize,
    pub high_watermark: usize,
    pub rejected: u64,
}

pub struct RuntimeServerConnectionLease {
    supervisor: RuntimeServerConnectionSupervisor,
}

impl RuntimeServerConnectionSupervisor {
    pub fn for_current_runtime(surface: &'static str) -> Self {
        let workers = tokio::runtime::Handle::current().metrics().num_workers();
        Self::new(surface, runtime_server_connection_limit(workers))
    }

    pub fn new(surface: &'static str, limit: usize) -> Self {
        assert!(
            limit > 0,
            "Runtime Server connection limit must be non-zero"
        );
        Self {
            surface,
            limit,
            active: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            high_watermark: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            rejected: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }

    pub fn has_capacity(&self) -> bool {
        self.active.load(std::sync::atomic::Ordering::Acquire) < self.limit
    }

    pub fn try_admit(&self) -> Option<RuntimeServerConnectionLease> {
        use std::sync::atomic::Ordering;
        let admitted = self
            .active
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |active| {
                (active < self.limit).then_some(active + 1)
            });
        let active = match admitted {
            Ok(previous) => previous + 1,
            Err(_) => {
                self.rejected.fetch_add(1, Ordering::AcqRel);
                self.record_pressure("connection-rejected");
                return None;
            }
        };
        let previous_high = self.high_watermark.fetch_max(active, Ordering::AcqRel);
        if active > previous_high {
            self.record_pressure("connection-high-watermark");
        }
        Some(RuntimeServerConnectionLease {
            supervisor: self.clone(),
        })
    }

    pub fn snapshot(&self) -> RuntimeServerConnectionSnapshot {
        use std::sync::atomic::Ordering;
        RuntimeServerConnectionSnapshot {
            active: self.active.load(Ordering::Acquire),
            limit: self.limit,
            high_watermark: self.high_watermark.load(Ordering::Acquire),
            rejected: self.rejected.load(Ordering::Acquire),
        }
    }

    fn record_pressure(&self, stage: &'static str) {
        let snapshot = self.snapshot();
        let mut observation =
            agent_semantic_runtime_observability::RuntimePerformanceObservation::new(
                self.surface,
                stage,
                0,
                1,
                if snapshot.rejected == 0 {
                    "within-budget"
                } else {
                    "budget-exceeded"
                },
            );
        observation.runtime_active_connections = Some(snapshot.active as u64);
        observation.runtime_connection_limit = Some(snapshot.limit as u64);
        observation.runtime_connection_high_watermark = Some(snapshot.high_watermark as u64);
        observation.runtime_rejected_connections = Some(snapshot.rejected);
        let _ = agent_semantic_runtime_observability::try_record_to_active_runtime(observation);
    }
}

impl Drop for RuntimeServerConnectionLease {
    fn drop(&mut self) {
        self.supervisor
            .active
            .fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
    }
}
