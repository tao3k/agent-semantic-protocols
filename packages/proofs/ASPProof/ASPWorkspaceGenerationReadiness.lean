namespace ASPProof.ASPWorkspaceGenerationReadiness

inductive ProjectionMode where
  | source
  | callableSkeleton
  | seeds
  deriving DecidableEq, Repr

structure RuntimeServer where
  healthy : Bool
  runtimeGeneration : Nat
  deriving DecidableEq, Repr

structure WorkspacePublication where
  workspaceIdentity : Nat
  registered : Bool
  currentRootDigest : Nat
  activeGeneration : Option Nat
  activeRootDigest : Option Nat
  sourceProjection : Bool
  callableSkeletonProjection : Bool
  seedsProjection : Bool
  deriving DecidableEq, Repr

structure AgentAuthority where
  canonicalPath : Nat
  physicalGeneration : Nat
  bindingDelivered : Bool
  pathReleased : Bool
  deriving DecidableEq, Repr

structure SystemState where
  server : RuntimeServer
  workspace : WorkspacePublication
  agent : AgentAuthority
  deriving DecidableEq, Repr

def projectionAvailable
    (workspace : WorkspacePublication) (mode : ProjectionMode) : Bool :=
  match mode with
  | .source => workspace.sourceProjection
  | .callableSkeleton => workspace.callableSkeletonProjection
  | .seeds => workspace.seedsProjection

def exactQueryAdmitted
    (workspace : WorkspacePublication) (mode : ProjectionMode) : Prop :=
  ∃ generation,
    workspace.activeGeneration = some generation ∧
    workspace.activeRootDigest = some workspace.currentRootDigest ∧
    projectionAvailable workspace mode = true

def publishCanonicalGeneration
    (workspace : WorkspacePublication) (generation : Nat) : WorkspacePublication :=
  { workspace with
    registered := true
    activeGeneration := some generation
    activeRootDigest := some workspace.currentRootDigest
    sourceProjection := true
    callableSkeletonProjection := true
    seedsProjection := true }

def reportServerHealthy (state : SystemState) : SystemState :=
  { state with server := { state.server with healthy := true } }

def repairWorkspace
    (state : SystemState) (generation : Nat) : SystemState :=
  { state with workspace := publishCanonicalGeneration state.workspace generation }

def publishParentWorkspace
    (parent child : WorkspacePublication) (generation : Nat) :
    WorkspacePublication × WorkspacePublication :=
  (publishCanonicalGeneration parent generation, child)

theorem serverHealthPreservesMissingGeneration
    (state : SystemState)
    (hMissing : state.workspace.activeGeneration = none) :
    (reportServerHealthy state).workspace.activeGeneration = none := by
  exact hMissing

theorem registrationDoesNotCreateGeneration
    (workspace : WorkspacePublication)
    (hMissing : workspace.activeGeneration = none) :
    ({ workspace with registered := true }).activeGeneration = none := by
  exact hMissing

theorem parentPublicationLeavesNestedWorkspaceUnchanged
    (parent child : WorkspacePublication) (generation : Nat) :
    (publishParentWorkspace parent child generation).2 = child := by
  rfl

theorem missingGenerationRejectsExactProjection
    (workspace : WorkspacePublication) (mode : ProjectionMode)
    (hMissing : workspace.activeGeneration = none) :
    ¬exactQueryAdmitted workspace mode := by
  intro hAdmitted
  obtain ⟨generation, hGeneration, _, _⟩ := hAdmitted
  have hImpossible : (none : Option Nat) = some generation :=
    hMissing.symm.trans hGeneration
  cases hImpossible

theorem digestMismatchRejectsExactProjection
    (workspace : WorkspacePublication) (mode : ProjectionMode)
    (generation activeDigest : Nat)
    (_hGeneration : workspace.activeGeneration = some generation)
    (hActive : workspace.activeRootDigest = some activeDigest)
    (hMismatch : activeDigest ≠ workspace.currentRootDigest) :
    ¬exactQueryAdmitted workspace mode := by
  intro hAdmitted
  obtain ⟨_, _, hReceiptDigest, _⟩ := hAdmitted
  have hSome : some activeDigest = some workspace.currentRootDigest :=
    hActive.symm.trans hReceiptDigest
  exact hMismatch (Option.some.inj hSome)

theorem missingProjectionRejectsExactQuery
    (workspace : WorkspacePublication) (mode : ProjectionMode)
    (hProjection : projectionAvailable workspace mode = false) :
    ¬exactQueryAdmitted workspace mode := by
  intro hAdmitted
  obtain ⟨_, _, _, hProjectionReceipt⟩ := hAdmitted
  exact Bool.noConfusion (hProjection.symm.trans hProjectionReceipt)

theorem canonicalPublicationAdmitsEveryProjection
    (workspace : WorkspacePublication) (generation : Nat)
    (mode : ProjectionMode) :
    exactQueryAdmitted (publishCanonicalGeneration workspace generation) mode := by
  refine ⟨generation, rfl, rfl, ?_⟩
  cases mode <;> rfl

theorem workspaceRepairPreservesAgentAuthority
    (state : SystemState) (generation : Nat) :
    (repairWorkspace state generation).agent = state.agent := by
  rfl

end ASPProof.ASPWorkspaceGenerationReadiness
