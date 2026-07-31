namespace ASPProof.ActivationAdmission

structure AdmissionIdentity where
  activationReadable : Bool
  artifactReceiptValid : Bool
  schemaValid : Bool
  projectIdentityMatches : Bool
  providerSelectionMatches : Bool
  repositoryCandidateGenerationMatches : Bool
  deriving DecidableEq, Repr

inductive AdmissionDecision where
  | reuse
  | rebuildAndPublish
  deriving DecidableEq, Repr

def CompleteIdentity (identity : AdmissionIdentity) : Prop :=
  identity.activationReadable = true ∧
  identity.artifactReceiptValid = true ∧
  identity.schemaValid = true ∧
  identity.projectIdentityMatches = true ∧
  identity.providerSelectionMatches = true ∧
  identity.repositoryCandidateGenerationMatches = true

instance completeIdentityDecidable (identity : AdmissionIdentity) :
    Decidable (CompleteIdentity identity) := by
  unfold CompleteIdentity
  infer_instance

def decideAdmission (identity : AdmissionIdentity) : AdmissionDecision :=
  if CompleteIdentity identity then .reuse else .rebuildAndPublish

inductive PublicationResult where
  | publishedFresh
  | failed
  deriving DecidableEq, Repr

def MayServe
    (decision : AdmissionDecision)
    (publication : PublicationResult) : Prop :=
  decision = .reuse ∨ publication = .publishedFresh

theorem reuse_iff_complete_identity (identity : AdmissionIdentity) :
    decideAdmission identity = .reuse ↔ CompleteIdentity identity := by
  simp [decideAdmission]

theorem any_identity_drift_requires_rebuild
    (identity : AdmissionIdentity)
    (drift : ¬ CompleteIdentity identity) :
    decideAdmission identity = .rebuildAndPublish := by
  simp [decideAdmission, drift]

theorem failed_rebuild_cannot_serve_stale_activation
    (identity : AdmissionIdentity)
    (drift : ¬ CompleteIdentity identity) :
    ¬ MayServe (decideAdmission identity) .failed := by
  rw [any_identity_drift_requires_rebuild identity drift]
  simp [MayServe]

def missingActivation : AdmissionIdentity where
  activationReadable := false
  artifactReceiptValid := false
  schemaValid := false
  projectIdentityMatches := false
  providerSelectionMatches := false
  repositoryCandidateGenerationMatches := false

theorem missing_activation_requires_rebuild :
    decideAdmission missingActivation = .rebuildAndPublish := by
  apply any_identity_drift_requires_rebuild
  simp [CompleteIdentity, missingActivation]

structure ProviderClosure where
  logicalPaths : List String
  deriving DecidableEq, Repr

def ProviderClosureMatches
    (activation receipt : ProviderClosure) : Prop :=
  receipt = activation

def reconcileProviderClosure
    (activation _receipt : ProviderClosure) : ProviderClosure :=
  activation

theorem reconciled_provider_closure_matches_activation
    (activation receipt : ProviderClosure) :
    ProviderClosureMatches activation (reconcileProviderClosure activation receipt) := by
  rfl

theorem retired_provider_cannot_survive_reconciliation
    (activation receipt : ProviderClosure)
    (retired : String)
    (notActive : retired ∉ activation.logicalPaths) :
    retired ∉ (reconcileProviderClosure activation receipt).logicalPaths := by
  simpa [reconcileProviderClosure] using notActive

end ASPProof.ActivationAdmission
