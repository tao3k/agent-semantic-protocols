namespace ASPProof.RuntimeArtifactAuthority

inductive RuntimeMode where
  | dev (root : String)
  | release
  deriving DecidableEq, Repr

structure DevConfig where
  enabled : Bool
  root : String
  deriving DecidableEq, Repr

def runtimeMode (config : Option DevConfig) : RuntimeMode :=
  match config with
  | some config => if config.enabled then .dev config.root else .release
  | none => .release

inductive ArtifactOrigin where
  | developWorkspace
  | lockedRelease
  | pathFallback
  deriving DecidableEq, Repr

def originAdmitted : RuntimeMode → ArtifactOrigin → Bool
  | .dev _, .developWorkspace => true
  | .release, .lockedRelease => true
  | _, _ => false

theorem dev_never_admits_locked_release :
    originAdmitted (.dev "/checkout") .lockedRelease = false := by
  rfl

theorem dev_never_admits_path_fallback :
    originAdmitted (.dev "/checkout") .pathFallback = false := by
  rfl

theorem release_never_admits_develop_workspace :
    originAdmitted .release .developWorkspace = false := by
  rfl

structure ArtifactReceipt where
  origin : ArtifactOrigin
  sourceRoot : Option String
  sourceGenerationMatches : Bool
  binaryDigestMatches : Bool
  commandBindingDigestMatches : Bool
  deriving DecidableEq, Repr

def devReceiptAdmitted (root : String) (receipt : ArtifactReceipt) : Bool :=
  originAdmitted (.dev root) receipt.origin &&
    receipt.sourceRoot == some root &&
    receipt.sourceGenerationMatches &&
    receipt.binaryDigestMatches &&
    receipt.commandBindingDigestMatches

theorem dev_receipt_requires_develop_workspace_origin
    (receipt : ArtifactReceipt)
    (h : devReceiptAdmitted "/checkout" receipt = true) :
    receipt.origin = .developWorkspace := by
  cases ho : receipt.origin <;>
    simp [devReceiptAdmitted, originAdmitted, ho] at h ⊢

theorem dev_receipt_requires_configured_root
    (root : String)
    (receipt : ArtifactReceipt)
    (h : devReceiptAdmitted root receipt = true) :
    receipt.sourceRoot = some root := by
  simp [devReceiptAdmitted] at h
  exact h.1.1.1.2

inductive Resolution where
  | admitted (receipt : ArtifactReceipt)
  | devArtifactUnavailable
  deriving DecidableEq, Repr

def resolveDev (root : String) (candidate : Option ArtifactReceipt) : Resolution :=
  match candidate with
  | some receipt =>
      if devReceiptAdmitted root receipt then .admitted receipt
      else .devArtifactUnavailable
  | none => .devArtifactUnavailable

theorem missing_dev_artifact_fails_closed :
    resolveDev "/checkout" none = .devArtifactUnavailable := by
  rfl

theorem release_candidate_fails_closed_in_dev_model
    (receipt : ArtifactReceipt)
    (h : receipt.origin = .lockedRelease) :
    resolveDev "/checkout" (some receipt) = .devArtifactUnavailable := by
  simp [resolveDev, devReceiptAdmitted, originAdmitted, h]

theorem disabled_dev_config_selects_release (root : String) :
    runtimeMode (some ⟨false, root⟩) = .release := by
  rfl

theorem enabled_dev_config_selects_its_root (root : String) :
    runtimeMode (some ⟨true, root⟩) = .dev root := by
  rfl

inductive InstallAuthority where
  | developmentInstaller (root : String)
  | lockedRelease
  deriving DecidableEq, Repr

def installAuthority : RuntimeMode → InstallAuthority
  | .dev root => .developmentInstaller root
  | .release => .lockedRelease

theorem dev_plain_install_uses_configured_root (root : String) :
    installAuthority (.dev root) = .developmentInstaller root := by
  rfl

theorem dev_plain_install_never_uses_locked_release (root : String) :
    installAuthority (.dev root) ≠ .lockedRelease := by
  simp [installAuthority]

inductive DevelopmentInstallPhase where
  | delegateBuild
  | publishReceipt
  deriving DecidableEq, Repr

def nextDevelopmentInstallPhase : DevelopmentInstallPhase → Option DevelopmentInstallPhase
  | .delegateBuild => some .publishReceipt
  | .publishReceipt => none

theorem delegated_receipt_cannot_reenter_build :
    nextDevelopmentInstallPhase .publishReceipt ≠ some .delegateBuild := by
  simp [nextDevelopmentInstallPhase]

inductive DevelopmentArtifactDomain where
  | checkout
  | stateHomeProviderStaging
  deriving DecidableEq, Repr

structure ProviderDevelopmentDescriptor where
  sourceRoot : String
  artifactDomain : DevelopmentArtifactDomain
  deriving DecidableEq, Repr

def developmentArtifactAdmitted
    (domain : DevelopmentArtifactDomain)
    (belowProviderSourceRoot belowTypedStagingRoot : Bool) : Bool :=
  match domain with
  | .checkout => belowProviderSourceRoot
  | .stateHomeProviderStaging => belowTypedStagingRoot

theorem checkout_domain_does_not_admit_staging_only :
    developmentArtifactAdmitted .checkout false true = false := by
  rfl

theorem staging_domain_does_not_admit_checkout_only :
    developmentArtifactAdmitted .stateHomeProviderStaging true false = false := by
  rfl

def provenanceSourceRoot (descriptor : ProviderDevelopmentDescriptor) : String :=
  descriptor.sourceRoot

theorem provenance_uses_provider_owned_source_root
    (descriptor : ProviderDevelopmentDescriptor) :
    provenanceSourceRoot descriptor = descriptor.sourceRoot := by
  rfl

structure WarmPathReceipt where
  configReads : Nat
  releaseLockReads : Nat
  providerInstalls : Nat
  databaseOpens : Nat
  processSpawns : Nat
  deriving DecidableEq, Repr

def residentWarmPath (receipt : WarmPathReceipt) : Prop :=
  receipt.configReads = 0 ∧
  receipt.releaseLockReads = 0 ∧
  receipt.providerInstalls = 0 ∧
  receipt.databaseOpens = 0 ∧
  receipt.processSpawns = 0

theorem canonical_warm_receipt_is_resident :
    residentWarmPath ⟨0, 0, 0, 0, 0⟩ := by
  simp [residentWarmPath]

end ASPProof.RuntimeArtifactAuthority
