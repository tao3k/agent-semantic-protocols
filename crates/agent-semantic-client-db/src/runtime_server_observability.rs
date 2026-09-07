// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use tokio::sync::mpsc;

#[derive(Clone)]
pub struct RuntimeServerEventPublisher {
    sender: mpsc::Sender<RuntimeServerEvent>,
    capacity: usize,
    dropped: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

impl RuntimeServerEventPublisher {
    pub(crate) fn new(sender: mpsc::Sender<RuntimeServerEvent>, capacity: usize) -> Self {
        Self {
            sender,
            capacity,
            dropped: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }

    pub fn dropped(&self) -> u64 {
        self.dropped.load(std::sync::atomic::Ordering::Acquire)
    }

    pub fn send(
        &self,
        event: RuntimeServerEvent,
    ) -> Result<(), mpsc::error::TrySendError<RuntimeServerEvent>> {
        self.sender.try_send(event)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(tag = "kind", content = "detail", rename_all = "kebab-case")]
pub enum RuntimeServerEvent {
    ConnectionRejected(String),
    ConnectionTaskFailed(String),
    WorkspaceGenerationMaterializationObserved {
        workspace_identity: String,
        build_mode: String,
        owner_count: usize,
        owner_source_bytes: u64,
        selector_count: usize,
        relation_count: usize,
    },
    WorkspaceGenerationResidentPublished {
        workspace_identity: String,
        build_mode: String,
        generation_digest: String,
        elapsed_micros: u64,
    },
    WorkspaceGenerationAdmissionFailed {
        workspace_identity: String,
        build_mode: String,
        error: String,
    },
    WorkspaceGenerationRestoreFailed {
        workspace_identity: String,
        error: String,
    },
}

pub(crate) fn publish_event(
    events: Option<&RuntimeServerEventPublisher>,
    event: RuntimeServerEvent,
) {
    if let Some(events) = events {
        if events.sender.try_send(event).is_err() {
            let dropped = events
                .dropped
                .fetch_add(1, std::sync::atomic::Ordering::AcqRel)
                .saturating_add(1);
            let mut observation =
                crate::runtime_server_opentelemetry::RuntimePerformanceObservation::new(
                    "runtime-server-diagnostics",
                    "diagnostic-queue-saturated",
                    0,
                    1,
                    "budget-exceeded",
                );
            observation.runtime_diagnostic_queue_depth = Some(events.capacity as u64);
            observation.runtime_diagnostic_queue_capacity = Some(events.capacity as u64);
            observation.runtime_dropped_diagnostics = Some(dropped);
            let _ = crate::runtime_server_opentelemetry::try_record_to_active_runtime(observation);
        }
    }
}
