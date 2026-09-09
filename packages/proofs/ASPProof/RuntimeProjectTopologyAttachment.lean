-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.ProjectTopologyIdentityRefinement

/-!
Executable refinement model for RFC 10.06.20.

The model separates a Runtime generation from its immutable Project Topology
attachment.  Search settlement admission requires the complete attachment;
exact Query materialization deliberately does not consume Search settlement or
topology state.
-/

namespace ASPProof.RuntimeProjectTopologyAttachment

open ASPProof.ProjectTopologyIdentityRefinement

structure RuntimeGenerationBinding where
  projectWorkspace : ProjectWorkspaceBinding
  generationDigest : Nat
  runtimeArtifactDigest : Nat
  sourceSnapshotDigest : Identity .sourceSnapshot
  providerCatalogDigest : Identity .providerCatalog
  deriving DecidableEq, Repr

inductive InferenceState where
  | admitted
  | failed
  | blocked
  deriving DecidableEq, Repr

structure TopologyInferenceReceipt where
  receiptIdentity : Nat
  state : InferenceState
  topologyBinding : ProductBinding
  libraryIdentity : Nat
  closureDigest : Nat
  deriving DecidableEq, Repr

structure RuntimeProjectTopologyAttachment where
  runtimeGeneration : RuntimeGenerationBinding
  topologyBinding : ProductBinding
  libraryIdentity : Nat
  inferenceReceipt : TopologyInferenceReceipt
  deriving DecidableEq, Repr

def attachmentAdmitted
    (expectedRuntime : RuntimeGenerationBinding)
    (independentlyAdmitted : List TopologyInferenceReceipt)
    (attachment : RuntimeProjectTopologyAttachment) : Bool :=
  attachment.runtimeGeneration == expectedRuntime &&
    attachment.topologyBinding.projectWorkspace == expectedRuntime.projectWorkspace &&
    attachment.topologyBinding.sourceSnapshotDigest == expectedRuntime.sourceSnapshotDigest &&
    attachment.topologyBinding.providerCatalogDigest == expectedRuntime.providerCatalogDigest &&
    attachment.inferenceReceipt.state == .admitted &&
    independentlyAdmitted.contains attachment.inferenceReceipt &&
    attachment.inferenceReceipt.topologyBinding == attachment.topologyBinding &&
    attachment.inferenceReceipt.libraryIdentity == attachment.libraryIdentity

/-! Tokio execution is represented by observable generation states rather than
thread identities.  CPU work may finish after cancellation, but only a ready
build carrying the exact admitted attachment can cross the atomic publication
boundary. -/

inductive TopologyBuildState where
  | building
  | canceled
  | failed
  | ready
  deriving DecidableEq, Repr

structure TopologyGenerationBuild where
  identity : ProductBinding
  state : TopologyBuildState
  attachment : Option RuntimeProjectTopologyAttachment
  deriving DecidableEq, Repr

def topologyBuildPublishable
    (expectedRuntime : RuntimeGenerationBinding)
    (independentlyAdmitted : List TopologyInferenceReceipt)
    (build : TopologyGenerationBuild) : Bool :=
  build.state == .ready &&
    match build.attachment with
    | none => false
    | some attachment =>
        build.identity == attachment.topologyBinding &&
          attachmentAdmitted expectedRuntime independentlyAdmitted attachment

def topologySingleFlightKey (build : TopologyGenerationBuild) : ProductBinding :=
  build.identity

structure RuntimeSearchGenerationBuild where
  graph : TopologyBuildState
  tantivy : TopologyBuildState
  topology : TopologyGenerationBuild
  deriving DecidableEq, Repr

def runtimeSearchGenerationPublishable
    (expectedRuntime : RuntimeGenerationBinding)
    (independentlyAdmitted : List TopologyInferenceReceipt)
    (build : RuntimeSearchGenerationBuild) : Bool :=
  build.graph == .ready && build.tantivy == .ready &&
    topologyBuildPublishable expectedRuntime independentlyAdmitted build.topology

structure SearchSettlementCandidate where
  requestIdentity : Nat
  runtimeGeneration : RuntimeGenerationBinding
  libraryIdentity : Nat
  topologyRootDigest : Identity .topologyRoot
  topologyClosureDigest : Identity .topologyClosure
  evidenceDigest : Nat
  oneGqlBlock : Bool
  exposesAscentSource : Bool
  hasPlannerState : Bool
  deriving DecidableEq, Repr

def searchSettlementAdmitted
    (expectedRuntime : RuntimeGenerationBinding)
    (independentlyAdmitted : List TopologyInferenceReceipt)
    (attachment : Option RuntimeProjectTopologyAttachment)
    (candidate : SearchSettlementCandidate) : Bool :=
  match attachment with
  | none => false
  | some attached =>
      attachmentAdmitted expectedRuntime independentlyAdmitted attached &&
        candidate.runtimeGeneration == expectedRuntime &&
        candidate.libraryIdentity == attached.libraryIdentity &&
        candidate.topologyRootDigest == attached.topologyBinding.topologyRootDigest &&
        candidate.topologyClosureDigest ==
          attached.topologyBinding.topologyClosureDigest &&
        candidate.oneGqlBlock && !candidate.exposesAscentSource &&
        !candidate.hasPlannerState

structure QueryMaterializationCandidate where
  runtimeGeneration : RuntimeGenerationBinding
  selectors : List Nat
  allSelectorsExact : Bool
  deriving DecidableEq, Repr

def queryMaterializationAdmitted
    (expectedRuntime : RuntimeGenerationBinding)
    (candidate : QueryMaterializationCandidate) : Bool :=
  candidate.runtimeGeneration == expectedRuntime &&
    candidate.allSelectorsExact && !candidate.selectors.isEmpty

def runtimeA : RuntimeGenerationBinding :=
  ⟨bindingA.projectWorkspace, 31, 41, bindingA.sourceSnapshotDigest,
    bindingA.providerCatalogDigest⟩

def receiptA : TopologyInferenceReceipt :=
  ⟨51, .admitted, bindingA, 61, 71⟩

def attachmentA : RuntimeProjectTopologyAttachment :=
  ⟨runtimeA, bindingA, 61, receiptA⟩

def settlementA : SearchSettlementCandidate :=
  ⟨81, runtimeA, attachmentA.libraryIdentity, bindingA.topologyRootDigest,
    bindingA.topologyClosureDigest, 91, true, false, false⟩

def settlementB : SearchSettlementCandidate :=
  { settlementA with requestIdentity := 82, evidenceDigest := 92 }

def queryA : QueryMaterializationCandidate :=
  ⟨runtimeA, [101, 102], true⟩

theorem exact_attachment_is_admitted :
    attachmentAdmitted runtimeA [receiptA] attachmentA = true := by
  decide

theorem missing_attachment_cannot_mint_a_search_settlement :
    searchSettlementAdmitted runtimeA [receiptA] none settlementA = false := by
  decide

theorem flat_evidence_without_attachment_has_no_gql_authority :
    let flatEvidenceCandidate := { settlementA with evidenceDigest := 999 }
    searchSettlementAdmitted runtimeA [receiptA] none flatEvidenceCandidate = false := by
  decide

theorem stale_source_attachment_is_rejected :
    let staleRuntime := { runtimeA with sourceSnapshotDigest := ⟨999⟩ }
    attachmentAdmitted staleRuntime [receiptA] attachmentA = false := by
  decide

theorem stale_provider_attachment_is_rejected :
    let staleRuntime := { runtimeA with providerCatalogDigest := ⟨999⟩ }
    attachmentAdmitted staleRuntime [receiptA] attachmentA = false := by
  decide

theorem another_runtime_generation_is_rejected :
    let nextRuntime := { runtimeA with generationDigest := 32 }
    attachmentAdmitted nextRuntime [receiptA] attachmentA = false := by
  decide

theorem another_runtime_artifact_is_rejected :
    let rebuiltRuntime := { runtimeA with runtimeArtifactDigest := 42 }
    attachmentAdmitted rebuiltRuntime [receiptA] attachmentA = false := by
  decide

theorem another_project_workspace_is_rejected :
    let movedWorkspace :=
      { runtimeA with
          projectWorkspace :=
            { runtimeA.projectWorkspace with workspaceRoot := ⟨999⟩ } }
    attachmentAdmitted movedWorkspace [receiptA] attachmentA = false := by
  decide

theorem inference_receipt_must_be_independently_admitted :
    attachmentAdmitted runtimeA [] attachmentA = false := by
  decide

theorem building_without_attachment_cannot_publish :
    topologyBuildPublishable runtimeA [receiptA]
      ⟨bindingA, .building, none⟩ = false := by
  decide

theorem canceled_cpu_completion_cannot_publish :
    topologyBuildPublishable runtimeA [receiptA]
      ⟨bindingA, .canceled, some attachmentA⟩ = false := by
  decide

theorem ready_requires_the_exact_admitted_attachment :
    topologyBuildPublishable runtimeA [receiptA]
      ⟨bindingA, .ready, some attachmentA⟩ = true := by
  decide

theorem single_flight_uses_the_complete_product_identity :
    let changedProvider := { bindingA with providerCatalogDigest := ⟨999⟩ }
    topologySingleFlightKey ⟨bindingA, .building, none⟩ ≠
      topologySingleFlightKey ⟨changedProvider, .building, none⟩ := by
  decide

theorem graph_and_tantivy_cannot_publish_without_topology :
    runtimeSearchGenerationPublishable runtimeA [receiptA]
      ⟨.ready, .ready, ⟨bindingA, .building, none⟩⟩ = false := by
  decide

theorem topology_cannot_publish_without_tantivy :
    runtimeSearchGenerationPublishable runtimeA [receiptA]
      ⟨.ready, .building, ⟨bindingA, .ready, some attachmentA⟩⟩ = false := by
  decide

theorem exactly_one_joint_ready_product_is_publishable :
    runtimeSearchGenerationPublishable runtimeA [receiptA]
      ⟨.ready, .ready, ⟨bindingA, .ready, some attachmentA⟩⟩ = true := by
  decide

theorem receipt_identity_collision_cannot_change_the_topology_binding :
    let forgedReceipt :=
      { receiptA with
          topologyBinding :=
            { bindingA with sourceSnapshotDigest := ⟨999⟩ } }
    let forgedAttachment := { attachmentA with inferenceReceipt := forgedReceipt }
    attachmentAdmitted runtimeA [receiptA] forgedAttachment = false := by
  decide

theorem planner_state_is_not_part_of_a_public_search_settlement :
    searchSettlementAdmitted runtimeA [receiptA] (some attachmentA)
      { settlementA with hasPlannerState := true } = false := by
  decide

theorem ascent_source_is_not_part_of_a_public_search_settlement :
    searchSettlementAdmitted runtimeA [receiptA] (some attachmentA)
      { settlementA with exposesAscentSource := true } = false := by
  decide

theorem another_topology_closure_cannot_replay_a_search_settlement :
    searchSettlementAdmitted runtimeA [receiptA] (some attachmentA)
      { settlementA with topologyClosureDigest := ⟨999⟩ } = false := by
  decide

theorem one_immutable_attachment_supports_independent_searches :
    searchSettlementAdmitted runtimeA [receiptA] (some attachmentA) settlementA = true ∧
      searchSettlementAdmitted runtimeA [receiptA] (some attachmentA) settlementB = true ∧
      settlementA.requestIdentity ≠ settlementB.requestIdentity := by
  decide

theorem exact_query_materialization_is_independent_of_search_attachment :
    queryMaterializationAdmitted runtimeA queryA = true := by
  decide

theorem query_does_not_accept_an_empty_or_inexact_selector_set :
    queryMaterializationAdmitted runtimeA
        { queryA with selectors := [] } = false ∧
      queryMaterializationAdmitted runtimeA
        { queryA with allSelectorsExact := false } = false := by
  decide

end ASPProof.RuntimeProjectTopologyAttachment
