use crate::workspace_generation_qualification::{
    WorkspaceGenerationQualificationEvidence, WorkspaceGenerationReadiness,
    WorkspaceGenerationRejection,
};

#[test]
fn admits_a_complete_non_empty_generation() {
    let readiness = WorkspaceGenerationQualificationEvidence {
        projection_segment_header_valid: true,
        candidate_source_owner_count: 3,
        admitted_source_owner_count: 2,
        rejected_source_owner_count: 1,
    }
    .qualify();

    assert_eq!(
        readiness,
        Ok(WorkspaceGenerationReadiness {
            source_owner_count: 2,
            legitimately_empty: false,
        })
    );
}

#[test]
fn admits_only_a_provider_proven_empty_workspace() {
    let readiness = WorkspaceGenerationQualificationEvidence {
        projection_segment_header_valid: true,
        candidate_source_owner_count: 0,
        admitted_source_owner_count: 0,
        rejected_source_owner_count: 0,
    }
    .qualify();

    assert_eq!(
        readiness,
        Ok(WorkspaceGenerationReadiness {
            source_owner_count: 0,
            legitimately_empty: true,
        })
    );
}

#[test]
fn rejects_an_invalid_projection_segment_header() {
    let readiness = WorkspaceGenerationQualificationEvidence {
        projection_segment_header_valid: false,
        candidate_source_owner_count: 1,
        admitted_source_owner_count: 1,
        rejected_source_owner_count: 0,
    }
    .qualify();

    assert_eq!(
        readiness,
        Err(WorkspaceGenerationRejection::InvalidProjectionSegmentHeader)
    );
}

#[test]
fn rejects_ready_empty_for_a_non_empty_inventory() {
    let readiness = WorkspaceGenerationQualificationEvidence {
        projection_segment_header_valid: true,
        candidate_source_owner_count: 2,
        admitted_source_owner_count: 0,
        rejected_source_owner_count: 2,
    }
    .qualify();

    assert_eq!(
        readiness,
        Err(
            WorkspaceGenerationRejection::EmptyProjectionForNonEmptySourceInventory {
                candidate_source_owner_count: 2,
                rejected_source_owner_count: 2,
            }
        )
    );
}

#[test]
fn rejects_an_incompletely_classified_inventory() {
    let readiness = WorkspaceGenerationQualificationEvidence {
        projection_segment_header_valid: true,
        candidate_source_owner_count: 3,
        admitted_source_owner_count: 1,
        rejected_source_owner_count: 1,
    }
    .qualify();

    assert_eq!(
        readiness,
        Err(WorkspaceGenerationRejection::IncompleteSourceInventory {
            candidate_source_owner_count: 3,
            admitted_source_owner_count: 1,
            rejected_source_owner_count: 1,
        })
    );
}
