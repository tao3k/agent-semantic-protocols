namespace ASPProof.RuntimeWorkspaceGeneration

abbrev WorkspaceId := String
abbrev ProviderId := String
abbrev WriterToken := String
abbrev LeaseId := String
abbrev GenerationId := String
abbrev Digest := String

structure RootDepth where
  liveOverlay : Nat
  committedBase : Nat
  deriving DecidableEq, Repr

def layeredRootDepth : RootDepth :=
  { liveOverlay := 1, committedBase := 0 }

/-- Static activation authority.  It admits a provider and one schema digest;
it does not contain workspace paths, source roots, or an active generation. -/
structure ActivationCapability where
  providerId : ProviderId
  schemaDigest : Digest
  artifactDigest : Digest
  deriving DecidableEq, Repr

/-- Language-provider output.  A provider reports typed project scope and source
evidence, but never chooses or publishes the active generation. -/
structure ProviderScopeReceipt where
  workspaceId : WorkspaceId
  providerId : ProviderId
  schemaDigest : Digest
  projectScopeDigest : Digest
  sourceSnapshotDigest : Digest
  deriving DecidableEq, Repr

/-- All content that participates in the runtime-server generation identity.
The selector set is part of the same identity as the source and module graph. -/
structure GenerationInputs where
  workspaceId : WorkspaceId
  providerId : ProviderId
  schemaDigest : Digest
  projectScopeDigest : Digest
  rootDepth : RootDepth
  sourceRootDigest : Digest
  baseRootDigest : Option Digest
  sourceProviderDigest : Digest
  dirtyPathsDigest : Option Digest
  moduleGraphDigest : Digest
  selectorSetDigest : Digest
  memoryBackendDigest : Digest
  deriving DecidableEq, Repr

opaque deriveGenerationId : GenerationInputs → GenerationId

/-- A generation may be staged only after a provider receipt refines the static
activation capability and the server-owned generation inputs. -/
structure PreparedGeneration where
  capability : ActivationCapability
  providerReceipt : ProviderScopeReceipt
  inputs : GenerationInputs
  generationId : GenerationId
  providerMatches : providerReceipt.providerId = capability.providerId
  providerInputMatches : inputs.providerId = providerReceipt.providerId
  workspaceMatches : inputs.workspaceId = providerReceipt.workspaceId
  schemaAdmitted : providerReceipt.schemaDigest = capability.schemaDigest
  schemaInputMatches : inputs.schemaDigest = providerReceipt.schemaDigest
  scopeMatches : inputs.projectScopeDigest = providerReceipt.projectScopeDigest
  identitySound : generationId = deriveGenerationId inputs

/-- Durable and resident publication are one receipt.  There is no representable
active state containing only a pointer, only a memory segment, or selectors from
a different generation. -/
structure PublicationReceipt where
  workspaceId : WorkspaceId
  generationId : GenerationId
  schemaDigest : Digest
  projectScopeDigest : Digest
  rootDepth : RootDepth
  sourceRootDigest : Digest
  baseRootDigest : Option Digest
  sourceProviderDigest : Digest
  dirtyPathsDigest : Option Digest
  moduleGraphDigest : Digest
  selectorSetDigest : Digest
  memoryBackendDigest : Digest
  durableCommitDigest : Digest
  deriving DecidableEq, Repr

structure ActiveGeneration where
  prepared : PreparedGeneration
  publication : PublicationReceipt
  workspaceMatches : publication.workspaceId = prepared.inputs.workspaceId
  generationMatches : publication.generationId = prepared.generationId
  schemaMatches : publication.schemaDigest = prepared.inputs.schemaDigest
  projectScopeMatches : publication.projectScopeDigest = prepared.inputs.projectScopeDigest
  rootDepthMatches : publication.rootDepth = prepared.inputs.rootDepth
  sourceRootMatches : publication.sourceRootDigest = prepared.inputs.sourceRootDigest
  baseRootMatches : publication.baseRootDigest = prepared.inputs.baseRootDigest
  sourceProviderMatches : publication.sourceProviderDigest = prepared.inputs.sourceProviderDigest
  dirtyPathsMatches : publication.dirtyPathsDigest = prepared.inputs.dirtyPathsDigest
  moduleGraphMatches : publication.moduleGraphDigest = prepared.inputs.moduleGraphDigest
  selectorSetMatches : publication.selectorSetDigest = prepared.inputs.selectorSetDigest
  memoryBackendMatches : publication.memoryBackendDigest = prepared.inputs.memoryBackendDigest

structure ReadLease where
  leaseId : LeaseId
  workspaceId : WorkspaceId
  generationId : GenerationId
  deriving DecidableEq, Repr

structure WorkspaceState where
  workspaceId : WorkspaceId
  writerToken : Option WriterToken := none
  active : Option ActiveGeneration := none
  staged : Option PreparedGeneration := none
  retained : List ActiveGeneration := []
  leases : List ReadLease := []

def generationVisible (state : WorkspaceState) (generationId : GenerationId) : Prop :=
  (∃ active, state.active = some active ∧ active.prepared.generationId = generationId) ∨
  ∃ retained, retained ∈ state.retained ∧ retained.prepared.generationId = generationId

def writerOnlineState (state : WorkspaceState) (writer : WriterToken) : WorkspaceState :=
  { state with writerToken := some writer }

def writerOfflineState (state : WorkspaceState) : WorkspaceState :=
  { state with writerToken := none }

def stageState (state : WorkspaceState) (prepared : PreparedGeneration) : WorkspaceState :=
  { state with staged := some prepared }

def commitState (state : WorkspaceState) (active : ActiveGeneration) : WorkspaceState :=
  { state with
      active := some active
      staged := none
      retained := state.active.toList ++ state.retained }

def acquireLeaseState (state : WorkspaceState) (lease : ReadLease) : WorkspaceState :=
  { state with leases := lease :: state.leases }

def releaseLeaseState (state : WorkspaceState) (leaseId : LeaseId) : WorkspaceState :=
  { state with leases := state.leases.filter (fun lease => lease.leaseId != leaseId) }

inductive WorkspaceEvent where
  | writerOnline (writer : WriterToken)
  | writerOffline
  | reconcileStale (writer : WriterToken) (prepared : PreparedGeneration)
  | publish (writer : WriterToken) (receipt : PublicationReceipt)
  | acquireLease (lease : ReadLease)
  | releaseLease (leaseId : LeaseId)

/-- The runtime workspace entry is the only state-transition authority.  Query
events can acquire or release leases, but cannot stage or publish generations. -/
inductive Step : WorkspaceState → WorkspaceEvent → WorkspaceState → Prop where
  | writerOnline (state writer) :
      Step state (.writerOnline writer) (writerOnlineState state writer)
  | writerOffline (state) :
      Step state .writerOffline (writerOfflineState state)
  | reconcileStale (state writer prepared)
      (writerOwns : state.writerToken = some writer)
      (workspaceMatches : prepared.inputs.workspaceId = state.workspaceId) :
      Step state (.reconcileStale writer prepared) (stageState state prepared)
  | publish (state writer prepared receipt)
      (writerOwns : state.writerToken = some writer)
      (stagedMatches : state.staged = some prepared)
      (receiptWorkspaceMatches : receipt.workspaceId = prepared.inputs.workspaceId)
      (receiptGenerationMatches : receipt.generationId = prepared.generationId)
      (receiptSchemaMatches : receipt.schemaDigest = prepared.inputs.schemaDigest)
      (receiptProjectScopeMatches : receipt.projectScopeDigest = prepared.inputs.projectScopeDigest)
      (receiptRootDepthMatches : receipt.rootDepth = prepared.inputs.rootDepth)
      (receiptSourceRootMatches : receipt.sourceRootDigest = prepared.inputs.sourceRootDigest)
      (receiptBaseRootMatches : receipt.baseRootDigest = prepared.inputs.baseRootDigest)
      (receiptSourceProviderMatches : receipt.sourceProviderDigest = prepared.inputs.sourceProviderDigest)
      (receiptDirtyPathsMatches : receipt.dirtyPathsDigest = prepared.inputs.dirtyPathsDigest)
      (receiptModuleGraphMatches : receipt.moduleGraphDigest = prepared.inputs.moduleGraphDigest)
      (receiptSelectorMatches : receipt.selectorSetDigest = prepared.inputs.selectorSetDigest)
      (receiptMemoryMatches : receipt.memoryBackendDigest = prepared.inputs.memoryBackendDigest) :
      Step state (.publish writer receipt)
        (commitState state {
          prepared := prepared
          publication := receipt
          workspaceMatches := receiptWorkspaceMatches
          generationMatches := receiptGenerationMatches
          schemaMatches := receiptSchemaMatches
          projectScopeMatches := receiptProjectScopeMatches
          rootDepthMatches := receiptRootDepthMatches
          sourceRootMatches := receiptSourceRootMatches
          baseRootMatches := receiptBaseRootMatches
          sourceProviderMatches := receiptSourceProviderMatches
          dirtyPathsMatches := receiptDirtyPathsMatches
          moduleGraphMatches := receiptModuleGraphMatches
          selectorSetMatches := receiptSelectorMatches
          memoryBackendMatches := receiptMemoryMatches
        })
  | acquireLease (state lease)
      (workspaceMatches : lease.workspaceId = state.workspaceId)
      (visible : generationVisible state lease.generationId) :
      Step state (.acquireLease lease) (acquireLeaseState state lease)
  | releaseLease (state leaseId) :
      Step state (.releaseLease leaseId) (releaseLeaseState state leaseId)

theorem generation_identity_deterministic
    (left right : GenerationInputs)
    (sameInputs : left = right) :
    deriveGenerationId left = deriveGenerationId right := by
  cases sameInputs
  rfl

theorem writer_offline_preserves_active
    (state next : WorkspaceState)
    (step : Step state .writerOffline next) :
    next.active = state.active := by
  cases step
  rfl

theorem stale_reconciliation_preserves_active
    (state next : WorkspaceState)
    (writer : WriterToken)
    (prepared : PreparedGeneration)
    (step : Step state (.reconcileStale writer prepared) next) :
    next.active = state.active := by
  cases step
  rfl

theorem acquire_lease_preserves_active
    (state next : WorkspaceState)
    (lease : ReadLease)
    (step : Step state (.acquireLease lease) next) :
    next.active = state.active := by
  cases step
  rfl

theorem release_lease_preserves_active
    (state next : WorkspaceState)
    (leaseId : LeaseId)
    (step : Step state (.releaseLease leaseId) next) :
    next.active = state.active := by
  cases step
  rfl

theorem active_publication_is_atomic (active : ActiveGeneration) :
    active.publication.workspaceId = active.prepared.inputs.workspaceId ∧
    active.publication.generationId = active.prepared.generationId ∧
    active.publication.schemaDigest = active.prepared.inputs.schemaDigest ∧
    active.publication.projectScopeDigest = active.prepared.inputs.projectScopeDigest ∧
    active.publication.rootDepth = active.prepared.inputs.rootDepth ∧
    active.publication.sourceRootDigest = active.prepared.inputs.sourceRootDigest ∧
    active.publication.baseRootDigest = active.prepared.inputs.baseRootDigest ∧
    active.publication.sourceProviderDigest = active.prepared.inputs.sourceProviderDigest ∧
    active.publication.dirtyPathsDigest = active.prepared.inputs.dirtyPathsDigest ∧
    active.publication.moduleGraphDigest = active.prepared.inputs.moduleGraphDigest ∧
    active.publication.selectorSetDigest = active.prepared.inputs.selectorSetDigest ∧
    active.publication.memoryBackendDigest = active.prepared.inputs.memoryBackendDigest := by
  exact ⟨active.workspaceMatches, active.generationMatches, active.schemaMatches,
    active.projectScopeMatches, active.rootDepthMatches, active.sourceRootMatches,
    active.baseRootMatches, active.sourceProviderMatches, active.dirtyPathsMatches,
    active.moduleGraphMatches, active.selectorSetMatches, active.memoryBackendMatches⟩

/-- Resident read evidence is admitted only when it is generation-pinned and
performs no database open on the read path. -/
structure ResidentReadReceipt where
  workspaceId : WorkspaceId
  generationId : GenerationId
  leaseId : LeaseId
  residentHit : Bool
  databaseOpens : Nat
  deriving DecidableEq, Repr

def ResidentOnlyRead (receipt : ResidentReadReceipt) : Prop :=
  receipt.residentHit = true ∧ receipt.databaseOpens = 0

abbrev ServerState := WorkspaceId → WorkspaceState

def replaceWorkspace
    (server : ServerState)
    (workspaceId : WorkspaceId)
    (state : WorkspaceState) : ServerState :=
  fun candidate => if candidate = workspaceId then state else server candidate

theorem workspace_transition_isolated
    (server : ServerState)
    (workspaceId otherWorkspace : WorkspaceId)
    (state : WorkspaceState)
    (distinct : otherWorkspace ≠ workspaceId) :
    replaceWorkspace server workspaceId state otherWorkspace = server otherWorkspace := by
  simp [replaceWorkspace, distinct]

end ASPProof.RuntimeWorkspaceGeneration
