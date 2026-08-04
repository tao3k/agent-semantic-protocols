namespace ASPProof.CodexMultiAgentV2RegistryMaterialization

structure RegistryAgent where
  sessionId : String
  parentSessionId : Option String
  generation : Nat
deriving DecidableEq

structure RegistrySnapshot where
  rootSessionId : String
  agents : List RegistryAgent

structure ControlPlaneProjection where
  agents : List RegistryAgent
  turns : List String
  delegations : List (String × String)
deriving DecidableEq

def durableRootCount (snapshot : RegistrySnapshot) : Nat :=
  (snapshot.agents.filter fun agent =>
    decide (agent.sessionId = snapshot.rootSessionId ∧ agent.parentSessionId = none)).length

def materialize (snapshot : RegistrySnapshot) : Option ControlPlaneProjection :=
  if durableRootCount snapshot = 1 then
    some {
      agents := snapshot.agents
      turns := []
      delegations := []
    }
  else
    none

theorem nonUniqueDurableRootRejects (snapshot : RegistrySnapshot)
    (nonUnique : durableRootCount snapshot ≠ 1) :
    materialize snapshot = none := by
  simp [materialize, nonUnique]

theorem registryFactsMintNoTurns
    (snapshot : RegistrySnapshot)
    (projection : ControlPlaneProjection)
    (published : materialize snapshot = some projection) :
    projection.turns = [] := by
  unfold materialize at published
  split at published
  · cases published
    rfl
  · contradiction

theorem registryFactsMintNoDelegations
    (snapshot : RegistrySnapshot)
    (projection : ControlPlaneProjection)
    (published : materialize snapshot = some projection) :
    projection.delegations = [] := by
  unfold materialize at published
  split at published
  · cases published
    rfl
  · contradiction

theorem registryMaterializationPreservesOnlyAgentFacts
    (snapshot : RegistrySnapshot)
    (projection : ControlPlaneProjection)
    (published : materialize snapshot = some projection) :
    projection.agents = snapshot.agents := by
  unfold materialize at published
  split at published
  · cases published
    rfl
  · contradiction

end ASPProof.CodexMultiAgentV2RegistryMaterialization
