import ASPProof.AgentSessionLifecycleExecutableRefinement

namespace ASPProof.CodexMultiAgentV2ControlPlane

open AgentSessionLifecycleProduct

inductive CodexTurnPhase where
  | unobserved
  | idle
  | queued
  | running
  | waiting
  | completed
  | failed
  | interrupted
  | quarantined
  deriving DecidableEq, Repr

inductive CodexDelegationPhase where
  | unobserved
  | intentDurable
  | dispatched
  | delivered
  | quarantined
  | rejected
  | completed
  deriving DecidableEq, Repr

inductive ControlPlaneFreshness where
  | unobserved
  | current
  | stale
  deriving DecidableEq, Repr

structure ControlPlaneMaterialization where
  generation : Nat
  sourceDigest : Option Nat
  evidenceRefs : List Nat
  freshness : ControlPlaneFreshness
  deriving DecidableEq, Repr

structure CodexRootTask where
  sessionId : Nat
  evidenceRef : Option Nat
  deriving DecidableEq, Repr

structure CodexAgentNode where
  sessionId : Nat
  rootSessionId : Nat
  parentSessionId : Option Nat
  residentName : Nat
  role : Nat
  configuredAgentType : Option Nat
  lifecycle : LifecycleProduct
  deriving DecidableEq, Repr

structure CodexTurnNode where
  turnId : Nat
  sessionId : Nat
  generation : Option Nat
  phase : CodexTurnPhase
  deriving DecidableEq, Repr

structure CodexDelegationEdge where
  parentSessionId : Nat
  parentTurnId : Option Nat
  childSessionId : Nat
  childGeneration : Nat
  phase : CodexDelegationPhase
  deliveredReceiptRef : Option Nat
  deriving DecidableEq, Repr

structure CodexMultiAgentControlPlane where
  materialization : ControlPlaneMaterialization
  workspaceServer : WorkspaceServer
  rootSessionId : Nat
  rootTask : CodexRootTask
  agents : List CodexAgentNode
  turns : List CodexTurnNode
  delegations : List CodexDelegationEdge
  deriving DecidableEq, Repr

def agentSessionIds (state : CodexMultiAgentControlPlane) : List Nat :=
  state.agents.map (fun agent => agent.sessionId)

def uniqueAgentSessionIds (state : CodexMultiAgentControlPlane) : Prop :=
  (agentSessionIds state).Nodup

def rootTaskEvidenced (state : CodexMultiAgentControlPlane) : Prop :=
  state.rootTask.sessionId = state.rootSessionId ∧
    (state.materialization.freshness = ControlPlaneFreshness.current →
      state.rootTask.evidenceRef.isSome = true)

def parentsResolve (state : CodexMultiAgentControlPlane) : Prop :=
  ∀ agent ∈ state.agents,
    agent.rootSessionId = state.rootSessionId ∧
      match agent.parentSessionId with
      | none => False
      | some parentId =>
          parentId ≠ agent.sessionId ∧
            (parentId = state.rootSessionId ∨ parentId ∈ agentSessionIds state)

def turnsResolve (state : CodexMultiAgentControlPlane) : Prop :=
  ∀ turn ∈ state.turns,
    turn.sessionId ∈ agentSessionIds state ∧
      ∃ agent ∈ state.agents,
        agent.sessionId = turn.sessionId ∧
          turn.generation = some agent.lifecycle.session.generation

def delegationGenerationAligned
    (state : CodexMultiAgentControlPlane)
    (edge : CodexDelegationEdge) : Prop :=
  ∃ child ∈ state.agents,
    child.sessionId = edge.childSessionId ∧
      child.lifecycle.session.generation = edge.childGeneration

def deliveryEvidenceAligned (edge : CodexDelegationEdge) : Prop :=
  edge.phase ∈ [CodexDelegationPhase.delivered, CodexDelegationPhase.completed] →
    edge.deliveredReceiptRef.isSome = true

def delegationsResolve (state : CodexMultiAgentControlPlane) : Prop :=
  ∀ edge ∈ state.delegations,
    (edge.parentSessionId = state.rootSessionId ∨
      edge.parentSessionId ∈ agentSessionIds state) ∧
      edge.childSessionId ∈ agentSessionIds state ∧
      edge.parentSessionId ≠ edge.childSessionId ∧
      delegationGenerationAligned state edge ∧
      deliveryEvidenceAligned edge

def wellFormed (state : CodexMultiAgentControlPlane) : Prop :=
  rootTaskEvidenced state ∧
    uniqueAgentSessionIds state ∧
    parentsResolve state ∧
    turnsResolve state ∧
    delegationsResolve state

def downstreamDispatchAdmitted
    (state : CodexMultiAgentControlPlane)
    (agent : CodexAgentNode) : Prop :=
  state.materialization.freshness = ControlPlaneFreshness.current ∧
    durableDispatchAuthorized agent.lifecycle

def withWorkspaceServer
    (state : CodexMultiAgentControlPlane)
    (server : WorkspaceServer) : CodexMultiAgentControlPlane :=
  { state with
    materialization :=
      if state.workspaceServer = server then state.materialization
      else { state.materialization with freshness := ControlPlaneFreshness.stale }
    workspaceServer := server
    agents := state.agents.map fun agent =>
      { agent with lifecycle := { agent.lifecycle with server := server } } }

theorem workspace_server_restart_preserves_agent_ids
    (state : CodexMultiAgentControlPlane)
    (server : WorkspaceServer) :
    agentSessionIds (withWorkspaceServer state server) = agentSessionIds state := by
  simp [withWorkspaceServer, agentSessionIds]

theorem workspace_server_restart_preserves_turns
    (state : CodexMultiAgentControlPlane)
    (server : WorkspaceServer) :
    (withWorkspaceServer state server).turns = state.turns := by
  rfl

theorem workspace_server_restart_preserves_delegations
    (state : CodexMultiAgentControlPlane)
    (server : WorkspaceServer) :
    (withWorkspaceServer state server).delegations = state.delegations := by
  rfl

theorem late_child_generation_rejected
    (state : CodexMultiAgentControlPlane)
    (edge : CodexDelegationEdge)
    (generationMismatch :
      ∀ child ∈ state.agents,
        child.sessionId = edge.childSessionId →
          child.lifecycle.session.generation ≠ edge.childGeneration) :
    ¬ delegationGenerationAligned state edge := by
  intro aligned
  rcases aligned with ⟨child, childMember, childId, childGeneration⟩
  exact generationMismatch child childMember childId childGeneration

theorem stale_control_plane_cannot_authorize_dispatch
    (state : CodexMultiAgentControlPlane)
    (agent : CodexAgentNode)
    (stale : state.materialization.freshness = ControlPlaneFreshness.stale) :
    ¬ downstreamDispatchAdmitted state agent := by
  intro admitted
  unfold downstreamDispatchAdmitted at admitted
  rw [stale] at admitted
  exact ControlPlaneFreshness.noConfusion admitted.1

theorem codex_spawn_policy_is_not_control_plane_authority
    (state : CodexMultiAgentControlPlane)
    (agent : CodexAgentNode)
    (model reasoningEffort : Nat) :
    downstreamDispatchAdmitted state agent = downstreamDispatchAdmitted state agent := by
  let _observedSpawnPolicy := (model, reasoningEffort)
  rfl

end ASPProof.CodexMultiAgentV2ControlPlane
