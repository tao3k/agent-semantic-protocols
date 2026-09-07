// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Adaptive, bounded resource calibration for Runtime-owned Search generation builds.

#[derive(Clone, Copy, Debug)]
pub(super) struct RuntimeSearchGenerationBuildResourceInput {
    pub(super) effective_cpu: usize,
    pub(super) server_worker_ceiling: usize,
    pub(super) process_memory_budget_bytes: usize,
    pub(super) blocking_lane_pressure: usize,
    pub(super) lexical_bytes: usize,
    pub(super) owner_count: usize,
    pub(super) changed_owner_count: usize,
}

#[derive(
    Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
pub(super) struct RuntimeSearchGenerationWorkloadKey {
    pub(super) strategy: u8,
    lexical_bytes_bucket: u32,
    owner_count_bucket: u32,
    changed_ratio_bucket: u8,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct RuntimeSearchCalibrationDecision {
    pub(super) strategy: String,
    pub(super) workers: usize,
    pub(super) memory_budget_bytes: usize,
    pub(super) observed_owners_per_second: u64,
    pub(super) sample_identity: String,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct RuntimeSearchCalibrationKey {
    engine_digest: String,
    pub(super) effective_cpu: usize,
    pub(super) process_memory_budget_bytes: usize,
    workload: RuntimeSearchGenerationWorkloadKey,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct RuntimeSearchCalibrationEntry {
    key: RuntimeSearchCalibrationKey,
    decision: RuntimeSearchCalibrationDecision,
}

#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct RuntimeSearchCalibrationStore {
    pub(super) schema_id: String,
    pub(super) schema_version: String,
    pub(super) entries: Vec<RuntimeSearchCalibrationEntry>,
}

const RUNTIME_SEARCH_CALIBRATION_ENTRY_LIMIT: usize = 256;

pub(super) fn runtime_search_calibration_key(
    engine_digest: &str,
    effective_cpu: usize,
    memory_budget_bytes: usize,
    workload: RuntimeSearchGenerationWorkloadKey,
) -> RuntimeSearchCalibrationKey {
    RuntimeSearchCalibrationKey {
        engine_digest: engine_digest.to_owned(),
        effective_cpu,
        process_memory_budget_bytes: memory_budget_bytes,
        workload,
    }
}

pub(super) fn load_runtime_search_calibration_store(
    path: Option<&std::path::Path>,
) -> RuntimeSearchCalibrationStore {
    let Some(path) = path else {
        return RuntimeSearchCalibrationStore::default();
    };
    let Ok(bytes) = std::fs::read(path) else {
        return RuntimeSearchCalibrationStore::default();
    };
    let Ok(store) = serde_json::from_slice::<RuntimeSearchCalibrationStore>(&bytes) else {
        return RuntimeSearchCalibrationStore::default();
    };
    if store.schema_id != "agent.semantic-protocols.runtime-search-calibration-store"
        || store.schema_version != "1"
    {
        return RuntimeSearchCalibrationStore::default();
    }
    store
}

pub(super) fn persist_runtime_search_calibration_store(
    path: &std::path::Path,
    store: &RuntimeSearchCalibrationStore,
) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Runtime search calibration store has no parent".to_owned())?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("create Runtime search calibration store: {error}"))?;
    let bytes = serde_json::to_vec(store)
        .map_err(|error| format!("encode Runtime search calibration store: {error}"))?;
    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    std::fs::write(&temporary, bytes)
        .map_err(|error| format!("stage Runtime search calibration store: {error}"))?;
    std::fs::rename(&temporary, path)
        .map_err(|error| format!("publish Runtime search calibration store: {error}"))
}

pub(super) fn workload_bucket(
    lexical_bytes: usize,
    owner_count: usize,
    changed_owner_count: usize,
    single_segment_bulk: bool,
) -> RuntimeSearchGenerationWorkloadKey {
    let ratio_bucket = if owner_count == 0 {
        0
    } else {
        u8::try_from(
            changed_owner_count
                .min(owner_count)
                .saturating_mul(8)
                .div_ceil(owner_count),
        )
        .unwrap_or(8)
    };
    RuntimeSearchGenerationWorkloadKey {
        strategy: u8::from(!single_segment_bulk),
        lexical_bytes_bucket: lexical_bytes.max(1).ilog2(),
        owner_count_bucket: owner_count.max(1).ilog2(),
        changed_ratio_bucket: ratio_bucket,
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSearchGenerationBuildResourceReceipt {
    pub schema_id: &'static str,
    pub schema_version: &'static str,
    pub effective_cpu: usize,
    pub chosen_workers: usize,
    pub memory_budget_bytes: usize,
    pub lexical_bytes: usize,
    pub owner_count: usize,
    pub changed_owner_ratio: f64,
    pub observed_owners_per_second: u64,
    pub strategy: &'static str,
}

pub(super) fn select_runtime_search_build_resources(
    input: RuntimeSearchGenerationBuildResourceInput,
    minimum_memory_per_worker_bytes: usize,
    throughput_by_workers: &std::collections::BTreeMap<usize, u64>,
    strategy: &'static str,
) -> Result<RuntimeSearchGenerationBuildResourceReceipt, String> {
    if input.effective_cpu == 0
        || input.server_worker_ceiling == 0
        || input.process_memory_budget_bytes == 0
        || minimum_memory_per_worker_bytes == 0
    {
        return Err("Runtime search build resource authority is zero".to_owned());
    }
    let cpu_ceiling = input
        .server_worker_ceiling
        .saturating_sub(input.blocking_lane_pressure)
        .max(1);
    let memory_ceiling = input.process_memory_budget_bytes / minimum_memory_per_worker_bytes;
    if memory_ceiling == 0 {
        return Err("Runtime search build memory cannot admit one Tantivy worker".to_owned());
    }
    let capacity_ceiling = cpu_ceiling
        .min(input.server_worker_ceiling)
        .min(memory_ceiling)
        .min(input.changed_owner_count.max(1));
    let changed_owner_ratio = if input.owner_count == 0 {
        0.0
    } else {
        input.changed_owner_count.min(input.owner_count) as f64 / input.owner_count as f64
    };
    let mut best_workers = 1;
    let mut best_throughput = 0;
    let mut observed_regression = false;
    for (&workers, &throughput) in throughput_by_workers.range(1..=capacity_ceiling) {
        if throughput > best_throughput {
            best_workers = workers;
            best_throughput = throughput;
        } else if workers > best_workers {
            observed_regression = true;
            break;
        }
    }
    let chosen_workers = if throughput_by_workers.is_empty() {
        capacity_ceiling.div_ceil(2)
    } else if observed_regression {
        best_workers
    } else {
        best_workers.saturating_mul(2).min(capacity_ceiling)
    };
    let memory_budget_bytes = minimum_memory_per_worker_bytes
        .checked_mul(chosen_workers)
        .ok_or_else(|| "Runtime search build memory envelope overflows".to_owned())?;
    if memory_budget_bytes > input.process_memory_budget_bytes {
        return Err("Runtime search build memory exceeds process authority".to_owned());
    }
    Ok(RuntimeSearchGenerationBuildResourceReceipt {
        schema_id: "agent.semantic-protocols.runtime-search-generation-build-resource-receipt",
        schema_version: "1",
        effective_cpu: input.effective_cpu,
        chosen_workers,
        memory_budget_bytes,
        lexical_bytes: input.lexical_bytes,
        owner_count: input.owner_count,
        changed_owner_ratio,
        observed_owners_per_second: best_throughput,
        strategy,
    })
}

pub(super) fn observed_peak(history: &std::collections::BTreeMap<usize, u64>) -> u64 {
    history.values().copied().max().unwrap_or(0)
}

pub(super) fn select_single_segment_bulk(
    owner_count: usize,
    bulk_history: &std::collections::BTreeMap<usize, u64>,
    parallel_history: &std::collections::BTreeMap<usize, u64>,
) -> bool {
    match (bulk_history.is_empty(), parallel_history.is_empty()) {
        (true, true) => owner_count <= 1,
        (false, true) => true,
        (true, false) => false,
        (false, false) => observed_peak(bulk_history) > observed_peak(parallel_history),
    }
}

pub(super) fn resident_index_minimum_memory_per_worker(
    process_memory_budget_bytes: usize,
) -> Result<usize, String> {
    agent_semantic_search::ResidentIndexBuildResources::new(
        1,
        process_memory_budget_bytes,
        agent_semantic_search::ResidentIndexBuildStrategy::ParallelSegments,
    )?;
    let mut rejected = 0usize;
    let mut accepted = process_memory_budget_bytes;
    while rejected + 1 < accepted {
        let candidate = rejected + (accepted - rejected) / 2;
        if agent_semantic_search::ResidentIndexBuildResources::new(
            1,
            candidate,
            agent_semantic_search::ResidentIndexBuildStrategy::ParallelSegments,
        )
        .is_ok()
        {
            accepted = candidate;
        } else {
            rejected = candidate;
        }
    }
    Ok(accepted)
}

pub(super) fn resident_index_server_worker_ceiling(
    effective_cpu: usize,
    process_memory_budget_bytes: usize,
) -> Result<usize, String> {
    for workers in (1..=effective_cpu).rev() {
        if agent_semantic_search::ResidentIndexBuildResources::new(
            workers,
            process_memory_budget_bytes,
            agent_semantic_search::ResidentIndexBuildStrategy::ParallelSegments,
        )
        .is_ok()
        {
            return Ok(workers);
        }
    }
    Err("Runtime Server resource envelope cannot admit a Tantivy worker".to_owned())
}

pub(super) fn cached_calibration_decisions(
    store: &RuntimeSearchCalibrationStore,
    engine_digest: &str,
    effective_cpu: usize,
    memory_budget_bytes: usize,
    bulk_workload_key: RuntimeSearchGenerationWorkloadKey,
    parallel_workload_key: RuntimeSearchGenerationWorkloadKey,
) -> Vec<RuntimeSearchCalibrationDecision> {
    let bulk = runtime_search_calibration_key(
        engine_digest,
        effective_cpu,
        memory_budget_bytes,
        bulk_workload_key,
    );
    let parallel = runtime_search_calibration_key(
        engine_digest,
        effective_cpu,
        memory_budget_bytes,
        parallel_workload_key,
    );
    store
        .entries
        .iter()
        .filter(|entry| entry.key == bulk || entry.key == parallel)
        .map(|entry| entry.decision.clone())
        .collect()
}

pub(super) fn upsert_runtime_search_calibration_decision(
    store: &mut RuntimeSearchCalibrationStore,
    key: RuntimeSearchCalibrationKey,
    decision: RuntimeSearchCalibrationDecision,
) {
    store.entries.retain(|entry| {
        entry.key != key
            || entry.decision.strategy != decision.strategy
            || entry.decision.workers != decision.workers
            || entry.decision.memory_budget_bytes != decision.memory_budget_bytes
    });
    store
        .entries
        .push(RuntimeSearchCalibrationEntry { key, decision });
    let overflow = store
        .entries
        .len()
        .saturating_sub(RUNTIME_SEARCH_CALIBRATION_ENTRY_LIMIT);
    if overflow > 0 {
        store.entries.drain(..overflow);
    }
}
