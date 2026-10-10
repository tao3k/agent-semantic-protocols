// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Resident process-local observation admission and memory-operation registry.

use crate::RuntimePerformanceObservation;

/// Non-blocking process-local admission boundary implemented by the resident
/// Runtime Server. Producers own no channel, exporter, database, or runtime.
pub trait RuntimeObservationSink: Send + Sync + 'static {
    fn try_record(&self, observation: RuntimePerformanceObservation) -> bool;
}

type RuntimeObservationSinkSlot = Option<(u64, std::sync::Arc<dyn RuntimeObservationSink>)>;

static ACTIVE_RUNTIME_OBSERVATION_SINK: std::sync::OnceLock<
    std::sync::Mutex<RuntimeObservationSinkSlot>,
> = std::sync::OnceLock::new();
static NEXT_RUNTIME_OBSERVATION_SINK_ID: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(1);

pub struct RuntimeObservationSinkRegistration {
    registration_id: u64,
    active: bool,
}

pub fn register_runtime_observation_sink(
    sink: std::sync::Arc<dyn RuntimeObservationSink>,
) -> Result<RuntimeObservationSinkRegistration, String> {
    let registration_id =
        NEXT_RUNTIME_OBSERVATION_SINK_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let active = ACTIVE_RUNTIME_OBSERVATION_SINK.get_or_init(|| std::sync::Mutex::new(None));
    *active
        .lock()
        .map_err(|_| "Runtime observation sink registry lock poisoned".to_owned())? =
        Some((registration_id, sink));
    Ok(RuntimeObservationSinkRegistration {
        registration_id,
        active: true,
    })
}

#[must_use]
pub fn try_record_to_active_runtime(observation: RuntimePerformanceObservation) -> bool {
    let Some(active) = ACTIVE_RUNTIME_OBSERVATION_SINK.get() else {
        return false;
    };
    let Ok(active) = active.lock() else {
        return false;
    };
    active
        .as_ref()
        .is_some_and(|(_, sink)| sink.try_record(observation))
}

impl RuntimeObservationSinkRegistration {
    pub fn unregister(mut self) {
        self.clear();
    }

    fn clear(&mut self) {
        if !self.active {
            return;
        }
        self.active = false;
        if let Some(active) = ACTIVE_RUNTIME_OBSERVATION_SINK.get()
            && let Ok(mut active) = active.lock()
            && active
                .as_ref()
                .is_some_and(|(registration_id, _)| *registration_id == self.registration_id)
        {
            *active = None;
        }
    }
}

impl Drop for RuntimeObservationSinkRegistration {
    fn drop(&mut self) {
        self.clear();
    }
}

static ACTIVE_MEMORY_OPERATIONS: std::sync::OnceLock<
    std::sync::Mutex<std::collections::BTreeMap<String, String>>,
> = std::sync::OnceLock::new();

pub struct RuntimeMemoryOperationGuard {
    operation_id: String,
}

impl Drop for RuntimeMemoryOperationGuard {
    fn drop(&mut self) {
        if let Some(active) = ACTIVE_MEMORY_OPERATIONS.get()
            && let Ok(mut active) = active.lock()
        {
            active.remove(&self.operation_id);
        }
    }
}

pub fn begin_runtime_memory_operation(
    workspace_identity: impl Into<String>,
    operation_id: impl Into<String>,
) -> RuntimeMemoryOperationGuard {
    let operation_id = operation_id.into();
    if let Ok(mut active) = ACTIVE_MEMORY_OPERATIONS
        .get_or_init(|| std::sync::Mutex::new(std::collections::BTreeMap::new()))
        .lock()
    {
        active.insert(operation_id.clone(), workspace_identity.into());
    }
    RuntimeMemoryOperationGuard { operation_id }
}

#[must_use]
pub fn active_runtime_memory_operations() -> Vec<(String, String)> {
    ACTIVE_MEMORY_OPERATIONS
        .get()
        .and_then(|active| active.lock().ok())
        .map(|active| {
            active
                .iter()
                .map(|(operation_id, workspace_identity)| {
                    (operation_id.clone(), workspace_identity.clone())
                })
                .collect()
        })
        .unwrap_or_default()
}
