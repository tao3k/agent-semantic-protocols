namespace ASPProof.RuntimeSelectorOverlay

structure OverlayAdmission where
  ownerFreshnessProved : Bool
  generationReady : Bool
  ownerDigestMatches : Bool
  byteRangeValid : Bool
  providerResponseValid : Bool
  projectionKindBound : Bool
  projectionBytesBound : Bool
  deriving DecidableEq, Repr

def Admissible (admission : OverlayAdmission) : Prop :=
  admission.ownerFreshnessProved = true ∧
  admission.generationReady = true ∧
  admission.ownerDigestMatches = true ∧
  admission.byteRangeValid = true ∧
  admission.providerResponseValid = true ∧
  admission.projectionKindBound = true ∧
  admission.projectionBytesBound = true

instance admissibleDecidable (admission : OverlayAdmission) :
    Decidable (Admissible admission) := by
  unfold Admissible
  infer_instance

inductive Publication where
  | rejected
  | published
  deriving DecidableEq, Repr

def publish (admission : OverlayAdmission) : Publication :=
  if Admissible admission then .published else .rejected

structure OverlayKey where
  projectionKind : String
  structuralSelector : String
  deriving DecidableEq, Repr

structure RuntimeGeneration where
  sourceEpoch : Nat
  selectors : List OverlayKey
  deriving DecidableEq, Repr

structure ResidentReadyResult where
  generation : RuntimeGeneration
  durableOpens : Nat
  providerSpawns : Nat
  deriving DecidableEq, Repr

def admitResidentReady (generation : RuntimeGeneration) : ResidentReadyResult :=
  { generation
    durableOpens := 0
    providerSpawns := 0 }

inductive RuntimeEntryPoint where
  | sessionStart
  | userPrompt
  | preTool
  | exactQuery
  | searchQuery
  deriving DecidableEq, Repr

def requestsGenerationAdmission : RuntimeEntryPoint → Bool
  | .sessionStart
  | .userPrompt
  | .preTool => true
  | .exactQuery
  | .searchQuery => false

structure AdmissionScope where
  workspaceIdentity : String
  canonicalProjectRoot : String
  deriving DecidableEq, Repr

structure CanonicalMaterialization where
  identityDigest : String
  deriving DecidableEq, Repr

structure DurableCommitReceipt where
  committed : Bool
  materializationIdentityDigest : String
  deriving DecidableEq, Repr

def MayPublishCommitted
    (receipt : DurableCommitReceipt)
    (materialization : CanonicalMaterialization) : Prop :=
  receipt.committed = true ∧
    receipt.materializationIdentityDigest = materialization.identityDigest

def applySelector
    (generation : RuntimeGeneration)
    (key : OverlayKey) : RuntimeGeneration :=
  { generation with selectors := key :: generation.selectors }

def checkout (generation : RuntimeGeneration) : RuntimeGeneration :=
  generation

def applyOwner (generation : RuntimeGeneration) : RuntimeGeneration :=
  { sourceEpoch := generation.sourceEpoch + 1
    selectors := [] }

def MayServe (publication : Publication) : Prop :=
  publication = .published

theorem publish_iff_complete_admission (admission : OverlayAdmission) :
    publish admission = .published ↔ Admissible admission := by
  simp [publish]

theorem invalid_overlay_cannot_serve
    (admission : OverlayAdmission)
    (invalid : ¬ Admissible admission) :
    ¬ MayServe (publish admission) := by
  simp [MayServe, publish, invalid]

theorem stale_owner_cannot_serve
    (admission : OverlayAdmission)
    (stale : admission.ownerFreshnessProved = false) :
    ¬ MayServe (publish admission) := by
  intro served
  have published : publish admission = .published := served
  have admissible := (publish_iff_complete_admission admission).mp published
  exact Bool.noConfusion (stale.symm.trans admissible.1)

theorem selector_only_publication_preserves_source_epoch
    (generation : RuntimeGeneration)
    (key : OverlayKey) :
    (applySelector generation key).sourceEpoch = generation.sourceEpoch := by
  rfl

theorem owner_publication_invalidates_selector_overlays
    (generation : RuntimeGeneration) :
    (applyOwner generation).selectors = [] := by
  rfl

theorem prior_lease_remains_snapshot_isolated
    (generation : RuntimeGeneration) :
    let priorLease := checkout generation
    let _currentGeneration := applyOwner generation
    priorLease.sourceEpoch = generation.sourceEpoch ∧
      priorLease.selectors = generation.selectors := by
  simp [checkout]

theorem current_generation_advances_after_owner_publication
    (generation : RuntimeGeneration) :
    (checkout (applyOwner generation)).sourceEpoch =
      (checkout generation).sourceEpoch + 1 := by
  rfl

theorem resident_ready_admission_preserves_generation_and_overlays
    (generation : RuntimeGeneration) :
    (admitResidentReady generation).generation = generation ∧
      (admitResidentReady generation).durableOpens = 0 ∧
      (admitResidentReady generation).providerSpawns = 0 := by
  simp [admitResidentReady]

theorem pre_tool_requests_generation_admission :
    requestsGenerationAdmission .preTool = true := by
  rfl

theorem exact_query_cannot_request_generation_admission :
    requestsGenerationAdmission .exactQuery = false := by
  rfl

theorem different_project_roots_have_distinct_admission_scopes
    (workspaceIdentity firstRoot secondRoot : String)
    (different : firstRoot ≠ secondRoot) :
    AdmissionScope.mk workspaceIdentity firstRoot ≠
      AdmissionScope.mk workspaceIdentity secondRoot := by
  intro equalScopes
  exact different (congrArg AdmissionScope.canonicalProjectRoot equalScopes)

theorem committed_materialization_can_publish_without_cross_connection_readback
    (materialization : CanonicalMaterialization) :
    MayPublishCommitted
      { committed := true
        materializationIdentityDigest := materialization.identityDigest }
      materialization := by
  simp [MayPublishCommitted]

theorem identity_drift_cannot_publish_committed_materialization
    (receipt : DurableCommitReceipt)
    (materialization : CanonicalMaterialization)
    (drift :
      receipt.materializationIdentityDigest ≠ materialization.identityDigest) :
    ¬ MayPublishCommitted receipt materialization := by
  intro publishable
  exact drift publishable.2

theorem projection_kind_separates_overlay_keys
    (sourceKind skeletonKind selector : String)
    (different : sourceKind ≠ skeletonKind) :
    OverlayKey.mk sourceKind selector ≠ OverlayKey.mk skeletonKind selector := by
  intro equalKeys
  exact different (congrArg OverlayKey.projectionKind equalKeys)

theorem serve_requires_successful_publication (publication : Publication) :
    MayServe publication → publication = .published := by
  intro served
  exact served

end ASPProof.RuntimeSelectorOverlay
