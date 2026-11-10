-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

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
  rootTaskId : String
  agents : List RegistryAgent
  turns : List String
  delegations : List (String × String)
deriving DecidableEq

inductive RegistryControlPlaneRequest where
  | refresh
  | read
deriving DecidableEq

inductive CodexExecutionOperation where
  | spawn
  | resume
  | send
  | interrupt
  | archive
  | delete
deriving DecidableEq

def emittedExecutionOperation
    (_request : RegistryControlPlaneRequest) : Option CodexExecutionOperation :=
  none

def rootTaskEvidenceAdmissible (snapshot : RegistrySnapshot) : Bool :=
  !snapshot.agents.isEmpty &&
    snapshot.agents.all fun agent => decide (agent.sessionId ≠ snapshot.rootSessionId)

def materialize (snapshot : RegistrySnapshot) : Option ControlPlaneProjection :=
  if rootTaskEvidenceAdmissible snapshot then
    some {
      rootTaskId := snapshot.rootSessionId
      agents := snapshot.agents
      turns := []
      delegations := []
    }
  else
    none

theorem missingScopedRootTaskEvidenceRejects (snapshot : RegistrySnapshot)
    (missing : rootTaskEvidenceAdmissible snapshot = false) :
    materialize snapshot = none := by
  simp [materialize, missing]

theorem registryMaterializationPreservesRootTaskIdentity
    (snapshot : RegistrySnapshot)
    (projection : ControlPlaneProjection)
    (published : materialize snapshot = some projection) :
    projection.rootTaskId = snapshot.rootSessionId := by
  unfold materialize at published
  split at published
  · cases published
    rfl
  · contradiction

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

theorem registryControlPlaneDoesNotExecuteCodex
    (request : RegistryControlPlaneRequest) :
    emittedExecutionOperation request = none := by
  cases request <;> rfl

end ASPProof.CodexMultiAgentV2RegistryMaterialization
