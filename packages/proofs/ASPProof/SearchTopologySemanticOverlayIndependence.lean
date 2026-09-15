-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

/-!
Executable V1 identity model for Search topology, exact Query materialization,
and Agent-produced Org summaries.

Search topology is a projection of the immutable Search Core.  Exact parser
materialization and Agent summaries are independently content-bound semantic
overlays.  Publishing either overlay cannot mutate the Search topology
identity for the same content generation.
-/

namespace ASPProof.SearchTopologySemanticOverlayIndependence

structure SearchCoreIdentity where
  contentGenerationDigest : Nat
  sourceSnapshotDigest : Nat
  providerCatalogDigest : Nat
  topologyDigest : Nat
  deriving DecidableEq, Repr

structure QueryOverlay where
  contentGenerationDigest : Nat
  selectorDigest : Nat
  projectionDigest : Nat
  deriving DecidableEq, Repr

structure AgentOrgSummaryArtifact where
  contentGenerationDigest : Nat
  baseTopologyDigest : Nat
  selectorDigest : Nat
  modelIdentityDigest : Nat
  promptDigest : Nat
  summaryDigest : Nat
  orgAstDigest : Nat
  overlayDigest : Nat
  deriving DecidableEq, Repr

structure RuntimeSemanticState where
  searchCore : SearchCoreIdentity
  queryOverlay : Option QueryOverlay
  summaryOverlay : Option AgentOrgSummaryArtifact
  deriving DecidableEq, Repr

def queryOverlayAdmitted
    (core : SearchCoreIdentity) (overlay : QueryOverlay) : Bool :=
  overlay.contentGenerationDigest == core.contentGenerationDigest

def summaryOverlayAdmitted
    (core : SearchCoreIdentity) (artifact : AgentOrgSummaryArtifact) : Bool :=
  artifact.contentGenerationDigest == core.contentGenerationDigest &&
    artifact.baseTopologyDigest == core.topologyDigest &&
    artifact.selectorDigest != 0 &&
    artifact.modelIdentityDigest != 0 &&
    artifact.promptDigest != 0 &&
    artifact.summaryDigest != 0 &&
    artifact.orgAstDigest != 0 &&
    artifact.overlayDigest != 0

def publishQueryOverlay
    (state : RuntimeSemanticState) (overlay : QueryOverlay) : RuntimeSemanticState :=
  if queryOverlayAdmitted state.searchCore overlay then
    { state with queryOverlay := some overlay }
  else state

def publishSummaryOverlay
    (state : RuntimeSemanticState)
    (artifact : AgentOrgSummaryArtifact) : RuntimeSemanticState :=
  if summaryOverlayAdmitted state.searchCore artifact then
    { state with summaryOverlay := some artifact }
  else state

def searchTopologyIdentity (state : RuntimeSemanticState) : Nat × Nat × Nat × Nat :=
  (state.searchCore.contentGenerationDigest,
    state.searchCore.sourceSnapshotDigest,
    state.searchCore.providerCatalogDigest,
    state.searchCore.topologyDigest)

theorem query_overlay_preserves_search_topology
    (state : RuntimeSemanticState) (overlay : QueryOverlay) :
    searchTopologyIdentity (publishQueryOverlay state overlay) =
      searchTopologyIdentity state := by
  by_cases admitted : queryOverlayAdmitted state.searchCore overlay = true
  · simp [searchTopologyIdentity, publishQueryOverlay, admitted]
  · simp [searchTopologyIdentity, publishQueryOverlay, admitted]

theorem agent_org_summary_preserves_search_topology
    (state : RuntimeSemanticState) (artifact : AgentOrgSummaryArtifact) :
    searchTopologyIdentity (publishSummaryOverlay state artifact) =
      searchTopologyIdentity state := by
  by_cases admitted : summaryOverlayAdmitted state.searchCore artifact = true
  · simp [searchTopologyIdentity, publishSummaryOverlay, admitted]
  · simp [searchTopologyIdentity, publishSummaryOverlay, admitted]

theorem query_and_summary_publication_commute
    (state : RuntimeSemanticState)
    (query : QueryOverlay)
    (summary : AgentOrgSummaryArtifact) :
    publishSummaryOverlay (publishQueryOverlay state query) summary =
      publishQueryOverlay (publishSummaryOverlay state summary) query := by
  by_cases queryAdmitted : queryOverlayAdmitted state.searchCore query = true
  · by_cases summaryAdmitted :
        summaryOverlayAdmitted state.searchCore summary = true
    · simp [publishSummaryOverlay, publishQueryOverlay, queryAdmitted,
        summaryAdmitted]
    · simp [publishSummaryOverlay, publishQueryOverlay, queryAdmitted,
        summaryAdmitted]
  · by_cases summaryAdmitted :
        summaryOverlayAdmitted state.searchCore summary = true
    · simp [publishSummaryOverlay, publishQueryOverlay, queryAdmitted,
        summaryAdmitted]
    · simp [publishSummaryOverlay, publishQueryOverlay, queryAdmitted,
        summaryAdmitted]

def coreA : SearchCoreIdentity := ⟨11, 12, 13, 14⟩
def stateA : RuntimeSemanticState := ⟨coreA, none, none⟩
def queryA : QueryOverlay := ⟨11, 21, 22⟩
def summaryA : AgentOrgSummaryArtifact := ⟨11, 14, 21, 31, 32, 33, 34, 35⟩
def staleSummary : AgentOrgSummaryArtifact := ⟨99, 14, 21, 31, 32, 33, 34, 35⟩
def wrongBaseTopology : AgentOrgSummaryArtifact := ⟨11, 99, 21, 31, 32, 33, 34, 35⟩

theorem matching_summary_is_admitted :
    summaryOverlayAdmitted coreA summaryA = true := by
  decide

theorem stale_summary_is_rejected :
    summaryOverlayAdmitted coreA staleSummary = false := by
  decide

theorem overlay_for_another_base_topology_is_rejected :
    summaryOverlayAdmitted coreA wrongBaseTopology = false := by
  decide

theorem stale_summary_cannot_replace_an_admitted_overlay :
    let admitted := publishSummaryOverlay stateA summaryA
    publishSummaryOverlay admitted staleSummary = admitted := by
  decide

theorem summary_requires_an_org_ast_digest :
    summaryOverlayAdmitted coreA { summaryA with orgAstDigest := 0 } = false := by
  decide

theorem summary_requires_an_overlay_digest :
    summaryOverlayAdmitted coreA { summaryA with overlayDigest := 0 } = false := by
  decide

end ASPProof.SearchTopologySemanticOverlayIndependence
