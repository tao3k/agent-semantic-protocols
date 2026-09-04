use std::os::unix::fs::PermissionsExt;

use agent_semantic_artifacts::runtime_artifact_activation::commit_runtime_artifact_activation;
use agent_semantic_artifacts::runtime_artifact_activation::read_runtime_artifact_activation_event;
use agent_semantic_artifacts::runtime_artifact_publication::publish_runtime_artifact;

const SAMPLE_COUNT: usize = 32;
const PUBLICATION_P99_MICROS: u128 = 1_000_000;
const LOCK_P99_MICROS: u128 = 10_000;

fn percentile_micros(samples: &[u128], percentile: usize) -> u128 {
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    sorted[(sorted.len() - 1) * percentile / 100]
}

fn executable_source(path: &std::path::Path, sample: usize) {
    std::fs::write(path, format!("#!/bin/sh\nexit {sample}\n")).expect("write artifact source");
    let mut permissions = std::fs::metadata(path)
        .expect("artifact metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).expect("mark artifact executable");
}

#[tokio::test]
async fn publication_latency_distribution_is_subsecond_with_millisecond_lock_scope() {
    let mut cold_total = Vec::with_capacity(SAMPLE_COUNT);
    let mut cold_lock = Vec::with_capacity(SAMPLE_COUNT);
    let mut cold_phase = Vec::with_capacity(SAMPLE_COUNT);
    for sample in 0..SAMPLE_COUNT {
        let temporary = tempfile::tempdir().expect("cold publication root");
        let state_home = temporary.path().join("state");
        let source = temporary.path().join("asp");
        let target = temporary.path().join("bin/asp");
        executable_source(&source, sample);

        let started = std::time::Instant::now();
        let receipt = publish_runtime_artifact(&state_home, &source, &target, "dev")
            .await
            .expect("publish cold artifact");
        cold_total.push(started.elapsed().as_micros());
        cold_lock.push(u128::from(receipt.lock_elapsed_micros));
        cold_phase.push(receipt.phase_trace);
    }

    let temporary = tempfile::tempdir().expect("warm publication root");
    let state_home = temporary.path().join("state");
    let target = temporary.path().join("bin/asp");
    let mut warm_total = Vec::with_capacity(SAMPLE_COUNT);
    let mut warm_lock = Vec::with_capacity(SAMPLE_COUNT);
    let mut warm_phase = Vec::with_capacity(SAMPLE_COUNT);
    for sample in 0..SAMPLE_COUNT {
        let source = temporary.path().join(format!("asp-{sample}"));
        executable_source(&source, sample);

        let started = std::time::Instant::now();
        let receipt = publish_runtime_artifact(&state_home, &source, &target, "dev")
            .await
            .expect("publish warm artifact");
        warm_total.push(started.elapsed().as_micros());
        warm_lock.push(u128::from(receipt.lock_elapsed_micros));
        warm_phase.push(receipt.phase_trace);
    }

    let cold_p50 = percentile_micros(&cold_total, 50);
    let cold_p95 = percentile_micros(&cold_total, 95);
    let cold_p99 = percentile_micros(&cold_total, 99);
    let cold_max = *cold_total.iter().max().expect("cold samples");
    let warm_p50 = percentile_micros(&warm_total, 50);
    let warm_p95 = percentile_micros(&warm_total, 95);
    let warm_p99 = percentile_micros(&warm_total, 99);
    let warm_max = *warm_total.iter().max().expect("warm samples");
    let cold_lock_p99 = percentile_micros(&cold_lock, 99);
    let warm_lock_p99 = percentile_micros(&warm_lock, 99);
    let cold_lock_max_index = cold_lock
        .iter()
        .enumerate()
        .max_by_key(|(_, elapsed)| *elapsed)
        .map(|(index, _)| index)
        .expect("cold lock samples");
    let warm_lock_max_index = warm_lock
        .iter()
        .enumerate()
        .max_by_key(|(_, elapsed)| *elapsed)
        .map(|(index, _)| index)
        .expect("warm lock samples");

    eprintln!(
        "runtime-artifact-publication-performance samples={SAMPLE_COUNT} environment=tempdir-local-filesystem coldP50Micros={cold_p50} coldP95Micros={cold_p95} coldP99Micros={cold_p99} coldMaxMicros={cold_max} warmP50Micros={warm_p50} warmP95Micros={warm_p95} warmP99Micros={warm_p99} warmMaxMicros={warm_max} coldLockP99Micros={cold_lock_p99} warmLockP99Micros={warm_lock_p99}"
    );
    eprintln!(
        "runtime-artifact-lock-outliers coldMaxMicros={} coldPhase={:?} warmMaxMicros={} warmPhase={:?}",
        cold_lock[cold_lock_max_index],
        cold_phase[cold_lock_max_index],
        warm_lock[warm_lock_max_index],
        warm_phase[warm_lock_max_index],
    );
    assert!(
        cold_p99 < PUBLICATION_P99_MICROS,
        "cold artifact publication p99 must remain subsecond: p99Micros={cold_p99}"
    );
    assert!(
        warm_p99 < PUBLICATION_P99_MICROS,
        "warm artifact publication p99 must remain subsecond: p99Micros={warm_p99}"
    );
    assert!(
        cold_lock_p99 < LOCK_P99_MICROS,
        "cold artifact mutation-lock p99 must remain below 10ms: p99Micros={cold_lock_p99}"
    );
    assert!(
        warm_lock_p99 < LOCK_P99_MICROS,
        "warm artifact mutation-lock p99 must remain below 10ms: p99Micros={warm_lock_p99}"
    );
}

#[tokio::test]
async fn previous_large_artifact_bytes_are_outside_the_publication_guard_scope() {
    const LARGE_BYTES: usize = 32 * 1024 * 1024;
    let temporary = tempfile::tempdir().expect("large previous artifact fixture");
    let state_home = temporary.path().join("state");
    let large_source = temporary.path().join("asp-large");
    let next_source = temporary.path().join("asp-next");
    let target = temporary.path().join("bin/asp");
    let mut large = vec![b'x'; LARGE_BYTES];
    large[..10].copy_from_slice(b"#!/bin/sh\n");
    std::fs::write(&large_source, large).expect("write large previous artifact");
    let mut permissions = std::fs::metadata(&large_source).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&large_source, permissions).unwrap();
    executable_source(&next_source, 1);

    publish_runtime_artifact(&state_home, &large_source, &target, "release")
        .await
        .expect("publish large previous artifact");
    let initial = read_runtime_artifact_activation_event(&state_home)
        .await
        .unwrap()
        .unwrap();
    commit_runtime_artifact_activation(&state_home, &initial, None)
        .await
        .expect("commit large previous artifact");

    let receipt = publish_runtime_artifact(&state_home, &next_source, &target, "release")
        .await
        .expect("publish after a large active artifact");
    let phase_trace_json =
        serde_json::to_string(&receipt.phase_trace).expect("serialize publication phase trace");
    let phase_trace_path = temporary.path().join("runtime-artifact-phase-trace.json");
    std::fs::write(&phase_trace_path, phase_trace_json.as_bytes())
        .expect("write publication phase trace artifact");
    eprintln!(
        "runtime-artifact-large-previous bytes={LARGE_BYTES} lockMicros={} phaseTracePath={} phaseTrace={phase_trace_json}",
        receipt.lock_elapsed_micros,
        phase_trace_path.display(),
    );
    assert!(
        receipt.lock_elapsed_micros < LOCK_P99_MICROS,
        "guard scope must not scale with previous artifact bytes: lockMicros={}",
        receipt.lock_elapsed_micros
    );
}
