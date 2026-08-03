import ASPProof.CodexMultiAgentV2ControlPlane

namespace ASPProof.Audit.CodexMultiAgentV2ControlPlane

open ASPProof.AgentSessionLifecycleProduct
open ASPProof.CodexMultiAgentV2ControlPlane

example
    (state : CodexMultiAgentControlPlane)
    (agent : CodexAgentNode)
    (member : agent ∈ state.agents)
    (parentId : Nat)
    (parentBinding : agent.parentSessionId = some parentId)
    (missingParent : parentId ∉ agentSessionIds state) :
    ¬ parentsResolve state := by
  intro resolves
  have resolved := resolves agent member
  rw [parentBinding] at resolved
  exact missingParent resolved.2.2

example
    (edge : CodexDelegationEdge)
    (delivered : edge.phase = CodexDelegationPhase.delivered)
    (missingReceipt : edge.deliveredReceiptRef = none) :
    ¬ deliveryEvidenceAligned edge := by
  intro aligned
  have receipt := aligned (by simp [delivered])
  simp [missingReceipt] at receipt

example
    (state : CodexMultiAgentControlPlane)
    (server : WorkspaceServer) :
    (withWorkspaceServer state server).turns = state.turns ∧
      (withWorkspaceServer state server).delegations = state.delegations := by
  exact ⟨rfl, rfl⟩

example
    (state : CodexMultiAgentControlPlane)
    (agent : CodexAgentNode)
    (stale : state.materialization.freshness = ControlPlaneFreshness.stale) :
    ¬ downstreamDispatchAdmitted state agent := by
  exact stale_control_plane_cannot_authorize_dispatch state agent stale

end ASPProof.Audit.CodexMultiAgentV2ControlPlane
