namespace ASPProof.CallableSkeletonGenerationAdmission

inductive ProjectionKind where
  | source
  | callableSkeleton
  deriving DecidableEq

inductive ExactProjectionMode where
  | code
  | skeleton
  | names
  | verbatim
  deriving DecidableEq

def publicKindFor : ExactProjectionMode → Option ProjectionKind
  | .code => some .source
  | .skeleton => some .callableSkeleton
  | .names => none
  | .verbatim => none

structure Snapshot where
  selectorPresent : Bool
  derivedProjectionPresent : Bool
  requestedKind : ProjectionKind
  storedKind : ProjectionKind
  activeGeneration : Nat
  projectionGeneration : Nat
  schemaValid : Bool

def admitted (snapshot : Snapshot) : Prop :=
  snapshot.selectorPresent = true ∧
    snapshot.derivedProjectionPresent = true ∧
    snapshot.requestedKind = snapshot.storedKind ∧
    snapshot.activeGeneration = snapshot.projectionGeneration ∧
    snapshot.schemaValid = true

def repairAdmissible (activeGeneration candidateGeneration : Nat)
    (schemaValid : Bool) : Prop :=
  activeGeneration = candidateGeneration ∧ schemaValid = true

theorem admission_implies_selector_present (snapshot : Snapshot)
    (proof : admitted snapshot) : snapshot.selectorPresent = true :=
  proof.1

theorem admission_implies_requested_projection_kind (snapshot : Snapshot)
    (proof : admitted snapshot) : snapshot.requestedKind = snapshot.storedKind :=
  proof.2.2.1

theorem admission_implies_same_generation (snapshot : Snapshot)
    (proof : admitted snapshot) :
    snapshot.activeGeneration = snapshot.projectionGeneration :=
  proof.2.2.2.1

theorem missing_projection_is_not_admitted (snapshot : Snapshot)
    (missing : snapshot.derivedProjectionPresent = false) :
    ¬ admitted snapshot := by
  intro proof
  have present : snapshot.derivedProjectionPresent = true := proof.2.1
  exact Bool.noConfusion (missing.symm.trans present)

theorem schema_failure_is_not_admitted (snapshot : Snapshot)
    (invalid : snapshot.schemaValid = false) :
    ¬ admitted snapshot := by
  intro proof
  have valid : snapshot.schemaValid = true := proof.2.2.2.2
  exact Bool.noConfusion (invalid.symm.trans valid)

theorem stale_repair_is_not_admissible (activeGeneration candidateGeneration : Nat)
    (stale : activeGeneration ≠ candidateGeneration) (schemaValid : Bool) :
    ¬ repairAdmissible activeGeneration candidateGeneration schemaValid := by
  intro proof
  exact stale proof.1

theorem repair_admission_implies_same_generation
    (activeGeneration candidateGeneration : Nat) (schemaValid : Bool)
    (proof : repairAdmissible activeGeneration candidateGeneration schemaValid) :
    activeGeneration = candidateGeneration :=
  proof.1

theorem skeleton_cache_mode_maps_to_callable_skeleton :
    publicKindFor .skeleton = some .callableSkeleton :=
  rfl

theorem code_cache_mode_is_not_callable_skeleton :
    publicKindFor .code ≠ some .callableSkeleton := by
  decide

end ASPProof.CallableSkeletonGenerationAdmission
