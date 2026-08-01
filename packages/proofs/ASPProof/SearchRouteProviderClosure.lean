import Std

namespace ASPProof.SearchRouteProviderClosure

/-!
Executable model for RFC 10.05.62 and 10.05.63.

The model separates operation-scoped provider closure from installation and
registry membership.  It also carries finite counterexamples for native-owner
schema drift and synthetic fingerprint time assumptions.
-/

universe u

structure Operation (Provider : Type u) where
  selected : Provider
  required : Provider → Prop
  selectedRequired : required selected
  singletonClosure : ∀ provider, required provider → provider = selected

def Admitted {Provider : Type u}
    (operation : Operation Provider)
    (ready : Provider → Prop) : Prop :=
  ∀ provider, operation.required provider → ready provider

theorem admitted_iff_selected
    {Provider : Type u}
    (operation : Operation Provider)
    (ready : Provider → Prop) :
    Admitted operation ready ↔ ready operation.selected := by
  constructor
  · intro admitted
    exact admitted operation.selected operation.selectedRequired
  · intro selectedReady provider required
    rw [operation.singletonClosure provider required]
    exact selectedReady

theorem unrelated_failure_noninterference
    {Provider : Type u}
    [DecidableEq Provider]
    (operation : Operation Provider)
    (ready : Provider → Prop)
    (unrelated : Provider)
    (unrelatedNe : unrelated ≠ operation.selected) :
    Admitted operation (fun provider =>
      if provider = unrelated then False else ready provider) ↔
      Admitted operation ready := by
  constructor
  · intro admitted provider required
    have providerEq := operation.singletonClosure provider required
    subst provider
    have selectedNe : operation.selected ≠ unrelated := Ne.symm unrelatedNe
    have selectedReady := admitted operation.selected operation.selectedRequired
    dsimp at selectedReady
    rw [if_neg selectedNe] at selectedReady
    exact selectedReady
  · intro admitted provider required
    have providerEq := operation.singletonClosure provider required
    subst provider
    have selectedNe : operation.selected ≠ unrelated := Ne.symm unrelatedNe
    have selectedReady := admitted operation.selected operation.selectedRequired
    rw [if_neg selectedNe]
    exact selectedReady

inductive OwnerField where
  | projectionMode
  | query
  | itemMode
  | requestedProjectionMode
  | requestedQuery
  deriving DecidableEq, Repr

inductive SchemaRequestField : OwnerField → Prop where
  | projectionMode : SchemaRequestField .projectionMode

inductive RuntimeRequestField : OwnerField → Prop where
  | query : RuntimeRequestField .query
  | itemMode : RuntimeRequestField .itemMode

inductive SchemaResponseField : OwnerField → Prop where
  | requestedProjectionMode :
      SchemaResponseField .requestedProjectionMode

inductive RuntimeResponseField : OwnerField → Prop where
  | requestedQuery : RuntimeResponseField .requestedQuery

def SchemaTransportCongruent : Prop :=
  (∀ field, SchemaRequestField field ↔ RuntimeRequestField field) ∧
  (∀ field, SchemaResponseField field ↔ RuntimeResponseField field)

theorem schema_runtime_drift_witness :
    SchemaRequestField .projectionMode ∧
    ¬ RuntimeRequestField .projectionMode := by
  constructor
  · exact .projectionMode
  · intro impossible
    cases impossible

theorem response_schema_runtime_drift_witness :
    SchemaResponseField .requestedProjectionMode ∧
    ¬ RuntimeResponseField .requestedProjectionMode := by
  constructor
  · exact .requestedProjectionMode
  · intro impossible
    cases impossible

theorem current_native_owner_contract_is_not_congruent :
    ¬ SchemaTransportCongruent := by
  intro congruent
  have runtimeMember :=
    (congruent.1 OwnerField.projectionMode).mp
      SchemaRequestField.projectionMode
  cases runtimeMember

structure NativeOwnerAdmission where
  manifestExplicit : Bool
  descriptorRegistered : Bool
  schemaCongruent : Bool
  deriving DecidableEq, Repr

def ClosedNativeOwnerAdmission (admission : NativeOwnerAdmission) : Prop :=
  admission.manifestExplicit = true ∧
  admission.descriptorRegistered = true ∧
  admission.schemaCongruent = true

def descriptorOnlyAdmission : NativeOwnerAdmission where
  manifestExplicit := false
  descriptorRegistered := true
  schemaCongruent := false

theorem descriptor_presence_does_not_close_admission :
    ¬ ClosedNativeOwnerAdmission descriptorOnlyAdmission := by
  intro admitted
  have manifestEquality := admitted.1
  change false = true at manifestEquality
  cases manifestEquality

def RegistrySurfaceCongruent
    {Method : Type u}
    (declared described : Method → Prop) : Prop :=
  ∀ method, declared method ↔ described method

theorem descriptor_without_method_breaks_registry_congruence
    {Method : Type u}
    (declared described : Method → Prop)
    (method : Method)
    (describedMethod : described method)
    (missingDeclaredMethod : ¬ declared method) :
    ¬ RegistrySurfaceCongruent declared described := by
  intro congruent
  exact missingDeclaredMethod ((congruent method).mpr describedMethod)

structure Fingerprint where
  contentDigest : Nat
  sizeBytes : Nat
  modifiedUnixNanos : Nat
  changeTimeUnixNanos : Nat
  deriving DecidableEq, Repr

def syntheticFingerprint (digest size : Nat) : Fingerprint where
  contentDigest := digest
  sizeBytes := size
  modifiedUnixNanos := 0
  changeTimeUnixNanos := 0

def ContentAuthority
    (fingerprint : Fingerprint)
    (sourceDigest sourceSize : Nat) : Prop :=
  fingerprint.contentDigest = sourceDigest ∧
  fingerprint.sizeBytes = sourceSize

def PositiveFilesystemTimes (fingerprint : Fingerprint) : Prop :=
  0 < fingerprint.modifiedUnixNanos ∧
  0 < fingerprint.changeTimeUnixNanos

theorem authenticated_content_admits_zero_times (digest size : Nat) :
    ContentAuthority (syntheticFingerprint digest size) digest size := by
  exact ⟨rfl, rfl⟩

theorem positive_time_requirement_rejects_synthetic_owner
    (digest size : Nat) :
    ¬ PositiveFilesystemTimes (syntheticFingerprint digest size) := by
  intro positiveTimes
  exact Nat.lt_irrefl 0 positiveTimes.1

end ASPProof.SearchRouteProviderClosure
