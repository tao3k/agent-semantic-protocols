-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.PolyglotSearchConformance

/-!
Executable refinement model for the reference polyglot search validator.  Each
stage is fail-closed and pure: rejection preserves the caller's state digest.
-/

structure DocumentConformance where
  canonicalUnicode : Bool
  canonicalFraming : Bool
  uniqueSections : Bool
  controlProfileKnown : Bool
  gqlReadOnly : Bool
  gqlBounded : Bool
  logicQueryOnly : Bool
  predicatesRegistered : Bool
  sourceSummaryBound : Bool
  evaluationBounded : Bool
  deriving Repr, DecidableEq, BEq

def documentAdmitted (check : DocumentConformance) : Bool :=
  check.canonicalUnicode &&
    check.canonicalFraming &&
    check.uniqueSections &&
    check.controlProfileKnown &&
    check.gqlReadOnly &&
    check.gqlBounded &&
    check.logicQueryOnly &&
    check.predicatesRegistered &&
    check.sourceSummaryBound &&
    check.evaluationBounded

def validDocument : DocumentConformance :=
  { canonicalUnicode := true
    canonicalFraming := true
    uniqueSections := true
    controlProfileKnown := true
    gqlReadOnly := true
    gqlBounded := true
    logicQueryOnly := true
    predicatesRegistered := true
    sourceSummaryBound := true
    evaluationBounded := true }

def gqlMutationDocument : DocumentConformance :=
  { validDocument with gqlReadOnly := false }

def forgedSummaryDocument : DocumentConformance :=
  { validDocument with sourceSummaryBound := false }

def unboundedLogicDocument : DocumentConformance :=
  { validDocument with evaluationBounded := false }

theorem valid_document_is_admitted :
    documentAdmitted validDocument = true := by
  decide

theorem gql_mutation_is_rejected :
    documentAdmitted gqlMutationDocument = false := by
  decide

theorem safe_summary_beside_mutating_source_is_rejected :
    documentAdmitted forgedSummaryDocument = false := by
  decide

theorem unbounded_logic_evaluation_is_rejected :
    documentAdmitted unboundedLogicDocument = false := by
  decide

structure RelationBatchConformance where
  schemaVersionMatches : Bool
  abiVersionMatches : Bool
  snapshotDigestMatches : Bool
  providerDigestMatches : Bool
  relationIdsNamespaced : Bool
  relationIdsUnique : Bool
  rowSchemaMatches : Bool
  endpointsKnown : Bool
  predicateVersionsMatch : Bool
  turboAuthorityCandidate : Bool
  provenancePresent : Bool
  deriving Repr, DecidableEq, BEq

def relationBatchAdmitted (check : RelationBatchConformance) : Bool :=
  check.schemaVersionMatches &&
    check.abiVersionMatches &&
    check.snapshotDigestMatches &&
    check.providerDigestMatches &&
    check.relationIdsNamespaced &&
    check.relationIdsUnique &&
    check.rowSchemaMatches &&
    check.endpointsKnown &&
    check.predicateVersionsMatch &&
    check.turboAuthorityCandidate &&
    check.provenancePresent

def validRelationBatch : RelationBatchConformance :=
  { schemaVersionMatches := true
    abiVersionMatches := true
    snapshotDigestMatches := true
    providerDigestMatches := true
    relationIdsNamespaced := true
    relationIdsUnique := true
    rowSchemaMatches := true
    endpointsKnown := true
    predicateVersionsMatch := true
    turboAuthorityCandidate := true
    provenancePresent := true }

def staleRelationBatch : RelationBatchConformance :=
  { validRelationBatch with snapshotDigestMatches := false }

def mismatchedRowBatch : RelationBatchConformance :=
  { validRelationBatch with rowSchemaMatches := false }

def elevatedTurboBatch : RelationBatchConformance :=
  { validRelationBatch with turboAuthorityCandidate := false }

theorem valid_relation_batch_is_admitted :
    relationBatchAdmitted validRelationBatch = true := by
  decide

theorem stale_relation_snapshot_is_rejected :
    relationBatchAdmitted staleRelationBatch = false := by
  decide

theorem relation_row_schema_mismatch_is_rejected :
    relationBatchAdmitted mismatchedRowBatch = false := by
  decide

theorem turbo_feature_authority_elevation_is_rejected :
    relationBatchAdmitted elevatedTurboBatch = false := by
  decide

structure FrontierConformance where
  semanticDigestMatches : Bool
  exposureCount : Nat
  exposureBudget : Nat
  selectionCount : Nat
  selectionBudget : Nat
  visibleIdsUnique : Bool
  selectionIsVisibleSubset : Bool
  omittedCount : Nat
  continuationPresent : Bool
  continuationDigestMatches : Bool
  omissionCertificatePresent : Bool
  deriving Repr, DecidableEq, BEq

def frontierAdmitted (check : FrontierConformance) : Bool :=
  check.semanticDigestMatches &&
    check.exposureBudget <= 10 &&
    check.exposureCount <= check.exposureBudget &&
    check.selectionBudget <= 3 &&
    check.selectionCount <= check.selectionBudget &&
    check.visibleIdsUnique &&
    check.selectionIsVisibleSubset &&
    (check.omittedCount == 0 ||
      (check.continuationPresent &&
        check.continuationDigestMatches &&
        check.omissionCertificatePresent))

def validFrontier : FrontierConformance :=
  { semanticDigestMatches := true
    exposureCount := 10
    exposureBudget := 10
    selectionCount := 3
    selectionBudget := 3
    visibleIdsUnique := true
    selectionIsVisibleSubset := true
    omittedCount := 7
    continuationPresent := true
    continuationDigestMatches := true
    omissionCertificatePresent := true }

def unexposedSelectionFrontier : FrontierConformance :=
  { validFrontier with selectionIsVisibleSubset := false }

def staleContinuationFrontier : FrontierConformance :=
  { validFrontier with continuationDigestMatches := false }

def duplicateVisibleFrontier : FrontierConformance :=
  { validFrontier with visibleIdsUnique := false }

theorem valid_progressive_frontier_is_admitted :
    frontierAdmitted validFrontier = true := by
  decide

theorem unexposed_selection_is_rejected :
    frontierAdmitted unexposedSelectionFrontier = false := by
  decide

theorem stale_frontier_continuation_is_rejected :
    frontierAdmitted staleContinuationFrontier = false := by
  decide

theorem duplicate_visible_candidates_are_rejected :
    frontierAdmitted duplicateVisibleFrontier = false := by
  decide

structure CertificateConformance where
  exactProjectionDigestBound : Bool
  formalAuditAxiomFree : Bool
  conformanceReceiptCount : Nat
  benchmarkSliceCount : Nat
  runtimeDigestsMatch : Bool
  safetyCountsZero : Bool
  qualityPassed : Bool
  efficiencyPassed : Bool
  canaryPassed : Bool
  rollbackPresent : Bool
  deriving Repr, DecidableEq, BEq

def certificateAdmitted (check : CertificateConformance) : Bool :=
  check.exactProjectionDigestBound &&
    check.formalAuditAxiomFree &&
    check.conformanceReceiptCount >= 5 &&
    check.benchmarkSliceCount >= 2 &&
    check.runtimeDigestsMatch &&
    check.safetyCountsZero &&
    check.qualityPassed &&
    check.efficiencyPassed &&
    check.canaryPassed &&
    check.rollbackPresent

def validCertificate : CertificateConformance :=
  { exactProjectionDigestBound := true
    formalAuditAxiomFree := true
    conformanceReceiptCount := 5
    benchmarkSliceCount := 2
    runtimeDigestsMatch := true
    safetyCountsZero := true
    qualityPassed := true
    efficiencyPassed := true
    canaryPassed := true
    rollbackPresent := true }

def unboundProjectionCertificate : CertificateConformance :=
  { validCertificate with exactProjectionDigestBound := false }

def runtimeDriftCertificate : CertificateConformance :=
  { validCertificate with runtimeDigestsMatch := false }

theorem valid_certificate_is_admitted :
    certificateAdmitted validCertificate = true := by
  decide

theorem certificate_must_bind_exact_projected_packet :
    certificateAdmitted unboundProjectionCertificate = false := by
  decide

theorem runtime_drift_certificate_is_rejected :
    certificateAdmitted runtimeDriftCertificate = false := by
  decide

structure PipelineConformance where
  document : DocumentConformance
  relations : RelationBatchConformance
  frontier : FrontierConformance
  certificate : CertificateConformance
  deriving Repr, DecidableEq, BEq

def pipelineAdmitted (pipeline : PipelineConformance) : Bool :=
  documentAdmitted pipeline.document &&
    relationBatchAdmitted pipeline.relations &&
    frontierAdmitted pipeline.frontier &&
    certificateAdmitted pipeline.certificate

def PipelineRefines (pipeline : PipelineConformance) : Prop :=
  documentAdmitted pipeline.document = true ∧
    relationBatchAdmitted pipeline.relations = true ∧
    frontierAdmitted pipeline.frontier = true ∧
    certificateAdmitted pipeline.certificate = true

def validPipeline : PipelineConformance :=
  { document := validDocument
    relations := validRelationBatch
    frontier := validFrontier
    certificate := validCertificate }

theorem valid_pipeline_is_executable :
    pipelineAdmitted validPipeline = true := by
  decide

theorem refinement_exposes_document_conformance
    (pipeline : PipelineConformance)
    (refines : PipelineRefines pipeline) :
    documentAdmitted pipeline.document = true :=
  refines.1

theorem refinement_exposes_relation_conformance
    (pipeline : PipelineConformance)
    (refines : PipelineRefines pipeline) :
    relationBatchAdmitted pipeline.relations = true :=
  refines.2.1

theorem refinement_exposes_frontier_conformance
    (pipeline : PipelineConformance)
    (refines : PipelineRefines pipeline) :
    frontierAdmitted pipeline.frontier = true :=
  refines.2.2.1

theorem refinement_exposes_certificate_conformance
    (pipeline : PipelineConformance)
    (refines : PipelineRefines pipeline) :
    certificateAdmitted pipeline.certificate = true :=
  refines.2.2.2

def validatorAfterState
    (beforeState : String)
    (admitted : Bool)
    (acceptedState : String) : String :=
  if admitted then acceptedState else beforeState

theorem rejected_document_preserves_state :
    validatorAfterState "state:before"
      (documentAdmitted gqlMutationDocument) "state:accepted" = "state:before" := by
  decide

theorem rejected_relation_batch_preserves_state :
    validatorAfterState "state:before"
      (relationBatchAdmitted staleRelationBatch) "state:accepted" = "state:before" := by
  decide

theorem rejected_frontier_preserves_state :
    validatorAfterState "state:before"
      (frontierAdmitted staleContinuationFrontier) "state:accepted" = "state:before" := by
  decide

theorem rejected_certificate_preserves_state :
    validatorAfterState "state:before"
      (certificateAdmitted runtimeDriftCertificate) "state:accepted" = "state:before" := by
  decide

end ASPProof.PolyglotSearchConformance
