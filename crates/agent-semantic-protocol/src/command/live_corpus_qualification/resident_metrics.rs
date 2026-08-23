use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ResidentLatencyDistribution {
    pub(super) sample_count: usize,
    pub(super) min_micros: u64,
    pub(super) p50_micros: u64,
    pub(super) p95_micros: u64,
    pub(super) p99_micros: u64,
    pub(super) max_micros: u64,
}

pub(super) struct ResidentSearchOutcome {
    pub(super) operation_id: String,
    pub(super) resident_read_elapsed_micros: u64,
    pub(super) service_elapsed_micros: u64,
    pub(super) elapsed_micros: u64,
    pub(super) candidate_count: usize,
    pub(super) selectors: Vec<String>,
    pub(super) owner_paths: Vec<String>,
    pub(super) work_counters:
        agent_semantic_client_db::workspace_db_ipc::RuntimeResidentReadWorkCounters,
}

pub(super) fn resident_latency_distribution(
    mut samples: Vec<u64>,
) -> Result<ResidentLatencyDistribution, String> {
    if samples.is_empty() {
        return Err("Live Corpus resident latency distribution is empty".to_owned());
    }
    samples.sort_unstable();
    let percentile = |percent: usize| samples[(samples.len() - 1) * percent / 100];
    Ok(ResidentLatencyDistribution {
        sample_count: samples.len(),
        min_micros: samples[0],
        p50_micros: percentile(50),
        p95_micros: percentile(95),
        p99_micros: percentile(99),
        max_micros: samples[samples.len() - 1],
    })
}

pub(super) fn require_resident_sample_budget(
    case_id: &str,
    operation: &str,
    sample_index: usize,
    elapsed_micros: u64,
    budget_micros: u64,
) -> Result<(), String> {
    if elapsed_micros > budget_micros {
        return Err(format!(
            "Live Corpus resident sample exceeded budget: case={case_id} operation={operation} sampleIndex={sample_index} elapsedMicros={elapsed_micros} budgetMicros={budget_micros}"
        ));
    }
    Ok(())
}

pub(super) fn require_zero_runtime_work(
    case_id: &str,
    operation: &str,
    sample_index: usize,
    counters: &agent_semantic_client_db::workspace_db_ipc::RuntimeResidentReadWorkCounters,
) -> Result<(), String> {
    if counters.database_opens != 0
        || counters.filesystem_reads != 0
        || counters.provider_spawns != 0
        || counters.control_socket_roundtrips != 0
    {
        return Err(format!(
            "Live Corpus warm resident read performed external work: case={case_id} operation={operation} sampleIndex={sample_index} databaseOpens={} filesystemReads={} providerSpawns={} controlSocketRoundtrips={}",
            counters.database_opens,
            counters.filesystem_reads,
            counters.provider_spawns,
            counters.control_socket_roundtrips
        ));
    }
    Ok(())
}
