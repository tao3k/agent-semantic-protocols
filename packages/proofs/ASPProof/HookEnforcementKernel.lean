namespace ASPProof.HookEnforcementKernel

inductive Decision where
  | allow
  | deny
  | block
  deriving DecidableEq, Repr

inductive Effect where
  | provedNoSource
  | sourceRead
  | sourceSearch
  | sourceExecute
  | sourcePatch
  | sourceMaterialize
  | unknownSourceLike
  deriving DecidableEq, Repr

inductive ControlPlaneState where
  | healthy
  | endpointMissing
  | supervisorUnavailable
  | residentUnroutable
  deriving DecidableEq, Repr

structure AgentAction where
  effect : Effect
  deriving DecidableEq, Repr

structure PolicySnapshot where
  providerExtensions : List String
  synchronousDependencyCount : Nat
  decide : AgentAction → Decision

def SnapshotAdmissible (snapshot : PolicySnapshot) (expectedExtensions : List String) : Prop :=
  snapshot.providerExtensions = expectedExtensions ∧ snapshot.synchronousDependencyCount = 0

def fallbackDecision (action : AgentAction) : Decision :=
  match action.effect with
  | .provedNoSource => .allow
  | .sourceRead
  | .sourceSearch
  | .sourceExecute
  | .sourcePatch
  | .sourceMaterialize
  | .unknownSourceLike => .deny

def enforce (lastKnownGood : Option PolicySnapshot) (action : AgentAction) : Decision :=
  match lastKnownGood with
  | some snapshot => snapshot.decide action
  | none => fallbackDecision action

def enforceWithControlPlane
    (_controlPlane : ControlPlaneState)
    (lastKnownGood : Option PolicySnapshot)
    (action : AgentAction) : Decision :=
  enforce lastKnownGood action

def liveDecision := enforce

def doctorDecision := enforce

theorem control_plane_unavailability_cannot_change_decision
    (left right : ControlPlaneState)
    (snapshot : Option PolicySnapshot)
    (action : AgentAction) :
    enforceWithControlPlane left snapshot action =
      enforceWithControlPlane right snapshot action := by
  rfl

theorem live_and_doctor_replay_are_equivalent
    (snapshot : Option PolicySnapshot)
    (action : AgentAction) :
    liveDecision snapshot action = doctorDecision snapshot action := by
  rfl

theorem last_known_good_deny_is_preserved
    (snapshot : PolicySnapshot)
    (action : AgentAction)
    (hDeny : snapshot.decide action = .deny) :
    enforce (some snapshot) action = .deny := by
  simpa [enforce] using hDeny

theorem missing_snapshot_denies_unknown_source_like :
    enforce none ⟨.unknownSourceLike⟩ = .deny := by
  rfl

theorem missing_snapshot_denies_source_materialization :
    enforce none ⟨.sourceMaterialize⟩ = .deny := by
  rfl

theorem incomplete_provider_projection_cannot_be_admitted
    (snapshot : PolicySnapshot)
    (expectedExtensions : List String)
    (hIncomplete : snapshot.providerExtensions ≠ expectedExtensions) :
    ¬ SnapshotAdmissible snapshot expectedExtensions := by
  intro hAdmissible
  exact hIncomplete hAdmissible.1

theorem synchronous_control_plane_dependency_cannot_be_admitted
    (snapshot : PolicySnapshot)
    (expectedExtensions : List String)
    (hDependency : snapshot.synchronousDependencyCount ≠ 0) :
    ¬ SnapshotAdmissible snapshot expectedExtensions := by
  intro hAdmissible
  exact hDependency hAdmissible.2

inductive ActivePointer where
  | previous
  | next
  deriving DecidableEq, Repr

def observeActive
    (pointer : ActivePointer)
    (previous next : PolicySnapshot) : PolicySnapshot :=
  match pointer with
  | .previous => previous
  | .next => next

theorem atomic_pointer_observes_whole_generation
    (pointer : ActivePointer)
    (previous next : PolicySnapshot) :
    observeActive pointer previous next = previous ∨
      observeActive pointer previous next = next := by
  cases pointer <;> simp [observeActive]

def coupledDecision
    (controlPlane : ControlPlaneState)
    (action : AgentAction) : Decision :=
  match controlPlane with
  | .healthy => fallbackDecision action
  | .endpointMissing
  | .supervisorUnavailable
  | .residentUnroutable => .allow

theorem coupled_control_plane_has_fail_open_counterexample :
    coupledDecision .endpointMissing ⟨.sourceRead⟩ = .allow ∧
      fallbackDecision ⟨.sourceRead⟩ = .deny := by
  constructor <;> rfl

end ASPProof.HookEnforcementKernel
