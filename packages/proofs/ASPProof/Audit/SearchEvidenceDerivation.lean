-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchEvidenceDerivation

open ASPProof.SearchEvidenceReflection ASPProof.SearchEvidenceDerivation

/--
error: Tactic `decide` proved that the proposition
  ∀ (a b : Bool), a = true → fullQuery a b = true
is false
-/
#guard_msgs in
example : ∀ a b : Bool, a = true → fullQuery a b = true := by decide

/--
error: Tactic `decide` proved that the proposition
  certifiedAnswer partialDirectory storeCall = Answer.ruledOut
is false
-/
#guard_msgs in
example : certifiedAnswer partialDirectory storeCall = .ruledOut := by decide

/--
error: Tactic `decide` proved that the proposition
  ∀ (node : Bool), cycleNext (cycleNext node) ≠ node
is false
-/
#guard_msgs in
example : ∀ node : Bool, cycleNext (cycleNext node) ≠ node := by decide

/--
error: Tactic `decide` proved that the proposition
  fullQuery true true = true
is false
-/
#guard_msgs in
example : fullQuery true true = true := by decide

/--
error: Tactic `decide` proved that the proposition
  selectorFrom candidate unregisteredProjection = some Node.refresh
is false
-/
#guard_msgs in
example : selectorFrom candidate unregisteredProjection = some .refresh := by decide

/--
error: Tactic `decide` proved that the proposition
  ¬4 ∈ acquisitionCandidateUnion heterogeneousRouteCandidates
is false
-/
#guard_msgs in
example : 4 ∉ acquisitionCandidateUnion heterogeneousRouteCandidates := by decide

/--
error: Tactic `decide` proved that the proposition
  directQueryMapping 1 jsonObjectWithoutSelector =
    some { producer := NativeProducer.json, selector := 42, projectionProgram := some 42 }
is false
-/
#guard_msgs in
example : directQueryMapping 1 jsonObjectWithoutSelector =
    some ⟨.json, 42, some 42⟩ := by decide

/--
error: Tactic `decide` proved that the proposition
  derivePolyglotPipeline (polyglotPipelineRelations.erase (NativeProducer.scheme, NativeProducer.json)) =
    some
      { entry := NativeProducer.rust, ranker := NativeProducer.python, scorer := NativeProducer.julia,
        packet := NativeProducer.json, validator := NativeProducer.scheme, renderer := NativeProducer.typescript,
        contract := NativeProducer.org, guide := NativeProducer.markdown, package := NativeProducer.nix }
is false
-/
#guard_msgs in
example :
    derivePolyglotPipeline (polyglotPipelineRelations.erase (.scheme, .json)) =
      some ⟨.rust, .python, .julia, .json, .scheme,
        .typescript, .org, .markdown, .nix⟩ := by decide

/--
error: Tactic `decide` proved that the proposition
  sourceExcerptAdmitted 1 failedNativeSelectorExcerpt = true
is false
-/
#guard_msgs in
example : sourceExcerptAdmitted 1 failedNativeSelectorExcerpt = true := by decide

/--
error: Tactic `decide` proved that the proposition
  factualPremiseEligible TopologyModality.synthesizedProposed = true
is false
-/
#guard_msgs in
example : factualPremiseEligible .synthesizedProposed = true := by decide

/--
error: Tactic `decide` proved that the proposition
  topologyAssistedFactualDerivation 1 staleSemanticTopologyEvidence TopologyModality.declared true = true
is false
-/
#guard_msgs in
example :
    topologyAssistedFactualDerivation 1 staleSemanticTopologyEvidence
      .declared true = true := by decide

/--
error: Tactic `decide` proved that the proposition
  loadTopologyLibrary 1 { structural := 7, semantic := 17, program := 27 } staleTopologyLibrary =
    some staleTopologyLibrary
is false
-/
#guard_msgs in
example :
    loadTopologyLibrary 1 ⟨7, 17, 27⟩ staleTopologyLibrary =
      some staleTopologyLibrary := by decide

/--
error: Tactic `decide` proved that the proposition
  (ReasoningNode.refreshRegistry, ReasoningNode.commitReceipt) ∈ reasoningEdges
is false
-/
#guard_msgs in
example : (.refreshRegistry, .commitReceipt) ∈ reasoningEdges := by decide

/--
error: Tactic `decide` proved that the proposition
  ¬{ origin := ReasoningNode.refreshRegistry, action := ReasoningNode.publishArtifact,
        document := ReasoningNode.publicationHeading, terminal := ReasoningNode.commitReceipt } ∈
      publicationPaths
is false
-/
#guard_msgs in
example :
    (⟨.refreshRegistry, .publishArtifact, .publicationHeading, .commitReceipt⟩ :
      PublicationPathFact) ∉ publicationPaths := by decide

/--
error: Tactic `decide` proved that the proposition
  admissibleAsPremise (DerivedOutput.frontier ReasoningNode.publishArtifact 7) = true
is false
-/
#guard_msgs in
example : admissibleAsPremise (.frontier .publishArtifact 7) = true := by decide

/--
error: Tactic `decide` proved that the proposition
  deltaComplete [1, 2] [1, 2, 3] [] = true
is false
-/
#guard_msgs in
example : deltaComplete [1, 2] [1, 2, 3] [] = true := by decide

/--
error: Tactic `decide` proved that the proposition
  defaultAgentProjectionExposesAscentSource = true
is false
-/
#guard_msgs in
example : defaultAgentProjectionExposesAscentSource = true := by decide

/--
error: Tactic `decide` proved that the proposition
  annotationCanProveBehavior 1 proposedPublicationSummary = true
is false
-/
#guard_msgs in
example : annotationCanProveBehavior 1 proposedPublicationSummary = true := by decide

/--
error: Tactic `decide` proved that the proposition
  classifyRelation RelationCoverage.boundedPartial false = RelationKnowledge.certifiedMissing
is false
-/
#guard_msgs in
example : classifyRelation .boundedPartial false = .certifiedMissing := by decide

/--
error: Tactic `decide` proved that the proposition
  reachedAtValid
      { seed := ReasoningNode.refreshRegistry, node := ReasoningNode.commitReceipt, depth := 1,
        path := [ReasoningNode.refreshRegistry, ReasoningNode.publishArtifact, ReasoningNode.commitReceipt] } =
    true
is false
-/
#guard_msgs in
example : reachedAtValid
    ⟨.refreshRegistry, .commitReceipt, 1,
      [.refreshRegistry, .publishArtifact, .commitReceipt]⟩ = true := by decide

/--
error: Tactic `decide` proved that the proposition
  dependencyClosure 1 transitiveProofDependencies [3] = [3, 2, 1]
is false
-/
#guard_msgs in
example : dependencyClosure 1 transitiveProofDependencies [3] = [3, 2, 1] := by decide

/--
error: Tactic `decide` proved that the proposition
  applyGenerationDelta 1 [1, 2] removePublishedEdge = some [1, 2]
is false
-/
#guard_msgs in
example : applyGenerationDelta 1 [1, 2] removePublishedEdge = some [1, 2] := by decide

/--
error: Tactic `decide` proved that the proposition
  fixedPoint reasoningEdges reasoningEdges = true
is false
-/
#guard_msgs in
example : fixedPoint reasoningEdges reasoningEdges = true := by decide

/--
error: Tactic `decide` proved that the proposition
  settlementBoundToLibrary projectTopologyIdentity replayedSearchTopologyIdentity = true
is false
-/
#guard_msgs in
example : settlementBoundToLibrary projectTopologyIdentity replayedSearchTopologyIdentity = true := by decide

/--
error: Tactic `decide` proved that the proposition
  admittedAnnotationCanProveBehavior 1
      { annotationIdentity := 7, annotation := acceptedPublicationSummary, admissionReceipt := none } =
    true
is false
-/
#guard_msgs in
example : admittedAnnotationCanProveBehavior 1
    ⟨7, acceptedPublicationSummary, none⟩ = true := by decide

/--
error: Tactic `decide` proved that the proposition
  admittedAnnotationCanProveBehavior 1
      { annotationIdentity := 7, annotation := acceptedPublicationSummary,
        admissionReceipt := some { semanticBinding := 2, annotationIdentity := 8, receiptIdentity := 51 } } =
    true
is false
-/
#guard_msgs in
example : admittedAnnotationCanProveBehavior 1
    ⟨7, acceptedPublicationSummary, some ⟨2, 8, 51⟩⟩ = true := by decide

/--
error: Tactic `decide` proved that the proposition
  fromScratchEquivalenceAdmitted 11 17 14 13 [] none = true
is false
-/
#guard_msgs in
example : fromScratchEquivalenceAdmitted 11 17 14 13 [] none = true := by decide

/--
error: Tactic `decide` proved that the proposition
  fromScratchEquivalenceAdmitted 11 17 14 13
      [{ sourceIdentity := 11, programIdentity := 17, topologyGeneration := 12, recomputedLibrary := 13,
          receiptIdentity := 61 }]
      (some
        { sourceIdentity := 11, programIdentity := 17, topologyGeneration := 12, recomputedLibrary := 13,
          receiptIdentity := 61 }) =
    true
is false
-/
#guard_msgs in
example : fromScratchEquivalenceAdmitted 11 17 14 13 [⟨11, 17, 12, 13, 61⟩]
    (some ⟨11, 17, 12, 13, 61⟩) = true := by decide

/- This is the rejected legacy rule: a nonzero identity embedded inside the
   candidate packet was treated as independent authorization. -/
def legacyFromScratchEquivalenceAdmitted
    (expectedGeneration candidateLibrary : Nat)
    (receipt : Option TopologyRebuildReceipt) : Bool :=
  match receipt with
  | none => false
  | some proof =>
      proof.topologyGeneration == expectedGeneration &&
        proof.recomputedLibrary == candidateLibrary && proof.receiptIdentity != 0

example : legacyFromScratchEquivalenceAdmitted 14 13
    (some ⟨99, 88, 14, 13, 61⟩) = true := by decide

example : fromScratchEquivalenceAdmitted 11 17 14 13 []
    (some ⟨11, 17, 14, 13, 61⟩) = false := by decide

/--
error: Tactic `decide` proved that the proposition
  retainedDerivedFacts transitiveDerivedFacts (invalidationClosure 2 transitiveDerivedFacts [2]) = [4]
is false
-/
#guard_msgs in
example : retainedDerivedFacts transitiveDerivedFacts
    (invalidationClosure 2 transitiveDerivedFacts [2]) = [4] := by decide

/--
error: Tactic `decide` proved that the proposition
  inferenceReceiptAdmitted 11 17
      [{ sourceIdentity := 11, programIdentity := 17, candidate := reasoningEdges,
          next := closureStep reasoningEdges reasoningEdges, termination := InferenceTerminationKind.budgetExhausted,
          receiptIdentity := 71 }]
      { sourceIdentity := 11, programIdentity := 17, candidate := reasoningEdges,
        next := closureStep reasoningEdges reasoningEdges, termination := InferenceTerminationKind.budgetExhausted,
        receiptIdentity := 71 } =
    true
is false
-/
#guard_msgs in
example : inferenceReceiptAdmitted 11 17
    [⟨11, 17, reasoningEdges, closureStep reasoningEdges reasoningEdges,
      .budgetExhausted, 71⟩]
    ⟨11, 17, reasoningEdges, closureStep reasoningEdges reasoningEdges,
      .budgetExhausted, 71⟩ = true := by decide

/- The rejected legacy fixed-point rule did not bind its conclusion to source,
   MRR program, or an independently admitted receipt identity. -/
def legacyInferenceReceiptAdmitted (receipt : InferenceSettlementReceipt) : Bool :=
  receipt.termination == .fixedPoint && receipt.candidate == receipt.next

example :
    let closure := closureWithin 2 reasoningEdges
    legacyInferenceReceiptAdmitted
      ⟨99, 88, closure, closureStep reasoningEdges closure, .fixedPoint, 71⟩ = true := by
  decide

example :
    let closure := closureWithin 2 reasoningEdges
    inferenceReceiptAdmitted 11 17
      [⟨99, 88, closure, closureStep reasoningEdges closure, .fixedPoint, 71⟩]
      ⟨99, 88, closure, closureStep reasoningEdges closure, .fixedPoint, 71⟩ = false := by
  decide

/--
error: Tactic `decide` proved that the proposition
  materializationSetAdmitted [10, 20, 30] [10, 10] = true
is false
-/
#guard_msgs in
example : materializationSetAdmitted [10, 20, 30] [10, 10] = true := by decide

#print axioms same_snapshot_individual_validity_is_insufficient
#print axioms joint_path_has_one_context
#print axioms joint_path_subpath
#print axioms joint_append_requires_shared_context
#print axioms positive_clause_does_not_prove_query
#print axioms query_satisfaction_requires_negative
#print axioms missing_negative_has_two_worlds
#print axioms partial_directory_preserves_positive
#print axioms partial_directory_does_not_rule_out_omitted_member
#print axioms certified_absence_requires_truth_absence
#print axioms directory_budget_250_32_is_partial
#print axioms support_ignores_duplicate_rows
#print axioms support_ignores_reordering
#print axioms support_ignores_aliases
#print axioms support_retains_attribution
#print axioms normalized_support_membership
#print axioms changed_binding_is_distinct_support
#print axioms trace_premises_recoverable
#print axioms run_budget_conservation
#print axioms run_has_finite_step_bound
#print axioms prior_budget_is_less_than_one_step_extended_budget
#print axioms frontier_step_strictly_decreases_remaining_budget
#print axioms eligible_call_cycle_returns_to_start
#print axioms eligibility_alone_allows_two_way_cycle
#print axioms spent_budget_cannot_take_another_step
#print axioms admitted_native_projection_mints_selector
#print axioms acquisition_union_preserves_unknown_extension
#print axioms provider_extension_prefilter_changes_acquisition_semantics
#print axioms candidate_path_alone_does_not_mint_selector
#print axioms stale_native_projection_does_not_mint_selector
#print axioms bare_file_uri_is_not_a_native_syntax_anchor
#print axioms canonical_item_selector_can_anchor_native_syntax
#print axioms stale_item_selector_cannot_anchor_native_syntax
#print axioms top_k_keeps_bounded_core_and_reports_omission
#print axioms omitted_candidate_is_not_in_rendered_top_k
#print axioms witness_closure_retains_required_connector
#print axioms flat_top_k_can_drop_required_connector
#print axioms restating_gql_facts_produces_no_ascent_output
#print axioms a_new_relation_survives_the_derivation_delta
#print axioms naive_prefix_can_hide_a_distinct_provider
#print axioms diversified_frontier_can_retain_distinct_relevant_providers
#print axioms an_unselected_provider_has_no_automatic_quota
#print axioms admitted_polyglot_nodes_have_direct_query_mappings
#print axioms heterogeneous_relations_derive_one_polyglot_pipeline
#print axioms missing_language_relation_blocks_polyglot_pipeline
#print axioms jq_projection_does_not_replace_a_canonical_selector
#print axioms stale_document_selector_is_not_queryable
#print axioms every_admitted_selector_has_a_direct_mapping
#print axioms source_excerpt_is_context_not_query_authority
#print axioms native_selector_failure_cannot_downgrade_to_source_excerpt
#print axioms body_only_edit_reuses_structural_topology_not_content
#print axioms structural_edit_changes_topology_identity
#print axioms proposed_semantics_cannot_prove_behavior
#print axioms proposed_semantics_can_open_search_frontier
#print axioms declared_topology_and_live_evidence_support_joint_derivation
#print axioms stale_semantic_topology_blocks_joint_derivation
#print axioms proposed_semantics_alone_cannot_enter_factual_derivation
#print axioms exact_topology_library_import_is_admitted
#print axioms semantic_identity_drift_rejects_topology_library_import
#print axioms proposed_claims_are_not_loaded_into_factual_closure
#print axioms live_evidence_delta_is_staged_without_claiming_new_closure
#print axioms recursive_closure_derives_a_non_input_fact
#print axioms relational_join_derives_a_documented_path
#print axioms three_relation_join_derives_publication_path
#print axioms publication_materialization_is_one_query_playbook_set
#print axioms a_search_frontier_is_a_proposal_not_a_fact
#print axioms a_new_fact_that_skips_delta_violates_incremental_completeness
#print axioms a_new_fact_exposed_by_delta_satisfies_incremental_completeness
#print axioms gql_settlement_preserves_direct_and_marks_novel_derived
#print axioms agent_facing_search_projection_is_fully_determined_by_evidence
#print axioms public_search_projection_has_no_out_of_band_decision_state
#print axioms default_agent_projection_is_one_gql_settlement
#print axioms natural_language_proposal_guides_but_does_not_prove
#print axioms admitted_natural_language_requires_current_binding
#print axioms partial_coverage_cannot_certify_missing
#print axioms complete_coverage_without_a_witness_certifies_missing
#print axioms seed_relative_depth_is_bound_to_its_path
#print axioms decision_core_retains_transitive_proof_dependencies
#print axioms generation_replacement_retracts_deleted_facts
#print axioms addition_only_delta_check_is_insufficient_for_deletion
#print axioms fuel_exhaustion_is_not_a_fixed_point_certificate
#print axioms bounded_closure_reaches_the_finite_example_fixed_point
#print axioms embedded_nonzero_rebuild_receipt_cannot_authorize_itself
#print axioms foreign_program_rebuild_receipt_is_rejected
#print axioms admitted_identity_collision_cannot_authorize_forged_rebuild_fields
#print axioms unadmitted_fixed_point_receipt_is_rejected
#print axioms foreign_program_fixed_point_receipt_is_rejected
#print axioms admitted_identity_collision_cannot_authorize_forged_inference_fields
#print axioms smallest_runtime_admitted_selector_subset_is_queryable
#print axioms selectors_learned_across_searches_may_form_one_query
#print axioms runtime_binding_drift_is_rejected_before_materialization
#print axioms workspace_root_drift_is_rejected_before_materialization
#print axioms worktree_context_drift_is_rejected_before_materialization
#print axioms one_runtime_bound_terminal_materializes_the_complete_selector_set
#print axioms a_partial_ready_receipt_is_rejected
#print axioms a_failed_receipt_cannot_expose_partial_materialization
#print axioms more_than_one_query_terminal_is_rejected
