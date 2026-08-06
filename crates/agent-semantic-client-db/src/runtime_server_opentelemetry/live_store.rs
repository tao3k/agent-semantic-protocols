use std::collections::{HashMap, HashSet, VecDeque};

use super::RuntimePerformanceObservation;

const MAX_SAMPLES_PER_STAGE: usize = 4_096;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct PerformanceKey {
    workspace_identity: String,
    surface: String,
    stage: String,
}

#[derive(Default)]
struct PerformanceWindow {
    elapsed_micros: VecDeque<u64>,
    budget_failure_count: u64,
    latest_attributes_json: Option<String>,
    failure_event_identities: HashSet<String>,
    failure_event_order: VecDeque<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct LivePerformanceSummary {
    pub observation_count: u64,
    pub budget_failure_count: u64,
    pub p50_micros: Option<u64>,
    pub p95_micros: Option<u64>,
    pub p99_micros: Option<u64>,
    pub latest_attributes_json: Option<String>,
}

#[derive(Default)]
pub(super) struct RuntimePerformanceLiveStore {
    windows: parking_lot::RwLock<HashMap<PerformanceKey, PerformanceWindow>>,
}

impl RuntimePerformanceLiveStore {
    pub fn record(&self, observation: &RuntimePerformanceObservation) -> bool {
        let Some(workspace_identity) = observation.workspace_identity.as_ref() else {
            // Daemon-global lifecycle, scheduler, memory, and connection
            // observations still belong in OpenTelemetry/Turso. They simply
            // do not participate in the workspace-keyed live percentile view.
            return true;
        };
        let key = PerformanceKey {
            workspace_identity: workspace_identity.clone(),
            surface: observation.surface.clone(),
            stage: observation.stage.clone(),
        };
        let mut windows = self.windows.write();
        let window = windows.entry(key).or_default();
        if observation.budget_status == "budget-exceeded"
            && let Some(identity) = observation.event_identity.as_ref()
        {
            if !window.failure_event_identities.insert(identity.clone()) {
                return false;
            }
            window.failure_event_order.push_back(identity.clone());
            if window.failure_event_order.len() > MAX_SAMPLES_PER_STAGE
                && let Some(expired) = window.failure_event_order.pop_front()
            {
                window.failure_event_identities.remove(&expired);
            }
        }
        if window.elapsed_micros.len() == MAX_SAMPLES_PER_STAGE {
            window.elapsed_micros.pop_front();
        }
        window.elapsed_micros.push_back(observation.elapsed_micros);
        if observation.budget_status == "budget-exceeded" {
            window.budget_failure_count = window.budget_failure_count.saturating_add(1);
        }
        window.latest_attributes_json = Some(attributes_json(observation));
        true
    }

    pub fn summary(
        &self,
        workspace_identity: &str,
        surface: &str,
        stage: &str,
    ) -> LivePerformanceSummary {
        let windows = self.windows.read();
        let Some(window) = windows.get(&PerformanceKey {
            workspace_identity: workspace_identity.to_owned(),
            surface: surface.to_owned(),
            stage: stage.to_owned(),
        }) else {
            return LivePerformanceSummary::default();
        };
        let mut samples = window.elapsed_micros.iter().copied().collect::<Vec<_>>();
        samples.sort_unstable();
        LivePerformanceSummary {
            observation_count: u64::try_from(samples.len()).unwrap_or(u64::MAX),
            budget_failure_count: window.budget_failure_count,
            p50_micros: quantile(&samples, 50),
            p95_micros: quantile(&samples, 95),
            p99_micros: quantile(&samples, 99),
            latest_attributes_json: window.latest_attributes_json.clone(),
        }
    }
}

fn quantile(samples: &[u64], percentile: usize) -> Option<u64> {
    let index = samples
        .len()
        .saturating_mul(percentile)
        .saturating_add(99)
        .saturating_div(100)
        .saturating_sub(1)
        .min(samples.len().saturating_sub(1));
    samples.get(index).copied()
}

fn attributes_json(observation: &RuntimePerformanceObservation) -> String {
    let mut attributes = serde_json::Map::new();
    attributes.insert(
        super::semconv::ELAPSED_MICROS.to_owned(),
        observation.elapsed_micros.into(),
    );
    attributes.insert(
        super::semconv::BUDGET_MICROS.to_owned(),
        observation.budget_micros.into(),
    );
    attributes.insert(
        super::semconv::BUDGET_STATUS.to_owned(),
        observation.budget_status.clone().into(),
    );
    insert_u64(
        &mut attributes,
        super::semconv::PROCESS_RESIDENT_MEMORY,
        observation.process_resident_bytes,
    );
    insert_u64(
        &mut attributes,
        super::semconv::PROCESS_PEAK_RESIDENT_MEMORY,
        observation.process_peak_resident_bytes,
    );
    insert_u64(
        &mut attributes,
        super::semconv::PROCESS_DISK_READ_BYTES,
        observation.process_disk_read_bytes,
    );
    insert_u64(
        &mut attributes,
        super::semconv::PROCESS_DISK_WRITE_BYTES,
        observation.process_disk_write_bytes,
    );
    insert_u64(
        &mut attributes,
        super::semconv::PROCESS_PAGE_INS,
        observation.process_page_ins,
    );
    insert_u64(
        &mut attributes,
        super::semconv::RUNTIME_EVENT_LOOP_LAG,
        observation.runtime_event_loop_lag_micros,
    );
    insert_u64(
        &mut attributes,
        super::semconv::RUNTIME_EVENT_LOOP_LAG_BUDGET,
        observation.runtime_event_loop_lag_budget_micros,
    );
    insert_u64(
        &mut attributes,
        super::semconv::RUNTIME_WORKER_THREADS,
        observation.runtime_worker_threads,
    );
    insert_u64(
        &mut attributes,
        super::semconv::RUNTIME_ALIVE_TASKS,
        observation.runtime_alive_tasks,
    );
    insert_u64(
        &mut attributes,
        super::semconv::RUNTIME_GLOBAL_QUEUE_DEPTH,
        observation.runtime_global_queue_depth,
    );
    if let Some(status) = observation.runtime_event_loop_lag_budget_status.as_ref() {
        attributes.insert(
            super::semconv::RUNTIME_EVENT_LOOP_LAG_BUDGET_STATUS.to_owned(),
            status.clone().into(),
        );
    }
    serde_json::Value::Object(attributes).to_string()
}

fn insert_u64(
    attributes: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    value: Option<u64>,
) {
    if let Some(value) = value {
        attributes.insert(key.to_owned(), value.into());
    }
}
