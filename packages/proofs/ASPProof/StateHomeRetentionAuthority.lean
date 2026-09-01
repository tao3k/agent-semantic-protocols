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

structure WorkspaceCatalogProjection where
  canonicalRoot : String
  derivedIdentity : String
  deriving DecidableEq, Repr

inductive WorkspaceCatalogReconcile where
  | unchanged (projection : WorkspaceCatalogProjection)
  | replaced (projection : WorkspaceCatalogProjection)
  | rejected
  deriving DecidableEq, Repr

structure WorkspaceCatalogReconcileReceipt where
  canonicalRoot : String
  previousDerivedIdentity : String
  currentDerivedIdentity : String
  previousRevision : Nat
  currentRevision : Nat
  deriving DecidableEq, Repr

def reconcileWorkspaceCatalogProjection
    (currentDerivedIdentity requestedIdentity : String)
    (existing : WorkspaceCatalogProjection) : WorkspaceCatalogReconcile :=
  if requestedIdentity != currentDerivedIdentity then
    .rejected
  else if existing.derivedIdentity = currentDerivedIdentity then
    .unchanged existing
  else
    .replaced
      { canonicalRoot := existing.canonicalRoot
        derivedIdentity := currentDerivedIdentity }

theorem noncanonical_workspace_request_preserves_catalog
    (currentDerivedIdentity requestedIdentity : String)
    (existing : WorkspaceCatalogProjection)
    (mismatch : requestedIdentity ≠ currentDerivedIdentity) :
    reconcileWorkspaceCatalogProjection currentDerivedIdentity requestedIdentity existing =
      .rejected := by
  simp [reconcileWorkspaceCatalogProjection, mismatch]

theorem stale_projection_is_replaced_by_current_derivation
    (currentDerivedIdentity : String)
    (existing : WorkspaceCatalogProjection)
    (stale : existing.derivedIdentity ≠ currentDerivedIdentity) :
    reconcileWorkspaceCatalogProjection currentDerivedIdentity currentDerivedIdentity existing =
      .replaced
        { canonicalRoot := existing.canonicalRoot
          derivedIdentity := currentDerivedIdentity } := by
  simp [reconcileWorkspaceCatalogProjection, stale]

theorem reconciled_projection_replay_is_idempotent
    (currentDerivedIdentity canonicalRoot : String) :
    reconcileWorkspaceCatalogProjection currentDerivedIdentity currentDerivedIdentity
        { canonicalRoot, derivedIdentity := currentDerivedIdentity } =
      .unchanged { canonicalRoot, derivedIdentity := currentDerivedIdentity } := by
  simp [reconcileWorkspaceCatalogProjection]

def reconcileWorkspaceCatalogReceipt
    (previousRevision : Nat)
    (previous : WorkspaceCatalogProjection)
    (result : WorkspaceCatalogReconcile) : Option WorkspaceCatalogReconcileReceipt :=
  match result with
  | .replaced current =>
      some
        { canonicalRoot := current.canonicalRoot
          previousDerivedIdentity := previous.derivedIdentity
          currentDerivedIdentity := current.derivedIdentity
          previousRevision
          currentRevision := previousRevision + 1 }
  | .unchanged _ | .rejected => none

theorem replaced_projection_emits_one_identity_bound_revision
    (previousRevision : Nat)
    (previous current : WorkspaceCatalogProjection) :
    reconcileWorkspaceCatalogReceipt previousRevision previous (.replaced current) =
      some
        { canonicalRoot := current.canonicalRoot
          previousDerivedIdentity := previous.derivedIdentity
          currentDerivedIdentity := current.derivedIdentity
          previousRevision
          currentRevision := previousRevision + 1 } := by
  rfl

theorem rejected_projection_emits_no_migration_receipt
    (previousRevision : Nat)
    (previous : WorkspaceCatalogProjection) :
    reconcileWorkspaceCatalogReceipt previousRevision previous .rejected = none := by
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
