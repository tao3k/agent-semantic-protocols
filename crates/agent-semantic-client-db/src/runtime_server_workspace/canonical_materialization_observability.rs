//! Performance accounting for canonical workspace materialization.

use super::WorkspaceCanonicalMaterialization;

#[derive(Clone, Copy)]
struct CanonicalMaterializationMetrics {
    owner_count: u64,
    selector_count: u64,
    relation_count: u64,
    source_bytes: u64,
    projection_bytes: u64,
}

fn canonical_materialization_metrics(
    materialization: &WorkspaceCanonicalMaterialization,
) -> CanonicalMaterializationMetrics {
    CanonicalMaterializationMetrics {
        owner_count: materialization.owners.len() as u64,
        selector_count: materialization
            .owners
            .iter()
            .map(|owner| owner.selectors.len() as u64)
            .sum(),
        relation_count: materialization.relations.len() as u64,
        source_bytes: materialization
            .owners
            .iter()
            .map(|owner| owner.bytes.len() as u64)
            .sum(),
        projection_bytes: materialization
            .owners
            .iter()
            .flat_map(|owner| &owner.selectors)
            .flat_map(|selector| &selector.derived_projections)
            .map(|projection| projection.bytes.len() as u64)
            .sum(),
    }
}

pub(super) fn prepare_canonical_index(
    materialization: &WorkspaceCanonicalMaterialization,
) -> (
    std::sync::Arc<crate::runtime_server_workspace::memory_backend::WorkspaceMemoryIndex>,
    u64,
) {
    let started = std::time::Instant::now();
    let index = crate::runtime_server_workspace::WorkspaceMemoryBackend::prepare_index(
        &materialization.owners,
        &materialization.relations,
    );
    (index, elapsed_micros(started))
}

fn elapsed_micros(started: std::time::Instant) -> u64 {
    started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64
}

pub(super) fn record_canonical_materialization_observations(
    materialization: &WorkspaceCanonicalMaterialization,
    prepare_index_elapsed_micros: u64,
) {
    const OBSERVATION_BUDGET_MICROS: u64 = 800_000;
    let observation_started = std::time::Instant::now();
    let metrics = canonical_materialization_metrics(materialization);
    record_canonical_materialization_observation(
        materialization,
        "prepare-index",
        prepare_index_elapsed_micros,
        OBSERVATION_BUDGET_MICROS,
        metrics,
    );
    record_canonical_materialization_observation(
        materialization,
        "payload-accounting",
        elapsed_micros(observation_started),
        OBSERVATION_BUDGET_MICROS,
        metrics,
    );
}

fn record_canonical_materialization_observation(
    materialization: &WorkspaceCanonicalMaterialization,
    operation: &str,
    elapsed_micros: u64,
    budget_micros: u64,
    metrics: CanonicalMaterializationMetrics,
) {
    let status = if elapsed_micros < budget_micros {
        "within-budget"
    } else {
        "budget-exceeded"
    };
    let mut observation = crate::runtime_server_opentelemetry::RuntimePerformanceObservation::new(
        "workspace-canonical-materialization",
        operation,
        elapsed_micros,
        budget_micros,
        status,
    )
    .with_materialization_metrics(
        metrics.owner_count,
        metrics.selector_count,
        metrics.relation_count,
        metrics.source_bytes,
        metrics.projection_bytes,
    );
    observation.workspace_identity = Some(materialization.workspace_identity.to_string());
    let _ = crate::runtime_server_opentelemetry::try_record_to_active_runtime(observation);
}
