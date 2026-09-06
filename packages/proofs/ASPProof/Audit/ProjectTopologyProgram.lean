import ASPProof.ProjectTopologyProgram

/-!
Adversarial witnesses for RFC 10.06.18.  Each theorem exhibits a tempting but
unsound shortcut that the production contract must reject.
-/

namespace ASPProof.Audit.ProjectTopologyProgram

open ASPProof.ProjectTopologyProgram

theorem closed_profile_enum_would_exclude_project_extensions :
    runtimeAuthorityProfile ∉ standardProfiles ∧
      profileWellFormed runtimeAuthorityProfile = true := by
  decide

theorem display_shape_erases_the_authority_difference :
    gqlProjection directShape = gqlProjection derivedShape ∧
      factualPremise directShape.modality = true ∧
      projectProgramMayProduce directShape.modality = false := by
  decide

theorem an_ascent_candidate_is_not_an_admitted_closure :
    exhaustedCandidate.proofDependenciesComplete = true ∧
      admitClosure [parserOwner] exhaustedCandidate = none := by
  decide

theorem a_project_extension_cannot_shadow_a_core_signature :
    signatureCompatible coreDeclares shadowedDeclares = false ∧
      signatureCompatible coreDeclares projectDeploysTo = true := by
  decide

theorem a_proposed_summary_cannot_justify_a_factual_edge :
    projectProgramMayProduce proposedMeaning.modality = true ∧
      factualPremise proposedMeaning.modality = false := by
  decide

theorem equal_activation_generation_allows_stale_topology :
    topologyBefore.activationGeneration =
        topologyWithStaleSource.activationGeneration ∧
      topologyBefore.sourceSnapshotDigest ≠
        topologyWithStaleSource.sourceSnapshotDigest := by
  decide

theorem append_only_incremental_update_cannot_express_deletion :
    containsFactKey (previousFacts ++ []) derivedMember.key = true ∧
      containsFactKey (replaceFacts previousFacts [derivedMember.key] [])
        derivedMember.key = false := by
  decide

theorem one_pass_deletion_does_not_close_transitive_dependencies :
    2 ∈ invalidationClosure 1 twoHopDependencies [1] ∧
      3 ∉ invalidationClosure 1 twoHopDependencies [1] ∧
      3 ∈ invalidationClosure 2 twoHopDependencies [1] := by
  decide

theorem fuel_zero_returns_even_though_the_relation_is_not_closed :
    closureRun 0 [] = [] ∧
      closureStep (closureRun 0 []) ≠ closureRun 0 [] := by
  decide

theorem a_delta_for_another_predecessor_cannot_be_replayed :
    applyDelta 40 previousFacts removalDelta = none := by
  decide

theorem scheme_source_without_compiled_abi_is_not_runtime_ready :
    uncompiledProgram.schemeProgramDigest = programA.schemeProgramDigest ∧
      programReady uncompiledProgram = false := by
  decide

theorem indexing_materialized_topology_would_create_a_feedback_input :
    eligibleTopologyInput .workspaceSource = true ∧
      eligibleTopologyInput .topologyMaterialization = false := by
  decide

end ASPProof.Audit.ProjectTopologyProgram
