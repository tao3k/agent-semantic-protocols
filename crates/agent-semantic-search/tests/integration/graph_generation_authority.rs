use agent_semantic_content_identity::{
    SourceSnapshotEvidence, SourceSnapshotKind,
    workspace_generation_evidence::{
        ValidatedWorkspaceGenerationV1, WorkspaceGenerationEvidenceError,
        WorkspaceGenerationEvidenceV1,
    },
};
use agent_semantic_search::graph_generation_authority::AdmittedGraphGenerationV1;
use std::time::{Duration, Instant};

fn generation(root: &str, leaf_count: u64) -> ValidatedWorkspaceGenerationV1 {
    ValidatedWorkspaceGenerationV1::new(WorkspaceGenerationEvidenceV1 {
        root_digest: root.to_owned(),
        root_depth: 1,
        leaf_count,
        owner_count: leaf_count - 1,
    })
    .unwrap()
}

fn snapshot(root: &str, leaf_count: usize) -> SourceSnapshotEvidence {
    SourceSnapshotEvidence::new(root, SourceSnapshotKind::Filesystem, leaf_count, "provider")
}

#[test]
fn graph_admission_rejects_the_observed_partial_overlay_shape() {
    let source = snapshot("partial-root", 19);
    let candidate = generation("partial-root", 19);
    let active = generation("active-root", 85);
    assert_eq!(
        AdmittedGraphGenerationV1::admit(&source, &candidate, &active).unwrap_err(),
        WorkspaceGenerationEvidenceError::GenerationMismatch {
            candidate_root: "partial-root".to_owned(),
            active_root: "active-root".to_owned(),
        }
    );
}

#[test]
fn graph_admission_rejects_snapshot_and_generation_coverage_drift() {
    let source = snapshot("active-root", 19);
    let active = generation("active-root", 85);
    assert_eq!(
        AdmittedGraphGenerationV1::admit(&source, &active, &active).unwrap_err(),
        WorkspaceGenerationEvidenceError::SnapshotGenerationMismatch {
            snapshot_root: "active-root".to_owned(),
            generation_root: "active-root".to_owned(),
            snapshot_leaf_count: 19,
            generation_leaf_count: 85,
        }
    );
}

#[test]
fn graph_admission_accepts_one_complete_active_generation() {
    let source = snapshot("active-root", 85);
    let active = generation("active-root", 85);
    let admitted = AdmittedGraphGenerationV1::admit(&source, &active, &active).unwrap();
    assert_eq!(admitted.source_snapshot().root_digest, "active-root");
    assert_eq!(admitted.generation(), active.evidence());
}

#[test]
fn graph_generation_admission_is_sub_millisecond() {
    let source = snapshot("active-root", 85);
    let active = generation("active-root", 85);
    let started = Instant::now();
    for _ in 0..1_024 {
        std::hint::black_box(AdmittedGraphGenerationV1::admit(&source, &active, &active).unwrap());
    }
    eprintln!(
        "graphGenerationAdmission iterations=1024 elapsedMicros={}",
        started.elapsed().as_micros()
    );
    assert!(
        started.elapsed() < Duration::from_millis(1),
        "1024 graph generation admissions exceeded the 1ms hard gate: {:?}",
        started.elapsed()
    );
}
