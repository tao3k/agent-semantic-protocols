namespace ASPProof.RuntimeServerSearchGeneration

abbrev WorkspaceId := Nat
abbrev Generation := Nat
abbrev Revision := Nat
abbrev Digest := Nat
abbrev ProviderId := Nat

structure ServerState where
  workspace : WorkspaceId
  revision : Revision
  activeGeneration : Generation
  stagedGeneration : Option Generation
  activeDigest : Digest
  readerOnline : Bool
  writerOnline : Bool
deriving DecidableEq, Repr

structure GenerationLease where
  workspace : WorkspaceId
  generation : Generation
  revision : Revision
deriving DecidableEq, Repr

structure QueryCost where
  graphHops : Nat
  agentRounds : Nat
  exposedTokens : Nat
deriving DecidableEq, Repr

def CliMayWriteGeneration : Prop := False

def CanQuery (state : ServerState) (lease : GenerationLease) : Prop :=
  state.readerOnline = true ∧
  lease.workspace = state.workspace ∧
  lease.generation = state.activeGeneration ∧
  lease.revision ≤ state.revision

def CanQueryGeneration (state : ServerState) (generation : Generation) : Prop :=
  state.readerOnline = true ∧ generation = state.activeGeneration

def stage (state : ServerState) (generation : Generation) : ServerState :=
  { state with stagedGeneration := some generation }

def publishCAS
    (state : ServerState)
    (expectedRevision : Revision)
    (nextGeneration : Generation)
    (nextDigest : Digest) : Option ServerState :=
  if state.writerOnline = true ∧
      expectedRevision = state.revision ∧
      state.activeGeneration < nextGeneration then
    some {
      state with
      revision := state.revision + 1
      activeGeneration := nextGeneration
      stagedGeneration := none
      activeDigest := nextDigest
    }
  else
    none

def SearchLoopAvailable
    (state : ServerState)
    (providerReady : ProviderId → Bool) : Prop :=
  state.readerOnline = true ∧ ∃ provider, providerReady provider = true

def stopWriter (state : ServerState) : ServerState :=
  { state with writerOnline := false }

def Dominates (left right : QueryCost) : Prop :=
  left.graphHops ≤ right.graphHops ∧
  left.agentRounds ≤ right.agentRounds ∧
  left.exposedTokens ≤ right.exposedTokens ∧
  (left.graphHops < right.graphHops ∨
   left.agentRounds < right.agentRounds ∨
   left.exposedTokens < right.exposedTokens)

def serverWarmCost : QueryCost :=
  { graphHops := 2, agentRounds := 1, exposedTokens := 180 }

def cliRebuildCost : QueryCost :=
  { graphHops := 7, agentRounds := 4, exposedTokens := 1100 }

theorem cli_has_no_generation_write_authority : ¬ CliMayWriteGeneration := by
  intro impossible
  exact impossible

theorem staged_generation_is_not_queryable
    (state : ServerState)
    (candidate : Generation)
    (hDifferent : candidate ≠ state.activeGeneration) :
    ¬ CanQueryGeneration (stage state candidate) candidate := by
  change ¬ (state.readerOnline = true ∧ candidate = state.activeGeneration)
  intro hQuery
  exact hDifferent hQuery.2

theorem stale_revision_publish_is_rejected
    (state : ServerState)
    (expectedRevision : Revision)
    (nextGeneration : Generation)
    (nextDigest : Digest)
    (hStale : expectedRevision ≠ state.revision) :
    publishCAS state expectedRevision nextGeneration nextDigest = none := by
  unfold publishCAS
  split
  next hAccepted =>
    exact False.elim (hStale hAccepted.2.1)
  next =>
    rfl

theorem offline_writer_cannot_publish
    (state : ServerState)
    (expectedRevision : Revision)
    (nextGeneration : Generation)
    (nextDigest : Digest)
    (hOffline : state.writerOnline = false) :
    publishCAS state expectedRevision nextGeneration nextDigest = none := by
  unfold publishCAS
  split
  next hAccepted =>
    have hFalseTrue : false = true := hOffline.symm.trans hAccepted.1
    cases hFalseTrue
  next =>
    rfl

theorem non_monotone_publish_is_rejected
    (state : ServerState)
    (nextGeneration : Generation)
    (nextDigest : Digest)
    (hNotFresh : ¬ state.activeGeneration < nextGeneration) :
    publishCAS state state.revision nextGeneration nextDigest = none := by
  unfold publishCAS
  split
  next hAccepted =>
    exact False.elim (hNotFresh hAccepted.2.2)
  next =>
    rfl

theorem successful_publish_is_monotone
    (state next : ServerState)
    (expectedRevision : Revision)
    (nextGeneration : Generation)
    (nextDigest : Digest)
    (hPublished :
      publishCAS state expectedRevision nextGeneration nextDigest = some next) :
    state.activeGeneration < next.activeGeneration := by
  unfold publishCAS at hPublished
  split at hPublished
  next h =>
    cases hPublished
    exact h.2.2
  next =>
    cases hPublished

theorem successful_publish_clears_staging
    (state next : ServerState)
    (expectedRevision : Revision)
    (nextGeneration : Generation)
    (nextDigest : Digest)
    (hPublished :
      publishCAS state expectedRevision nextGeneration nextDigest = some next) :
    next.stagedGeneration = none := by
  unfold publishCAS at hPublished
  split at hPublished
  next =>
    cases hPublished
    rfl
  next =>
    cases hPublished

theorem query_lease_is_generation_consistent
    (state : ServerState)
    (lease : GenerationLease)
    (hQuery : CanQuery state lease) :
    lease.generation = state.activeGeneration := by
  exact hQuery.2.2.1

theorem one_ready_provider_preserves_searchloop
    (state : ServerState)
    (providerReady : ProviderId → Bool)
    (provider : ProviderId)
    (hOnline : state.readerOnline = true)
    (hReady : providerReady provider = true) :
    SearchLoopAvailable state providerReady := by
  exact ⟨hOnline, ⟨provider, hReady⟩⟩

theorem writer_failure_preserves_active_query
    (state : ServerState)
    (hReader : state.readerOnline = true) :
    CanQuery
      (stopWriter state)
      {
        workspace := state.workspace
        generation := state.activeGeneration
        revision := state.revision
      } := by
  exact ⟨hReader, rfl, rfl, Nat.le_refl state.revision⟩

theorem server_warm_cost_dominates_cli_rebuild :
    Dominates serverWarmCost cliRebuildCost := by
  unfold Dominates serverWarmCost cliRebuildCost
  exact ⟨by decide, by decide, by decide, Or.inl (by decide)⟩

end ASPProof.RuntimeServerSearchGeneration
