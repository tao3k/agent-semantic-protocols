-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.RuntimeWorkspaceGeneration

abbrev ProjectId := String
abbrev WorkspaceId := String
abbrev ProviderId := String
abbrev WriterToken := String
abbrev LeaseId := String
abbrev GenerationId := String
abbrev Digest := String

structure ProjectWorkspaceKey where
  projectId : ProjectId
  workspaceId : WorkspaceId
  deriving DecidableEq, Repr

/-- The V1 catalog is refined in place.  A pre-ProjectId V1 document is not a
second schema generation and cannot contribute resident admission authority. -/
inductive WorkspaceAdmissionCatalogV1 where
  | current (entries : List ProjectWorkspaceKey)
  | legacyWithoutProjectId (workspaceIds : List WorkspaceId)
  deriving DecidableEq, Repr

def admittedProjectWorkspaces : WorkspaceAdmissionCatalogV1 → List ProjectWorkspaceKey
  | .current entries => entries
  | .legacyWithoutProjectId _ => []

theorem legacy_v1_catalog_has_no_project_workspace_authority
    (workspaceIds : List WorkspaceId) :
    admittedProjectWorkspaces (.legacyWithoutProjectId workspaceIds) = [] := by
  rfl

theorem current_v1_catalog_preserves_project_workspace_identity
    (entries : List ProjectWorkspaceKey) :
    admittedProjectWorkspaces (.current entries) = entries := by
  rfl

inductive WorkspaceAdmissionIngress where
  | hostControl
  | initializedReadSession
  deriving DecidableEq, Repr

def admitsColdWorkspace : WorkspaceAdmissionIngress → Bool
  | .hostControl => true
  | .initializedReadSession => false

def coldAdmission
    (ingress : WorkspaceAdmissionIngress)
    (key : ProjectWorkspaceKey)
    (entries : List ProjectWorkspaceKey) : List ProjectWorkspaceKey :=
  if admitsColdWorkspace ingress then key :: entries else entries

theorem host_control_can_admit_empty_v1_catalog (key : ProjectWorkspaceKey) :
    coldAdmission .hostControl key [] = [key] := by
  rfl

theorem initialized_read_session_cannot_mutate_v1_catalog
    (key : ProjectWorkspaceKey)
    (entries : List ProjectWorkspaceKey) :
    coldAdmission .initializedReadSession key entries = entries := by
  rfl

/-- Cold control admission creates only the project/workspace identity binding.
Generation construction and workspace database bootstrap belong to later,
distinct authorities. -/
structure ColdWorkspaceIdentityAdmission where
  entries : List ProjectWorkspaceKey
  generationReady : Bool
  databaseOpened : Bool
  deriving DecidableEq, Repr

def admitColdWorkspaceIdentity
    (key : ProjectWorkspaceKey)
    (entries : List ProjectWorkspaceKey) : ColdWorkspaceIdentityAdmission :=
  { entries := key :: entries
    generationReady := false
    databaseOpened := false }

theorem cold_identity_admission_does_not_construct_generation
    (key : ProjectWorkspaceKey)
    (entries : List ProjectWorkspaceKey) :
    (admitColdWorkspaceIdentity key entries).generationReady = false := by
  rfl

theorem cold_identity_admission_does_not_open_workspace_database
    (key : ProjectWorkspaceKey)
    (entries : List ProjectWorkspaceKey) :
    (admitColdWorkspaceIdentity key entries).databaseOpened = false := by
  rfl

structure PublicReadIdentity where
  key : ProjectWorkspaceKey
  projectRootExposed : Bool
  privateOwnerEndpointExposed : Bool
  deriving DecidableEq, Repr

def PublicReadIdentityAdmitted (identity : PublicReadIdentity) : Prop :=
  identity.projectRootExposed = false ∧
  identity.privateOwnerEndpointExposed = false

theorem public_read_routes_only_by_project_workspace_identity
    (key : ProjectWorkspaceKey) :
    PublicReadIdentityAdmitted
      { key := key
        projectRootExposed := false
        privateOwnerEndpointExposed := false } := by
  simp [PublicReadIdentityAdmitted]

structure BinaryRuntimeFacts where
  binaryDigest : Digest
  activityObserved : Bool
  healthy : Bool
  deriving DecidableEq, Repr

inductive BinaryServingAuthority where
  | digestActivityHealth
  | mutableGlobalSwitch
  deriving DecidableEq, Repr

def binaryServingAuthorityAdmitted : BinaryServingAuthority → Bool
  | .digestActivityHealth => true
  | .mutableGlobalSwitch => false

theorem mutable_global_switch_is_not_binary_authority :
    binaryServingAuthorityAdmitted .mutableGlobalSwitch = false := by
  rfl

structure RootDepth where
  liveOverlay : Nat
  committedBase : Nat
  deriving DecidableEq, Repr

def layeredRootDepth : RootDepth :=
  { liveOverlay := 1, committedBase := 0 }

/-- Source coverage may be empty after an incremental deletion. Completeness
requires only that parser-owned owners never exceed the admitted source leaves. -/
structure SourceCoverage where
  leafCount : Nat
  ownerCount : Nat
  deriving DecidableEq, Repr

def CompleteSourceCoverage (coverage : SourceCoverage) : Prop :=
  coverage.ownerCount ≤ coverage.leafCount

theorem empty_source_coverage_is_complete :
    CompleteSourceCoverage { leafCount := 0, ownerCount := 0 } := by
  simp [CompleteSourceCoverage]

/-- Static provider-artifact authority.  It admits a provider and one schema digest;
it does not contain workspace paths, source roots, or an active generation. -/
structure ProviderArtifactCapability where
  providerId : ProviderId
  schemaDigest : Digest
  artifactDigest : Digest
  deriving DecidableEq, Repr

/-- Language-provider output.  A provider reports typed project scope and source
evidence, but never chooses or publishes the active generation. -/
structure ProviderScopeReceipt where
  projectId : ProjectId
  workspaceId : WorkspaceId
  providerId : ProviderId
  schemaDigest : Digest
  projectScopeDigest : Digest
  sourceSnapshotDigest : Digest
  deriving DecidableEq, Repr

/-- All content that participates in the runtime-server generation identity.
The selector set is part of the same identity as the source and module graph. -/
structure GenerationInputs where
  projectId : ProjectId
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

/-- V1 source lineage has exactly two legal shapes: a baseline carries neither
parent nor dirty-set evidence, while a successor carries both.  A half-overlay
cannot identify an immutable generation. -/
def OverlayLineageComplete (baseRootDigest dirtyPathsDigest : Option Digest) : Prop :=
  baseRootDigest.isSome = dirtyPathsDigest.isSome

theorem baseline_overlay_lineage_is_complete :
    OverlayLineageComplete none none := by
  rfl

theorem successor_overlay_lineage_is_complete (baseRoot dirtyPaths : Digest) :
    OverlayLineageComplete (some baseRoot) (some dirtyPaths) := by
  rfl

theorem base_without_dirty_paths_is_not_a_generation (baseRoot : Digest) :
    ¬ OverlayLineageComplete (some baseRoot) none := by
  simp [OverlayLineageComplete]

theorem dirty_paths_without_base_is_not_a_generation (dirtyPaths : Digest) :
    ¬ OverlayLineageComplete none (some dirtyPaths) := by
  simp [OverlayLineageComplete]

opaque deriveGenerationId : GenerationInputs → GenerationId

/-- A generation may be staged only after a provider receipt refines the static
provider-artifact capability and the server-owned generation inputs. -/
structure PreparedGeneration where
  capability : ProviderArtifactCapability
  providerReceipt : ProviderScopeReceipt
  inputs : GenerationInputs
  generationId : GenerationId
  providerMatches : providerReceipt.providerId = capability.providerId
  providerInputMatches : inputs.providerId = providerReceipt.providerId
  projectMatches : inputs.projectId = providerReceipt.projectId
  workspaceMatches : inputs.workspaceId = providerReceipt.workspaceId
  schemaAdmitted : providerReceipt.schemaDigest = capability.schemaDigest
  schemaInputMatches : inputs.schemaDigest = providerReceipt.schemaDigest
  scopeMatches : inputs.projectScopeDigest = providerReceipt.projectScopeDigest
  identitySound : generationId = deriveGenerationId inputs

/-- Durable and resident publication are one receipt.  There is no representable
active state containing only a pointer, only a memory segment, or selectors from
a different generation. -/
structure PublicationReceipt where
  projectId : ProjectId
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
  projectMatches : publication.projectId = prepared.inputs.projectId
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
  projectId : ProjectId
  workspaceId : WorkspaceId
  generationId : GenerationId
  deriving DecidableEq, Repr

structure WorkspaceState where
  projectId : ProjectId
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
      (projectMatches : prepared.inputs.projectId = state.projectId)
      (workspaceMatches : prepared.inputs.workspaceId = state.workspaceId) :
      Step state (.reconcileStale writer prepared) (stageState state prepared)
  | publish (state writer prepared receipt)
      (writerOwns : state.writerToken = some writer)
      (stagedMatches : state.staged = some prepared)
      (receiptProjectMatches : receipt.projectId = prepared.inputs.projectId)
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
          projectMatches := receiptProjectMatches
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
      (projectMatches : lease.projectId = state.projectId)
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
    active.publication.projectId = active.prepared.inputs.projectId ∧
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
  exact ⟨active.projectMatches, active.workspaceMatches, active.generationMatches, active.schemaMatches,
    active.projectScopeMatches, active.rootDepthMatches, active.sourceRootMatches,
    active.baseRootMatches, active.sourceProviderMatches, active.dirtyPathsMatches,
    active.moduleGraphMatches, active.selectorSetMatches, active.memoryBackendMatches⟩

/-- Resident read evidence is admitted only when it is generation-pinned and
performs no filesystem, database, provider, or control-plane operation. -/
structure ResidentReadReceipt where
  projectId : ProjectId
  workspaceId : WorkspaceId
  generationId : GenerationId
  leaseId : LeaseId
  residentHit : Bool
  filesystemReads : Nat
  databaseOpens : Nat
  providerSpawns : Nat
  controlSocketRoundtrips : Nat
  deriving DecidableEq, Repr

def ResidentOnlyRead (receipt : ResidentReadReceipt) : Prop :=
  receipt.residentHit = true ∧
  receipt.filesystemReads = 0 ∧
  receipt.databaseOpens = 0 ∧
  receipt.providerSpawns = 0 ∧
  receipt.controlSocketRoundtrips = 0

def ResidentReadAdmitted
    (key : ProjectWorkspaceKey)
    (receipt : ResidentReadReceipt) : Prop :=
  receipt.projectId = key.projectId ∧
  receipt.workspaceId = key.workspaceId ∧
  ResidentOnlyRead receipt

theorem resident_read_cannot_cross_project
    (key : ProjectWorkspaceKey)
    (receipt : ResidentReadReceipt)
    (projectMismatch : receipt.projectId ≠ key.projectId) :
    ¬ ResidentReadAdmitted key receipt := by
  intro admitted
  exact projectMismatch admitted.1

abbrev ServerState := ProjectWorkspaceKey → WorkspaceState

def replaceWorkspace
    (server : ServerState)
    (key : ProjectWorkspaceKey)
    (state : WorkspaceState) : ServerState :=
  fun candidate => if candidate = key then state else server candidate

theorem workspace_transition_isolated
    (server : ServerState)
    (key otherKey : ProjectWorkspaceKey)
    (state : WorkspaceState)
    (distinct : otherKey ≠ key) :
    replaceWorkspace server key state otherKey = server otherKey := by
  simp [replaceWorkspace, distinct]

end ASPProof.RuntimeWorkspaceGeneration
