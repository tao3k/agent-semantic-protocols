-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.RuntimeProjectTopologyAttachment

namespace ASPProof.Audit.RuntimeProjectTopologyAttachment

open ASPProof.RuntimeProjectTopologyAttachment

theorem equal_activation_observation_cannot_repair_a_stale_source_attachment :
    let activationBefore := 86
    let activationAfter := 86
    let staleRuntime := { runtimeA with sourceSnapshotDigest := ⟨999⟩ }
    activationBefore = activationAfter ∧
      attachmentAdmitted staleRuntime [receiptA] attachmentA = false := by
  decide

theorem matching_display_shape_cannot_replace_the_attachment :
    settlementA.oneGqlBlock = true ∧
      searchSettlementAdmitted runtimeA [receiptA] none settlementA = false := by
  decide

theorem a_self_embedded_receipt_is_not_independently_admitted :
    attachmentA.inferenceReceipt.state = .admitted ∧
      attachmentAdmitted runtimeA [] attachmentA = false := by
  decide

theorem query_independence_does_not_weaken_runtime_binding :
    let staleRuntime := { runtimeA with generationDigest := 32 }
    queryMaterializationAdmitted runtimeA queryA = true ∧
      queryMaterializationAdmitted staleRuntime queryA = false := by
  decide

theorem a_late_blocking_result_cannot_escape_cancellation :
    topologyBuildPublishable runtimeA [receiptA]
      (TopologyGenerationBuild.mk bindingA .canceled (some attachmentA)) = false := by
  rfl

theorem activation_tokens_cannot_make_a_build_publishable :
    let activationBefore : Nat := 86
    let activationAfter : Nat := 87
    let build : TopologyGenerationBuild :=
      { identity := ASPProof.ProjectTopologyIdentityRefinement.bindingA
        state := .building
        attachment := some attachmentA }
    activationBefore < activationAfter ∧
      topologyBuildPublishable runtimeA [receiptA] build = false := by
  decide

#print axioms exact_attachment_is_admitted
#print axioms missing_attachment_cannot_mint_a_search_settlement
#print axioms flat_evidence_without_attachment_has_no_gql_authority
#print axioms stale_source_attachment_is_rejected
#print axioms stale_provider_attachment_is_rejected
#print axioms another_runtime_generation_is_rejected
#print axioms another_runtime_artifact_is_rejected
#print axioms another_project_workspace_is_rejected
#print axioms inference_receipt_must_be_independently_admitted
#print axioms receipt_identity_collision_cannot_change_the_topology_binding
#print axioms planner_state_is_not_part_of_a_public_search_settlement
#print axioms ascent_source_is_not_part_of_a_public_search_settlement
#print axioms another_topology_closure_cannot_replay_a_search_settlement
#print axioms one_immutable_attachment_supports_independent_searches
#print axioms exact_query_materialization_is_independent_of_search_attachment
#print axioms query_does_not_accept_an_empty_or_inexact_selector_set
#print axioms building_without_attachment_cannot_publish
#print axioms canceled_cpu_completion_cannot_publish
#print axioms ready_requires_the_exact_admitted_attachment
#print axioms single_flight_uses_the_complete_product_identity
#print axioms graph_and_tantivy_cannot_publish_without_topology
#print axioms topology_cannot_publish_without_tantivy
#print axioms exactly_one_joint_ready_product_is_publishable
#print axioms a_late_blocking_result_cannot_escape_cancellation
#print axioms activation_tokens_cannot_make_a_build_publishable

end ASPProof.Audit.RuntimeProjectTopologyAttachment
