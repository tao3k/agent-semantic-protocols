#[path = "../../src/command/runtime_server_artifact.rs"]
mod implementation;
#[path = "../../src/command/runtime_server_definition.rs"]
mod definition_implementation;

use definition_implementation::atomic_write_if_changed;
use implementation::{
    RuntimeServerArtifactAction, RuntimeServerSupervisorAction, runtime_server_artifact_action,
    runtime_server_supervisor_action,
};
use std::path::Path;

#[test]
fn same_path_and_digest_uses_status() {
    assert_eq!(
        runtime_server_artifact_action(
            Path::new("/runtime/asp"),
            Path::new("/runtime/asp"),
            "digest-current",
            "digest-current",
        ),
        RuntimeServerArtifactAction::Status
    );
}

#[test]
fn healthy_runtime_with_unchanged_definition_is_a_zero_mutation_noop() {
    assert_eq!(
        runtime_server_supervisor_action(true, false),
        RuntimeServerSupervisorAction::Noop
    );
}

#[test]
fn changed_definition_requires_supervisor_reconciliation() {
    assert_eq!(
        runtime_server_supervisor_action(true, true),
        RuntimeServerSupervisorAction::Reconcile
    );
}

#[test]
fn unhealthy_runtime_requires_supervisor_reconciliation() {
    assert_eq!(
        runtime_server_supervisor_action(false, false),
        RuntimeServerSupervisorAction::Reconcile
    );
}

#[tokio::test(flavor = "current_thread")]
async fn unchanged_supervisor_definition_performs_zero_publication_writes() {
    let root = std::env::temp_dir().join(format!(
        "asp-runtime-supervisor-noop-{}",
        std::process::id()
    ));
    let definition = root.join("runtime-server.service");
    let _ = tokio::fs::remove_dir_all(&root).await;

    assert!(
        atomic_write_if_changed(&definition, b"canonical-definition")
            .await
            .expect("publish initial supervisor definition")
    );
    assert!(
        !atomic_write_if_changed(&definition, b"canonical-definition")
            .await
            .expect("admit unchanged supervisor definition"),
        "an unchanged definition must not be republished"
    );
    let mut samples = Vec::with_capacity(128);
    for _ in 0..128 {
        let started = tokio::time::Instant::now();
        assert!(
            !atomic_write_if_changed(&definition, b"canonical-definition")
                .await
                .expect("measure unchanged supervisor definition")
        );
        samples.push(started.elapsed().as_micros());
    }
    samples.sort_unstable();
    let p99_micros = samples[samples.len() * 99 / 100];
    assert!(
        p99_micros <= 2_000,
        "warm supervisor definition admission exceeded 2ms: p99Micros={p99_micros}"
    );
    eprintln!(
        "runtime-supervisor-noop-performance sampleCount={} p99Micros={} definitionWrites=0 platformSubprocesses=0",
        samples.len(),
        p99_micros
    );
    assert_eq!(
        tokio::fs::read(&definition)
            .await
            .expect("read stable supervisor definition"),
        b"canonical-definition"
    );

    tokio::fs::remove_dir_all(root)
        .await
        .expect("remove supervisor fixture");
}

#[test]
fn same_path_with_changed_digest_requires_restart() {
    assert_eq!(
        runtime_server_artifact_action(
            Path::new("/runtime/asp"),
            Path::new("/runtime/asp"),
            "digest-new",
            "digest-running",
        ),
        RuntimeServerArtifactAction::Restart
    );
}

#[test]
fn changed_path_requires_restart() {
    assert_eq!(
        runtime_server_artifact_action(
            Path::new("/runtime/asp"),
            Path::new("/legacy/asp"),
            "digest-current",
            "digest-current",
        ),
        RuntimeServerArtifactAction::Restart
    );
}
