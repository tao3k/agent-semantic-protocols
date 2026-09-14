// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use crate::{
    RuntimeObservationSink, RuntimePerformanceObservation, register_runtime_observation_sink,
    try_record_to_active_runtime,
};

struct CountingSink(Arc<AtomicU64>);

impl RuntimeObservationSink for CountingSink {
    fn try_record(&self, _observation: RuntimePerformanceObservation) -> bool {
        self.0.fetch_add(1, Ordering::Relaxed);
        true
    }
}

fn observation() -> RuntimePerformanceObservation {
    RuntimePerformanceObservation::new("test", "sink", 0, 1, "within-budget")
}

#[test]
fn stale_registration_cannot_revoke_the_current_runtime_sink() {
    let stale_count = Arc::new(AtomicU64::new(0));
    let stale = register_runtime_observation_sink(Arc::new(CountingSink(Arc::clone(&stale_count))))
        .expect("register stale sink");

    let current_count = Arc::new(AtomicU64::new(0));
    let current =
        register_runtime_observation_sink(Arc::new(CountingSink(Arc::clone(&current_count))))
            .expect("register current sink");

    stale.unregister();
    assert!(try_record_to_active_runtime(observation()));
    assert_eq!(stale_count.load(Ordering::Relaxed), 0);
    assert_eq!(current_count.load(Ordering::Relaxed), 1);

    current.unregister();
    assert!(!try_record_to_active_runtime(observation()));
}

#[test]
fn observation_serialization_preserves_v1_identity() {
    let value = serde_json::to_value(observation()).expect("serialize observation");
    assert_eq!(value["schemaVersion"], "1");
    assert_eq!(
        value["schemaId"],
        "agent.semantic-protocols.runtime-server-performance-observation"
    );
}
