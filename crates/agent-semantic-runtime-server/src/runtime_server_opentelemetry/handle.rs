// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::sync::Arc;

// Runtime Server owns the concrete observation ingress handle.

use tokio::sync::{mpsc, oneshot};

use agent_semantic_runtime_observability::{RuntimeLifecycleEvent, RuntimePerformanceObservation};

#[derive(Clone, Debug)]
pub struct RuntimeServerOpenTelemetryHandle {
    pub(super) sender: mpsc::Sender<RuntimeTelemetryLaneMessage>,
    pub(super) dropped_observations: Arc<std::sync::atomic::AtomicU64>,
}

#[derive(Debug)]
#[expect(
    clippy::large_enum_variant,
    reason = "observations stay inline because heap allocation belongs outside the sub-millisecond telemetry admission path; fences are rare control messages"
)]
pub(super) enum RuntimeTelemetryLaneMessage {
    Observation(RuntimePerformanceObservation),
    Fence(oneshot::Sender<()>),
}

impl RuntimeServerOpenTelemetryHandle {
    pub fn try_record_lifecycle(&self, event: RuntimeLifecycleEvent) -> bool {
        self.try_record(event.into_observation())
    }

    pub fn try_record(&self, observation: RuntimePerformanceObservation) -> bool {
        match self
            .sender
            .try_send(RuntimeTelemetryLaneMessage::Observation(observation))
        {
            Ok(()) => true,
            Err(_) => {
                self.dropped_observations
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                false
            }
        }
    }

    pub fn dropped_observation_count(&self) -> u64 {
        self.dropped_observations
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Waits for the resident lane to consume every message admitted before
    /// this fence. Unlike a scheduler yield, the acknowledgement establishes
    /// an explicit FIFO happens-before relation with the live query store.
    pub async fn flush(&self) -> Result<(), String> {
        let (acknowledge, acknowledged) = oneshot::channel();
        self.sender
            .send(RuntimeTelemetryLaneMessage::Fence(acknowledge))
            .await
            .map_err(|_| {
                "Runtime Server telemetry lane closed before accepting fence".to_owned()
            })?;
        acknowledged.await.map_err(|_| {
            "Runtime Server telemetry lane closed before acknowledging fence".to_owned()
        })
    }
}

impl agent_semantic_runtime_observability::RuntimeObservationSink
    for RuntimeServerOpenTelemetryHandle
{
    fn try_record(&self, observation: RuntimePerformanceObservation) -> bool {
        RuntimeServerOpenTelemetryHandle::try_record(self, observation)
    }
}
