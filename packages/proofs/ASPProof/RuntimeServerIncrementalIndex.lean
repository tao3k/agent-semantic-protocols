namespace ASPProof.RuntimeServerIncrementalIndex

abbrev WorkspaceId := Nat
abbrev ProviderId := Nat
abbrev Generation := Nat
abbrev Revision := Nat
abbrev Digest := Nat

structure IndexState where
  workspace : WorkspaceId
  activeGeneration : Generation
  readerOnline : Bool
  writerOnline : Bool
  providerRevision : ProviderId → Revision

structure IndexDelta where
  workspace : WorkspaceId
  provider : ProviderId
  baseGeneration : Generation
  workspaceSnapshot : Digest
  snapshotRevision : Revision
  factsDigest : Digest
deriving DecidableEq, Repr

structure StagedBatch where
  baseGeneration : Generation
  nextGeneration : Generation
  coveredProviders : ProviderId → Bool

structure CacheReceipt where
  searchGeneration : Generation
  modelPrefixDigest : Digest
  modelPrefixHit : Bool
deriving DecidableEq, Repr

def DeltaAdmissible (state : IndexState) (delta : IndexDelta) : Prop :=
  state.writerOnline = true ∧
  delta.workspace = state.workspace ∧
  delta.baseGeneration = state.activeGeneration ∧
  state.providerRevision delta.provider < delta.snapshotRevision

def CanCoalesce (left right : IndexDelta) : Prop :=
  left.workspace = right.workspace ∧
  left.baseGeneration = right.baseGeneration ∧
  left.workspaceSnapshot = right.workspaceSnapshot ∧
  (left.provider ≠ right.provider ∨
    (left.provider = right.provider ∧
     left.snapshotRevision = right.snapshotRevision ∧
     left.factsDigest = right.factsDigest))

def BatchComplete
    (required : ProviderId → Bool)
    (batch : StagedBatch) : Prop :=
  ∀ provider, required provider = true →
    batch.coveredProviders provider = true

def CanPublish
    (state : IndexState)
    (required : ProviderId → Bool)
    (batch : StagedBatch) : Prop :=
  state.writerOnline = true ∧
  batch.baseGeneration = state.activeGeneration ∧
  state.activeGeneration < batch.nextGeneration ∧
  BatchComplete required batch

def SearchCacheValid (state : IndexState) (receipt : CacheReceipt) : Prop :=
  receipt.searchGeneration = state.activeGeneration

def ModelPrefixHit (receipt : CacheReceipt) : Prop :=
  receipt.modelPrefixHit = true

def beginRepair (state : IndexState) : IndexState :=
  { state with writerOnline := true }

theorem stale_delta_is_rejected
    (state : IndexState)
    (delta : IndexDelta)
    (hStale :
      delta.snapshotRevision ≤ state.providerRevision delta.provider) :
    ¬ DeltaAdmissible state delta := by
  intro hAdmissible
  exact (Nat.not_lt_of_ge hStale) hAdmissible.2.2.2

theorem offline_writer_rejects_delta
    (state : IndexState)
    (delta : IndexDelta)
    (hOffline : state.writerOnline = false) :
    ¬ DeltaAdmissible state delta := by
  intro hAdmissible
  have hFalseTrue : false = true := hOffline.symm.trans hAdmissible.1
  cases hFalseTrue

theorem distinct_provider_deltas_can_coalesce
    (left right : IndexDelta)
    (hWorkspace : left.workspace = right.workspace)
    (hBase : left.baseGeneration = right.baseGeneration)
    (hSnapshot : left.workspaceSnapshot = right.workspaceSnapshot)
    (hDistinct : left.provider ≠ right.provider) :
    CanCoalesce left right := by
  exact ⟨hWorkspace, hBase, hSnapshot, Or.inl hDistinct⟩

theorem different_workspace_deltas_cannot_coalesce
    (left right : IndexDelta)
    (hWorkspace : left.workspace ≠ right.workspace) :
    ¬ CanCoalesce left right := by
  intro hCoalesce
  exact hWorkspace hCoalesce.1

theorem different_base_generation_deltas_cannot_coalesce
    (left right : IndexDelta)
    (hBase : left.baseGeneration ≠ right.baseGeneration) :
    ¬ CanCoalesce left right := by
  intro hCoalesce
  exact hBase hCoalesce.2.1

theorem different_workspace_snapshot_deltas_cannot_coalesce
    (left right : IndexDelta)
    (hSnapshot : left.workspaceSnapshot ≠ right.workspaceSnapshot) :
    ¬ CanCoalesce left right := by
  intro hCoalesce
  exact hSnapshot hCoalesce.2.2.1

theorem same_provider_different_snapshot_cannot_coalesce
    (left right : IndexDelta)
    (hProvider : left.provider = right.provider)
    (hRevision : left.snapshotRevision ≠ right.snapshotRevision) :
    ¬ CanCoalesce left right := by
  intro hCoalesce
  cases hCoalesce.2.2.2 with
  | inl hDistinct =>
      exact hDistinct hProvider
  | inr hIdentity =>
      exact hRevision hIdentity.2.1

theorem incomplete_batch_cannot_publish
    (state : IndexState)
    (required : ProviderId → Bool)
    (batch : StagedBatch)
    (provider : ProviderId)
    (hRequired : required provider = true)
    (hMissing : batch.coveredProviders provider = false) :
    ¬ CanPublish state required batch := by
  intro hPublish
  have hCovered : batch.coveredProviders provider = true :=
    hPublish.2.2.2 provider hRequired
  have hFalseTrue : false = true := hMissing.symm.trans hCovered
  cases hFalseTrue

theorem repair_does_not_change_active_generation
    (state : IndexState) :
    (beginRepair state).activeGeneration = state.activeGeneration := by
  rfl

theorem model_prefix_hit_does_not_validate_search_cache
    (state : IndexState)
    (receipt : CacheReceipt)
    (hHit : receipt.modelPrefixHit = true)
    (hStale : receipt.searchGeneration ≠ state.activeGeneration) :
    ModelPrefixHit receipt ∧ ¬ SearchCacheValid state receipt := by
  exact ⟨hHit, hStale⟩

theorem valid_search_cache_does_not_imply_model_prefix_hit
    (state : IndexState)
    (receipt : CacheReceipt)
    (hGeneration :
      receipt.searchGeneration = state.activeGeneration)
    (hMiss : receipt.modelPrefixHit = false) :
    SearchCacheValid state receipt ∧ ¬ ModelPrefixHit receipt := by
  constructor
  · exact hGeneration
  · intro hHit
    have hFalseTrue : false = true := hMiss.symm.trans hHit
    cases hFalseTrue

end ASPProof.RuntimeServerIncrementalIndex
