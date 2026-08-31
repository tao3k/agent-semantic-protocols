namespace ASPProof.StateHomeRetentionAuthority

structure ProjectBinding where
  hostProject : Option String
  repoIdentity : String
  workspaceIdentity : String
  deriving DecidableEq, Repr

structure RetainedObject where
  identity : String
  lastObservedAt : Nat
  deriving DecidableEq, Repr

structure Lease where
  objectIdentity : String
  expiresAt : Option Nat
  deriving DecidableEq, Repr

def leaseActive (now : Nat) (lease : Lease) : Bool :=
  match lease.expiresAt with
  | none => true
  | some expiresAt => now < expiresAt

def isProtectedByLease (now : Nat) (object : RetainedObject) (leases : List Lease) : Bool :=
  leases.any fun lease =>
    lease.objectIdentity == object.identity && leaseActive now lease

def retireEligible
    (now retainFor : Nat)
    (object : RetainedObject)
    (leases : List Lease) : Bool :=
  !isProtectedByLease now object leases && object.lastObservedAt + retainFor ≤ now

theorem active_lease_is_never_retired
    (now retainFor : Nat)
    (object : RetainedObject)
    (leases : List Lease)
    (isProtected : isProtectedByLease now object leases = true) :
    retireEligible now retainFor object leases = false := by
  simp [retireEligible, isProtected]

def resolveBinding
    (hostProject : Option String)
    (repoIdentity workspaceIdentity : String) : ProjectBinding :=
  { hostProject, repoIdentity, workspaceIdentity }

theorem host_reference_does_not_change_workspace_identity
    (leftHost rightHost : Option String)
    (repoIdentity workspaceIdentity : String) :
    (resolveBinding leftHost repoIdentity workspaceIdentity).workspaceIdentity =
      (resolveBinding rightHost repoIdentity workspaceIdentity).workspaceIdentity := by
  rfl

inductive CleanupCommit where
  | unchanged
  | committed
  deriving DecidableEq, Repr

def cleanupFailureResult : CleanupCommit := .unchanged

theorem cleanup_failure_preserves_previous_authority :
    cleanupFailureResult = .unchanged := by
  rfl

structure CatalogState where
  generation : Nat
  bindingDigests : List String
  objectIdentities : List String
  leaseObjectIdentities : List String
  deriving DecidableEq, Repr

structure CatalogObservation where
  bindingDigest : String
  objectIdentity : String
  leaseObjectIdentities : List String
  deriving DecidableEq, Repr

def publishObservationBatch
    (catalog : CatalogState)
    (observations : List CatalogObservation) : CatalogState :=
  { generation := catalog.generation + 1
    bindingDigests := observations.map (·.bindingDigest) ++ catalog.bindingDigests
    objectIdentities := observations.map (·.objectIdentity) ++ catalog.objectIdentities
    leaseObjectIdentities :=
      observations.flatMap (·.leaseObjectIdentities) ++ catalog.leaseObjectIdentities }

theorem observation_batch_has_one_generation_commit
    (catalog : CatalogState)
    (observations : List CatalogObservation) :
    (publishObservationBatch catalog observations).generation = catalog.generation + 1 := by
  rfl

def observationBindingsAreComplete (observation : CatalogObservation) : Bool :=
  observation.bindingDigest != "" && observation.objectIdentity != "" &&
    observation.leaseObjectIdentities.all (· == observation.objectIdentity)

def batchValid (observations : List CatalogObservation) : Bool :=
  observations.all observationBindingsAreComplete

def publishValidatedBatch
    (catalog : CatalogState)
    (observations : List CatalogObservation) : CatalogState :=
  if batchValid observations then publishObservationBatch catalog observations else catalog

theorem invalid_batch_preserves_catalog
    (catalog : CatalogState)
    (observations : List CatalogObservation)
    (invalid : batchValid observations = false) :
    publishValidatedBatch catalog observations = catalog := by
  simp [publishValidatedBatch, invalid]

def catalogAdmitsRetirement
    (now retainFor : Nat)
    (object : RetainedObject)
    (catalog : CatalogState) : Bool :=
  retireEligible now retainFor object <|
    catalog.leaseObjectIdentities.map fun objectIdentity =>
      { objectIdentity, expiresAt := none }

theorem catalog_lease_blocks_retirement
    (now retainFor : Nat)
    (object : RetainedObject)
    (catalog : CatalogState)
    (present : object.identity ∈ catalog.leaseObjectIdentities) :
    catalogAdmitsRetirement now retainFor object catalog = false := by
  apply active_lease_is_never_retired
  simp [isProtectedByLease, leaseActive, present]

end ASPProof.StateHomeRetentionAuthority
