-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import Std.Tactic

namespace ASPProof.ResidentGrepCost

abbrev OwnerId := Nat
abbrev Gram := Nat
abbrev OwnerSet := OwnerId → Prop
abbrev GramSet := Gram → Prop

/-- Mathematical view of the generation-bound packed trigram directory. -/
structure TrigramIndex where
  owners : OwnerSet
  posting : Gram → OwnerSet

def candidates (index : TrigramIndex) (mandatory : GramSet) : OwnerSet :=
  fun owner => index.owners owner ∧
    ∀ gram, mandatory gram → index.posting gram owner

def SoundMandatoryGrams
    (index : TrigramIndex)
    (exactHits : OwnerSet)
    (mandatory : GramSet) : Prop :=
  ∀ owner, exactHits owner →
    index.owners owner ∧
      ∀ gram, mandatory gram → index.posting gram owner

theorem exact_hits_subset_candidates
    (index : TrigramIndex)
    (exactHits mandatory)
    (sound : SoundMandatoryGrams index exactHits mandatory) :
    ∀ owner, exactHits owner → candidates index mandatory owner := by
  intro owner hit
  exact sound owner hit

/-- Once candidate acquisition has produced any sound superset, authoritative
exact verification recovers precisely the exact hits.  Candidate intersection
may therefore stop at a singleton without changing result semantics. -/
theorem exact_verification_of_sound_candidates_is_complete
    (index : TrigramIndex)
    (exactHits mandatory)
    (sound : SoundMandatoryGrams index exactHits mandatory) :
    ∀ owner, exactHits owner ↔ candidates index mandatory owner ∧ exactHits owner := by
  intro owner
  constructor
  · intro hit
    exact ⟨sound owner hit, hit⟩
  · intro verified
    exact verified.2

/-- Rarest-first execution may stop after any non-empty subset of the proved
mandatory grams.  The partial candidate set is wider, but still contains every
exact hit and is therefore safe for authoritative exact verification. -/
theorem mandatory_subset_retains_candidate_soundness
    (index : TrigramIndex)
    (exactHits : OwnerSet)
    {mandatory selected : GramSet}
    (sound : SoundMandatoryGrams index exactHits mandatory)
    (selectedFromMandatory : ∀ gram, selected gram → mandatory gram) :
    SoundMandatoryGrams index exactHits selected := by
  intro owner hit
  have admitted := sound owner hit
  exact ⟨admitted.1, fun gram selectedGram =>
    admitted.2 gram (selectedFromMandatory gram selectedGram)⟩

theorem more_mandatory_grams_never_widen
    (index : TrigramIndex)
    {mandatory refined : GramSet}
    (refines : ∀ gram, mandatory gram → refined gram) :
    ∀ owner, candidates index refined owner → candidates index mandatory owner := by
  intro owner member
  exact ⟨member.1, fun gram gramMember => member.2 gram (refines gram gramMember)⟩

theorem empty_candidates_imply_no_exact_hits
    (index : TrigramIndex)
    (exactHits mandatory)
    (sound : SoundMandatoryGrams index exactHits mandatory)
    (empty : ∀ owner, ¬ candidates index mandatory owner) :
    ∀ owner, ¬ exactHits owner := by
  intro owner hit
  have candidate := exact_hits_subset_candidates index exactHits mandatory sound owner hit
  exact empty owner candidate

/-- Exact verification may follow intersection with a complete second axis.
No ranking limit is moved by this refinement. HIR soundness remains a separate
obligation checked against the Rust reference corpus. -/
theorem candidate_scope_pushdown_preserves_hits
    (index : TrigramIndex) (exactHits scope : OwnerSet) (mandatory : GramSet)
    (sound : SoundMandatoryGrams index exactHits mandatory) (owner : OwnerId) :
    (exactHits owner ∧ scope owner) ↔
      (candidates index mandatory owner ∧ scope owner ∧ exactHits owner) := by
  constructor
  · intro hit
    exact ⟨sound owner hit.1, hit.2, hit.1⟩
  · intro hit
    exact ⟨hit.2.2, hit.2.1⟩

/-- When rg's exact admitted owner paths are already an upper bound for its
result, pushing that scope into Tantivy is the same set intersection as
searching the generation first. Ranking is deliberately absent from V1 fusion. -/
def scopedTantivyCandidates (tantivy scope : OwnerSet) : OwnerSet :=
  fun owner => tantivy owner ∧ scope owner

theorem exact_rg_owner_scope_pushdown_preserves_fusion
    (tantivy exactRgScope : OwnerSet) (owner : OwnerId) :
    (tantivy owner ∧ exactRgScope owner) ↔
      scopedTantivyCandidates tantivy exactRgScope owner := by
  rfl

/-- A regex plan without a sound mandatory trigram is typed not-materialized. -/
inductive PlanAdmission where
  | selective
  | fusedScope
  | notMaterialized
  deriving DecidableEq, Repr

def boundedAdmission : PlanAdmission → Bool
  | .selective | .fusedScope => true
  | .notMaterialized => false

def mayPublishAbsence
    (admission : PlanAdmission)
    (exactVerificationComplete candidateSetEmpty : Bool) : Prop :=
  boundedAdmission admission = true ∧
    exactVerificationComplete = true ∧
    candidateSetEmpty = true

theorem not_materialized_cannot_publish_absence
    (verified empty : Bool) :
    ¬ mayPublishAbsence .notMaterialized verified empty := by
  simp [mayPublishAbsence, boundedAdmission]

/-- Completeness of the scope is an explicit obligation, not a receipt label. -/
theorem complete_fused_scope_empty_implies_absence
    (scope exactHits : OwnerSet)
    (covers : ∀ owner, exactHits owner → scope owner)
    (verifiedEmpty : ∀ owner, ¬ (scope owner ∧ exactHits owner)) :
    ∀ owner, ¬ exactHits owner := by
  intro owner hit
  exact verifiedEmpty owner ⟨covers owner hit, hit⟩

/-- Field-for-field cost-bearing subset of the Rust resident GREP receipt. -/
structure RustReceipt where
  candidateGramCount : Nat
  decodedPostingCount : Nat
  smallestPostingCount : Nat
  candidateOwnerCount : Nat
  residentOwnerReadCount : Nat
  processCount : Nat
  filesystemOperationCount : Nat
  deriving DecidableEq, Repr

def externallyPure (receipt : RustReceipt) : Prop :=
  receipt.processCount = 0 ∧ receipt.filesystemOperationCount = 0

theorem externally_pure_has_no_binary_or_filesystem_work
    (receipt : RustReceipt)
    (pure : externallyPure receipt) :
    receipt.processCount + receipt.filesystemOperationCount = 0 := by
  rcases pure with ⟨processes, filesystem⟩
  omega

/-- Counts are logical work, not elapsed time or an executable Rust refinement. -/
def currentLogicalWork (receipt : RustReceipt) : Nat :=
  receipt.decodedPostingCount + receipt.residentOwnerReadCount

/-- Sequential delta streams must traverse prefixes, not constant-time probes. -/
def prefixDecodeWork (lengths : List Nat) (stopAfter : Nat) : Nat :=
  (lengths.map (fun length => min length stopAfter)).sum

theorem prefix_decode_bounded_by_full_decode (lengths : List Nat) (stopAfter : Nat) :
    prefixDecodeWork lengths stopAfter ≤ lengths.sum := by
  induction lengths with
  | nil => simp [prefixDecodeWork]
  | cons head tail ih =>
    simp only [prefixDecodeWork, List.map_cons, List.sum_cons] at *
    have bound : min head stopAfter ≤ head := Nat.min_le_left _ _
    omega

/-- Even eliminating all optimizable work cannot eliminate fixed request cost. -/
theorem fixed_cost_blocks_target_factor (fixed before after factor : Nat)
    (floor : fixed + before < factor * fixed) :
    ¬ factor * (fixed + after) ≤ fixed + before := by
  have bound : factor * fixed ≤ factor * (fixed + after) :=
    Nat.mul_le_mul_left factor (Nat.le_add_right fixed after)
  omega

/-- Limit-before-intersection can lose a hit; do not infer global absence. -/
theorem limit_before_intersection_loses_hit :
    (([0, 1] : List Nat).take 1).filter (fun owner => owner == 1) = [] ∧
    (([0, 1] : List Nat).filter (fun owner => owner == 1)).take 1 = [1] := by
  decide

/-- A candidate accelerator may return false positives. Applying the public
result limit before exact verification can therefore erase a real match. -/
theorem candidate_limit_before_exact_verification_loses_hit :
    ([false, true].take 1).filter id = [] ∧
    ([false, true].filter id).take 1 = [true] := by
  decide

inductive GrepQualificationLane where
  | residentNormal
  | externalReference
  deriving DecidableEq, Repr

def grepQualificationProcessCount : GrepQualificationLane → Nat
  | .residentNormal => 0
  | .externalReference => 1

theorem resident_semantic_matrix_has_zero_external_processes :
    grepQualificationProcessCount .residentNormal = 0 := by
  rfl

theorem external_rg_is_confined_to_reference_qualification :
    grepQualificationProcessCount .externalReference = 1 ∧
    grepQualificationProcessCount .residentNormal = 0 := by
  decide

def applyOrderedGlobRule (current isMatch isInclude : Bool) : Bool :=
  if isMatch then isInclude else current

theorem later_matching_positive_glob_reincludes :
    applyOrderedGlobRule (applyOrderedGlobRule true true false) true true = true := by
  rfl

theorem later_matching_exclusion_glob_wins :
    applyOrderedGlobRule (applyOrderedGlobRule false true true) true false = false := by
  rfl

theorem nonmatching_glob_preserves_prior_decision (current isInclude : Bool) :
    applyOrderedGlobRule current false isInclude = current := by
  rfl

def packedGramBytes : Nat := 4
def directoryEntryBytes : Nat := 32

theorem v1_directory_field_accounting :
    packedGramBytes + 4 + 8 + 8 + 8 = directoryEntryBytes := by
  decide

structure MemoryModel where
  postingHeapBytes : Nat
  corpusHeapBytes : Nat
  mappedArtifactBytes : Nat
  deriving DecidableEq, Repr

def zeroCopyQueryState (memory : MemoryModel) : Prop :=
  memory.postingHeapBytes = 0 ∧ memory.corpusHeapBytes = 0

theorem mapped_generation_has_zero_workspace_heap_retention
    (memory : MemoryModel)
    (zeroCopy : zeroCopyQueryState memory) :
    memory.postingHeapBytes + memory.corpusHeapBytes = 0 := by
  rcases zeroCopy with ⟨postings, corpus⟩
  omega

/-- The Rust builder's flat `(gram, owner-id)` arena retains one fixed-width
record per posting.  A map of independently allocated posting vectors retains
the same records plus at least one allocator header per distinct gram. -/
def flatPostingArenaBytes (postingCount recordBytes : Nat) : Nat :=
  postingCount * recordBytes

def fragmentedPostingArenaBytes
    (postingCount distinctGramCount recordBytes vectorHeaderBytes : Nat) : Nat :=
  flatPostingArenaBytes postingCount recordBytes +
    distinctGramCount * vectorHeaderBytes

theorem flat_posting_arena_never_exceeds_fragmented
    (postingCount distinctGramCount recordBytes vectorHeaderBytes : Nat) :
    flatPostingArenaBytes postingCount recordBytes ≤
      fragmentedPostingArenaBytes
        postingCount distinctGramCount recordBytes vectorHeaderBytes := by
  unfold fragmentedPostingArenaBytes
  omega

theorem fragmented_arena_overhead_is_per_gram
    (postingCount distinctGramCount recordBytes vectorHeaderBytes : Nat) :
    fragmentedPostingArenaBytes
        postingCount distinctGramCount recordBytes vectorHeaderBytes =
      flatPostingArenaBytes postingCount recordBytes +
        distinctGramCount * vectorHeaderBytes := rfl

structure TokioCpuLaneModel where
  laneLimit : Nat
  activeCpuLanes : Nat
  reactorCpuScans : Nat
  deriving DecidableEq, Repr

def schedulerAdmitted (model : TokioCpuLaneModel) : Prop :=
  model.activeCpuLanes ≤ model.laneLimit ∧ model.reactorCpuScans = 0

theorem admitted_tokio_lane_preserves_reactor
    (model : TokioCpuLaneModel)
    (admitted : schedulerAdmitted model) :
    model.reactorCpuScans = 0 := admitted.2

/-- The premise must come from hashing the captured bytes, not an external receipt.
This models comparison equivalence, not collision resistance or Rust refinement. -/
theorem cached_digest_check_equivalent {Bytes Digest : Type}
    (hash : Bytes → Digest) (bytes : Bytes) (cached proof : Digest)
    (sameBytes : cached = hash bytes) :
    (proof = cached) ↔ (proof = hash bytes) := by
  rw [sameBytes]

/-- Old assembly hashes once per owner plus once per selector. -/
theorem owner_hash_work_decomposition (bytes selectors : Nat) :
    bytes * (selectors + 1) = bytes + bytes * selectors := by
  simp [Nat.mul_add, Nat.add_comm]

theorem owner_hash_reuse_never_increases_byte_work (bytes selectors : Nat) :
    bytes ≤ bytes * (selectors + 1) := by
  rw [owner_hash_work_decomposition]
  omega

/-- Queueing on a blocking worker moves work; it does not eliminate work. -/
def offloadedAssembly (work : Nat) : Nat × Nat := (0, work)

theorem offloaded_assembly_has_no_reactor_work (work : Nat) :
    (offloadedAssembly work).1 = 0 ∧ (offloadedAssembly work).2 = work := by
  simp [offloadedAssembly]

structure RecoveryIdentity where
  source : Nat
  execution : Nat
  scope : Nat
  deriving DecidableEq

def mayReuseRecovery (stored current : RecoveryIdentity) : Prop := stored = current

theorem changed_source_rejects_recovery (stored current : RecoveryIdentity)
    (changed : stored.source ≠ current.source) : ¬ mayReuseRecovery stored current := by
  intro same
  exact changed (congrArg RecoveryIdentity.source same)

/-- A counterexample: identical request lanes can refer to different source. -/
theorem same_lane_does_not_prove_freshness :
    (7 : Nat) = 7 ∧ ¬ mayReuseRecovery ⟨1, 2, 3⟩ ⟨4, 2, 3⟩ := by
  constructor
  · rfl
  · unfold mayReuseRecovery
    decide

/-- A process-cold reopen may establish freshness from a current snapshot or
from a gap-free continuity authority that outlives the Runtime process.  A
persisted request lane is only a coalescing identity. -/
inductive RecoveryFreshnessAuthority where
  | currentSnapshot
  | uninterruptedContinuity
  | persistedLane
  deriving DecidableEq, Repr

def admitsRecoveryFreshness
    (authority : RecoveryFreshnessAuthority)
    (sourceEqual : Bool)
    (filesystemReads : Nat) : Prop :=
  sourceEqual = true ∧
    match authority with
    | .currentSnapshot => 0 < filesystemReads
    | .uninterruptedContinuity => filesystemReads = 0
    | .persistedLane => False

theorem zero_io_without_continuity_cannot_admit
    (sourceEqual : Bool) :
    ¬ admitsRecoveryFreshness .persistedLane sourceEqual 0 := by
  simp [admitsRecoveryFreshness]

theorem current_snapshot_admission_observes_workspace
    (filesystemReads : Nat)
    (observed : 0 < filesystemReads) :
    admitsRecoveryFreshness .currentSnapshot true filesystemReads := by
  exact ⟨rfl, observed⟩

theorem uninterrupted_continuity_admits_zero_io :
    admitsRecoveryFreshness .uninterruptedContinuity true 0 := by
  simp [admitsRecoveryFreshness]

theorem changed_execution_rejects_recovery (stored current : RecoveryIdentity)
    (changed : stored.execution ≠ current.execution) : ¬ mayReuseRecovery stored current := by
  intro same
  exact changed (congrArg RecoveryIdentity.execution same)

theorem changed_scope_rejects_recovery (stored current : RecoveryIdentity)
    (changed : stored.scope ≠ current.scope) : ¬ mayReuseRecovery stored current := by
  intro same
  exact changed (congrArg RecoveryIdentity.scope same)

/-- Abstract retained completion state. Runtime tests, not this model, must
establish the watch/mutex implementation's ordering and race behavior. -/
structure CompletionState where
  terminal : Bool
  notified : Bool
  deriving DecidableEq

def publishCompletion : CompletionState := ⟨true, true⟩
def cancelWaiter (shared : CompletionState) : CompletionState := shared

theorem publication_retains_terminal_for_late_waiters :
    publishCompletion.terminal = true ∧ publishCompletion.notified = true := by decide

theorem waiter_cancellation_preserves_shared_terminal (shared : CompletionState) :
    cancelWaiter shared = shared := rfl

/-- One semantic materialization claim serves every identical live caller.
Rust tests establish atomicity and retained-event ordering; this model states
the protocol cardinality and forbids caller-driven retries. -/
structure FirstCallSingleFlight where
  callers : Nat
  computationClaims : Nat
  terminalWaiters : Nat
  callerRetries : Nat
  publicBuildingTerminals : Nat
  deriving DecidableEq, Repr

def firstCallSingleFlight (callers : Nat) : FirstCallSingleFlight :=
  { callers
    computationClaims := if callers = 0 then 0 else 1
    terminalWaiters := callers
    callerRetries := 0
    publicBuildingTerminals := 0 }

theorem one_claim_serves_all_waiters (callers : Nat) (live : 0 < callers) :
    (firstCallSingleFlight callers).computationClaims = 1 ∧
      (firstCallSingleFlight callers).terminalWaiters = callers := by
  simp [firstCallSingleFlight, Nat.ne_of_gt live]

theorem no_public_building_terminal (callers : Nat) :
    (firstCallSingleFlight callers).callerRetries = 0 ∧
      (firstCallSingleFlight callers).publicBuildingTerminals = 0 := by
  simp [firstCallSingleFlight]

/-- Request-local policy only; no wall-clock or scheduler guarantee is assumed. -/
inductive RequestPhase where
  | resident
  | firstComputation
  deriving DecidableEq

def requestBudget (phase : RequestPhase) (observationBudget : Nat) : Nat :=
  match phase with
  | .resident => 1000
  | .firstComputation => observationBudget

def observeMiss (_phase : RequestPhase) : RequestPhase := .firstComputation

theorem ready_phase_keeps_resident_budget (observationBudget : Nat) :
    requestBudget .resident observationBudget = 1000 := rfl

theorem first_phase_is_monotone :
    observeMiss (observeMiss .resident) = observeMiss .resident := rfl

/-- Logical work decomposition for one empty-cache Search.  Attachment fields
are modeled explicitly so they cannot disappear inside a latency label. -/
structure FirstSearchWork where
  sourceBytes : Nat
  snapshotWork : Nat
  searchCoreWork : Nat
  exactVerificationWork : Nat
  candidateParserWork : Nat
  requestRefinementWork : Nat
  fullParserProjectionWork : Nat
  durabilityWork : Nat
  globalGraphWork : Nat
  deriving DecidableEq, Repr

def splitCriticalPathWork (work : FirstSearchWork) : Nat :=
  work.snapshotWork + work.searchCoreWork + work.exactVerificationWork +
    work.candidateParserWork + work.requestRefinementWork

def blockingLegacyWork (work : FirstSearchWork) : Nat :=
  splitCriticalPathWork work + work.fullParserProjectionWork +
    work.durabilityWork + work.globalGraphWork

theorem first_search_work_excludes_attachments (work : FirstSearchWork) :
    splitCriticalPathWork work =
      work.snapshotWork + work.searchCoreWork + work.exactVerificationWork +
        work.candidateParserWork + work.requestRefinementWork := rfl

theorem split_critical_path_never_adds_work (work : FirstSearchWork) :
    splitCriticalPathWork work ≤ blockingLegacyWork work := by
  unfold blockingLegacyWork
  omega

/-- Capturing previously unknown exact bytes is not constant logical work.
This is not a wall-clock lower-bound theorem. -/
def ExactSourceAcquisition (work : FirstSearchWork) : Prop :=
  work.sourceBytes ≤ work.snapshotWork

theorem source_acquisition_has_byte_lower_bound (work : FirstSearchWork)
    (exact : ExactSourceAcquisition work) :
    work.sourceBytes ≤ splitCriticalPathWork work := by
  unfold ExactSourceAcquisition at exact
  unfold splitCriticalPathWork
  omega

/-- Parser artifacts are content-addressed independently from the generation
proof that admits them into one current workspace. -/
structure ParserArtifactIdentity where
  provider : Nat
  parser : Nat
  queryPack : Nat
  auxiliaryInput : Nat
  ownerPath : Nat
  ownerContent : Nat
  deriving DecidableEq, Repr

structure ParserArtifactBinding where
  artifact : ParserArtifactIdentity
  generationProof : Nat
  deriving DecidableEq, Repr

def parserArtifactReusable
    (stored current : ParserArtifactBinding) : Prop :=
  stored.artifact = current.artifact

theorem generation_rebind_preserves_parser_artifact
    (artifact : ParserArtifactIdentity) (oldProof newProof : Nat) :
    parserArtifactReusable ⟨artifact, oldProof⟩ ⟨artifact, newProof⟩ := rfl

theorem changed_owner_content_rejects_parser_artifact
    (stored current : ParserArtifactBinding)
    (changed : stored.artifact.ownerContent ≠ current.artifact.ownerContent) :
    ¬ parserArtifactReusable stored current := by
  intro reusable
  exact changed (congrArg (fun binding => binding.ownerContent) reusable)

/-- An owner binds only auxiliary inputs in its semantic ancestor cut.  An
unrelated nested configuration therefore cannot invalidate its parser
artifact. -/
def applicableAuxiliaryInputs
    (applies : Nat → Nat → Bool)
    (ownerPath : Nat)
    (inputs : List (Nat × Nat)) : List (Nat × Nat) :=
  inputs.filter fun input => applies input.1 ownerPath

theorem unrelated_auxiliary_is_outside_owner_cut
    (applies : Nat → Nat → Bool)
    (ownerPath : Nat)
    (inputs : List (Nat × Nat))
    (unrelated : Nat × Nat)
    (outside : applies unrelated.1 ownerPath = false) :
    applicableAuxiliaryInputs applies ownerPath (unrelated :: inputs) =
      applicableAuxiliaryInputs applies ownerPath inputs := by
  simp [applicableAuxiliaryInputs, outside]

theorem applicable_auxiliary_enters_owner_cut
    (applies : Nat → Nat → Bool)
    (ownerPath : Nat)
    (inputs : List (Nat × Nat))
    (related : Nat × Nat)
    (inside : applies related.1 ownerPath = true) :
    applicableAuxiliaryInputs applies ownerPath (related :: inputs) =
      related :: applicableAuxiliaryInputs applies ownerPath inputs := by
  simp [applicableAuxiliaryInputs, inside]

/-- Resident readability and restart restoration are deliberately different
states.  A durability error cannot retroactively invalidate admitted memory. -/
inductive InternalGenerationReadiness where
  | residentReady
  | durableReady
  | durabilityFailed
  deriving DecidableEq, Repr

def processReadable : InternalGenerationReadiness → Bool
  | .residentReady | .durableReady | .durabilityFailed => true

def restartRestorable : InternalGenerationReadiness → Bool
  | .durableReady => true
  | .residentReady | .durabilityFailed => false

theorem failed_durability_preserves_resident_read :
    processReadable .durabilityFailed = true := rfl

theorem resident_is_not_restart_restorable :
    restartRestorable .residentReady = false := rfl

theorem candidate_projection_work_is_bounded
    (candidateOwners allOwners workPerOwner : Nat)
    (subset : candidateOwners ≤ allOwners) :
    candidateOwners * workPerOwner ≤ allOwners * workPerOwner :=
  Nat.mul_le_mul_right workPerOwner subset

/-- Cache admission precedes provider lifecycle work. -/
inductive ParserArtifactLookup where
  | hit
  | miss
  deriving DecidableEq, Repr

def providerStarts : ParserArtifactLookup → Nat
  | .hit => 0
  | .miss => 1

theorem parser_cache_hit_has_zero_provider_starts :
    providerStarts .hit = 0 := rfl

/-- Candidate parsing has owner-proportional payload work, but provider/frame
setup is paid once per provider group rather than once per owner. -/
def serialOwnerProjectionWork
    (owners setupPerRpc payloadPerOwner : Nat) : Nat :=
  owners * setupPerRpc + owners * payloadPerOwner

def groupedOwnerProjectionWork
    (owners providerGroups setupPerRpc payloadPerOwner : Nat) : Nat :=
  providerGroups * setupPerRpc + owners * payloadPerOwner

theorem provider_group_batch_never_exceeds_serial_owner_rpc
    (owners providerGroups setupPerRpc payloadPerOwner : Nat)
    (groupsBounded : providerGroups ≤ owners) :
    groupedOwnerProjectionWork owners providerGroups setupPerRpc payloadPerOwner ≤
      serialOwnerProjectionWork owners setupPerRpc payloadPerOwner := by
  unfold groupedOwnerProjectionWork serialOwnerProjectionWork
  exact Nat.add_le_add_right
    (Nat.mul_le_mul_right setupPerRpc groupsBounded)
    (owners * payloadPerOwner)

/-- A complete-generation producer batch pays the workspace inventory term
once.  The former serial-successor shape is exactly that work plus one
duplicate inventory term for every producer after the first. -/
def oneGenerationBarrierWork (inventoryWork producerWork : Nat) : Nat :=
  inventoryWork + producerWork

def serialGenerationSuccessorWork
    (producerCount inventoryWork producerWork : Nat) : Nat :=
  oneGenerationBarrierWork inventoryWork producerWork +
    (producerCount - 1) * inventoryWork

theorem one_generation_barrier_removes_duplicate_inventory
    (producerCount inventoryWork producerWork : Nat) :
    oneGenerationBarrierWork inventoryWork producerWork ≤
      serialGenerationSuccessorWork producerCount inventoryWork producerWork := by
  unfold serialGenerationSuccessorWork
  exact Nat.le_add_right _ _

/-- Process-cold exact replay is a request-local proof over the immutable
segment and the current owner bytes.  It cannot mint workspace Ready. -/
structure ProcessColdExactReplay where
  durableExecutionBound : Bool
  currentRuntimeBundleMatches : Bool
  currentOwnerDigestMatches : Bool
  exactSelectorHit : Bool
  relocationRequired : Bool
  deriving DecidableEq, Repr

/-- Stable V1 keeps one digest payload at two representation boundaries:
Query/exact-segment wire hex and typed Runtime content identity. -/
inductive DigestBoundary where
  | queryWireHex
  | runtimeTyped
  deriving DecidableEq, Repr

structure BoundaryDigest where
  payload : Nat
  boundary : DigestBoundary
  deriving DecidableEq, Repr

def sameDigestContent (left right : BoundaryDigest) : Prop :=
  left.payload = right.payload

theorem v1_digest_boundary_translation_preserves_content (payload : Nat) :
    sameDigestContent
      { payload := payload, boundary := .queryWireHex }
      { payload := payload, boundary := .runtimeTyped } := by
  rfl

/-- Work counters are the architectural process-cold performance gate. -/
structure ProcessColdReplayWork where
  pipelineTasks : Nat
  ownerSnapshotCopies : Nat
  providerStarts : Nat
  fullGenerationAdmissions : Nat
  asyncWorkerCpuTasks : Nat
  blockingTaskCpuPermits : Nat
  blockingTaskMemoryPermits : Nat
  permitsOwnedByBlockingTasks : Bool
  runtimeWorkerThreads : Nat
  backgroundCpuCapacity : Nat
  deriving DecidableEq, Repr

def exactOwnerReplayWork : ProcessColdReplayWork where
  pipelineTasks := 1
  ownerSnapshotCopies := 0
  providerStarts := 0
  fullGenerationAdmissions := 0
  asyncWorkerCpuTasks := 0
  blockingTaskCpuPermits := 1
  blockingTaskMemoryPermits := 1
  permitsOwnedByBlockingTasks := true
  runtimeWorkerThreads := 4
  backgroundCpuCapacity := 3

theorem exact_owner_replay_has_no_provider_generation_or_async_cpu_work :
    exactOwnerReplayWork.providerStarts +
      exactOwnerReplayWork.fullGenerationAdmissions +
      exactOwnerReplayWork.asyncWorkerCpuTasks = 0 := by
  rfl

theorem exact_owner_replay_is_one_resource_owned_blocking_pipeline :
    exactOwnerReplayWork.pipelineTasks = 1 ∧
      exactOwnerReplayWork.blockingTaskCpuPermits = 1 ∧
      exactOwnerReplayWork.blockingTaskMemoryPermits = 1 ∧
      exactOwnerReplayWork.permitsOwnedByBlockingTasks = true := by
  decide

theorem exact_owner_replay_preflight_copies_no_owner_snapshot :
    exactOwnerReplayWork.ownerSnapshotCopies = 0 := by
  rfl

theorem four_thread_runtime_reserves_one_worker_from_background_cpu :
    exactOwnerReplayWork.runtimeWorkerThreads =
      exactOwnerReplayWork.backgroundCpuCapacity + 1 := by
  decide

def processColdExactReplayAdmitted (replay : ProcessColdExactReplay) : Bool :=
  replay.durableExecutionBound &&
    replay.currentRuntimeBundleMatches &&
    replay.currentOwnerDigestMatches &&
    replay.exactSelectorHit &&
    !replay.relocationRequired

def processColdExactReplayPublishesWorkspaceReady
    (_replay : ProcessColdExactReplay) : Bool := false

theorem process_cold_exact_replay_never_publishes_workspace_ready
    (replay : ProcessColdExactReplay) :
    processColdExactReplayPublishesWorkspaceReady replay = false := rfl

theorem changed_owner_rejects_process_cold_exact_replay
    (replay : ProcessColdExactReplay)
    (changed : replay.currentOwnerDigestMatches = false) :
    processColdExactReplayAdmitted replay = false := by
  simp [processColdExactReplayAdmitted, changed]

theorem changed_runtime_bundle_rejects_process_cold_exact_replay
    (replay : ProcessColdExactReplay)
    (changed : replay.currentRuntimeBundleMatches = false) :
    processColdExactReplayAdmitted replay = false := by
  simp [processColdExactReplayAdmitted, changed]

theorem relocation_rejects_process_cold_exact_replay
    (replay : ProcessColdExactReplay)
    (relocation : replay.relocationRequired = true) :
    processColdExactReplayAdmitted replay = false := by
  simp [processColdExactReplayAdmitted, relocation]

/-- Request graph construction is proportional to the bounded materialized
owner cut, never implicitly to every workspace owner. -/
def requestGraphWork (owners workPerOwner : Nat) : Nat := owners * workPerOwner

theorem request_graph_work_is_candidate_bounded
    (requestOwners allOwners workPerOwner : Nat)
    (subset : requestOwners ≤ allOwners) :
    requestGraphWork requestOwners workPerOwner ≤
      requestGraphWork allOwners workPerOwner := by
  unfold requestGraphWork
  exact Nat.mul_le_mul_right workPerOwner subset

/-- Publishing a parser result into the resident read model contains no
workspace-sized rewrite term. Durable parser artifact I/O is accounted for by
candidate bytes, independently of the canonical generation. -/
def residentSemanticPublicationWork (candidateBytes : Nat) : Nat := candidateBytes

def canonicalRewritePublicationWork
    (candidateBytes workspaceBytes : Nat) : Nat := candidateBytes + workspaceBytes

theorem resident_semantic_publication_excludes_workspace_rewrite
    (candidateBytes workspaceBytes : Nat) :
    residentSemanticPublicationWork candidateBytes ≤
      canonicalRewritePublicationWork candidateBytes workspaceBytes := by
  unfold residentSemanticPublicationWork canonicalRewritePublicationWork
  omega

/-- A resident overlay is a read-through reference to the immutable base plus
delta-owned bytes. Copying base owner bytes into the overlay violates the
candidate-bounded publication model even when no canonical file is rewritten. -/
structure ResidentOverlayPublication where
  copiedBaseOwnerBytes : Nat
  deltaOwnerBytes : Nat
  deriving DecidableEq, Repr

def residentOverlayPublicationWork (publication : ResidentOverlayPublication) : Nat :=
  publication.copiedBaseOwnerBytes + publication.deltaOwnerBytes

def deltaOnlyOverlay (publication : ResidentOverlayPublication) : Prop :=
  publication.copiedBaseOwnerBytes = 0

theorem delta_only_overlay_work_equals_delta
    (publication : ResidentOverlayPublication)
    (deltaOnly : deltaOnlyOverlay publication) :
    residentOverlayPublicationWork publication = publication.deltaOwnerBytes := by
  unfold deltaOnlyOverlay at deltaOnly
  unfold residentOverlayPublicationWork
  omega

theorem copying_base_bytes_adds_forbidden_work (baseBytes deltaBytes : Nat) :
    residentOverlayPublicationWork ⟨baseBytes, deltaBytes⟩ = baseBytes + deltaBytes := rfl

/-- The generation-bound Search data plane is constructed at admission. Read
handles retain that immutable plane by reference; they do not repeat any
workspace-proportional validation or index construction. `Arc` behavior and
wall-clock complexity remain Rust obligations, not claims of this model. -/
structure SearchPlaneLifecycleWork where
  admissionBuildCount : Nat
  readHandleCount : Nat
  readHandleRebuildCount : Nat
  deriving DecidableEq, Repr

def admittedSearchPlane (work : SearchPlaneLifecycleWork) : Prop :=
  work.admissionBuildCount = 1 ∧ work.readHandleRebuildCount = 0

def requestWorkspaceBuildWork
    (work : SearchPlaneLifecycleWork) (workspaceBuildWork : Nat) : Nat :=
  work.readHandleRebuildCount * workspaceBuildWork

def handleReferenceWork
    (work : SearchPlaneLifecycleWork)
    (referenceWork : Nat)
    (_workspaceBuildWork : Nat) : Nat :=
  work.readHandleCount * referenceWork

theorem admitted_search_plane_has_zero_request_workspace_build
    (work : SearchPlaneLifecycleWork)
    (workspaceBuildWork : Nat)
    (admitted : admittedSearchPlane work) :
    requestWorkspaceBuildWork work workspaceBuildWork = 0 := by
  unfold requestWorkspaceBuildWork
  rw [admitted.2]
  simp

theorem read_handle_reference_work_is_independent_of_workspace
    (handles referenceWork firstWorkspaceWork secondWorkspaceWork : Nat) :
    handleReferenceWork ⟨1, handles, 0⟩ referenceWork firstWorkspaceWork =
      handleReferenceWork ⟨1, handles, 0⟩ referenceWork secondWorkspaceWork := by
  rfl

theorem rebuilding_each_handle_multiplies_workspace_work
    (handles workspaceBuildWork : Nat) :
    requestWorkspaceBuildWork ⟨1, handles, handles⟩ workspaceBuildWork =
      handles * workspaceBuildWork := rfl

/-- Internally encoded byte coverage carries its layout directly. Externally
restored bytes still cross one complete validation boundary. -/
inductive ByteCoverageArtifactOrigin where
  | internallyConstructed
  | externallyRestored
  deriving DecidableEq, Repr

def fullByteCoverageValidationPasses : ByteCoverageArtifactOrigin → Nat
  | .internallyConstructed => 0
  | .externallyRestored => 1

theorem internal_byte_coverage_has_no_duplicate_full_validation :
    fullByteCoverageValidationPasses .internallyConstructed = 0 := rfl

theorem restored_byte_coverage_retains_full_validation :
    fullByteCoverageValidationPasses .externallyRestored = 1 := rfl

/-- Internally constructed settlement values already crossed typed input
validation. External JSON remains untrusted and must cross full admission. -/
inductive SettlementOrigin where
  | internalTyped
  | externalJson
  deriving DecidableEq, Repr

def settlementDecodeAdmissions : SettlementOrigin → Nat
  | .internalTyped => 0
  | .externalJson => 1

theorem internal_settlement_has_no_duplicate_decode :
    settlementDecodeAdmissions .internalTyped = 0 := rfl

theorem external_settlement_retains_full_admission :
    settlementDecodeAdmissions .externalJson = 1 := rfl

/-- A ready candidate set forks its generation lease. Registry acquisition is
reserved for a materialization miss. -/
def residentRegistryAcquisitions (allOwnersReady : Bool) : Nat :=
  if allOwnersReady then 0 else 2

theorem ready_owner_scope_has_zero_registry_acquisitions :
    residentRegistryAcquisitions true = 0 := rfl

/-- Generation-owned topology indexes make request lookup depend only on the
selected bounded subgraph, not on complete workspace topology size. -/
def indexedSettlementLookupWork
    (selectedNodes incidentEdges _totalNodes _totalEdges : Nat) : Nat :=
  selectedNodes + 2 * incidentEdges

def fullScanSettlementLookupWork
    (evidenceItems totalNodes totalEdges : Nat) : Nat :=
  evidenceItems * totalNodes + totalNodes + 2 * totalEdges

theorem indexed_settlement_work_is_independent_of_workspace
    (selectedNodes incidentEdges firstTotalNodes secondTotalNodes
      firstTotalEdges secondTotalEdges : Nat) :
    indexedSettlementLookupWork selectedNodes incidentEdges
        firstTotalNodes firstTotalEdges =
      indexedSettlementLookupWork selectedNodes incidentEdges
        secondTotalNodes secondTotalEdges := by
  rfl

theorem full_scan_settlement_pays_workspace_node_scan
    (evidenceItems totalNodes totalEdges : Nat) :
    totalNodes ≤
      fullScanSettlementLookupWork evidenceItems totalNodes totalEdges := by
  unfold fullScanSettlementLookupWork
  omega

/-- A one-slot topology cache is reusable only for the same immutable
generation and the same canonical owner scope. -/
structure TopologyCacheIdentity where
  generation : Nat
  topologySourceGeneration : Nat
  ownerScope : List Nat
  deriving DecidableEq, Repr

def topologyCacheReusable
    (stored requested : TopologyCacheIdentity) : Prop :=
  stored = requested

theorem changed_topology_owner_scope_rejects_cache
    (stored requested : TopologyCacheIdentity)
    (changed : stored.ownerScope ≠ requested.ownerScope) :
    ¬ topologyCacheReusable stored requested := by
  intro same
  exact changed (congrArg TopologyCacheIdentity.ownerScope same)

theorem changed_topology_source_generation_rejects_cache
    (stored requested : TopologyCacheIdentity)
    (changed : stored.topologySourceGeneration ≠ requested.topologySourceGeneration) :
    ¬ topologyCacheReusable stored requested := by
  intro same
  exact changed (congrArg TopologyCacheIdentity.topologySourceGeneration same)

theorem exact_topology_identity_reuses_cache (identity : TopologyCacheIdentity) :
    topologyCacheReusable identity identity := rfl

/-- Graph entry-node grounding is a generation-admission index lookup.  The
workspace cardinality is retained in the model so a request-time full scan
cannot be hidden inside the same cost term. -/
structure GraphEntryGroundingWork where
  workspaceOwners : Nat
  requestedEntryNodes : Nat
  indexedLookups : Nat
  requestWorkspaceScans : Nat
  deriving DecidableEq, Repr

def indexedGraphEntryGrounding (workspaceOwners requestedEntryNodes : Nat) :
    GraphEntryGroundingWork :=
  ⟨workspaceOwners, requestedEntryNodes, requestedEntryNodes, 0⟩

theorem graph_entry_grounding_has_zero_request_workspace_scans
    (workspaceOwners requestedEntryNodes : Nat) :
    let work := indexedGraphEntryGrounding workspaceOwners requestedEntryNodes
    work.requestWorkspaceScans = 0 ∧
      work.indexedLookups = requestedEntryNodes := by
  exact ⟨rfl, rfl⟩

/-- Blocking topology work owns its resource permit independently from the
request future. Tokio cannot cancel a running blocking closure, so request
cancellation is not a resource terminal. -/
def topologyBlockingMemoryPermitHeld
    (_callerCancelled blockingTerminal : Bool) : Bool :=
  !blockingTerminal

theorem cancelled_topology_caller_retains_blocking_memory_authority :
    topologyBlockingMemoryPermitHeld true false = true := by
  rfl

theorem topology_blocking_terminal_releases_memory_authority
    (callerCancelled : Bool) :
    topologyBlockingMemoryPermitHeld callerCancelled true = false := by
  rfl

/-- Runtime resource admission is a product, so zero or out-of-authority byte
work cannot hide behind a valid CPU or memory claim. -/
structure UnifiedRuntimeResourceRequest where
  cpu : Nat
  workBytes : Nat
  memoryBytes : Nat
  deriving DecidableEq, Repr

def unifiedRuntimeResourceAdmitted
    (backgroundCpu workByteBudget memoryBudget : Nat)
    (request : UnifiedRuntimeResourceRequest) : Prop :=
  0 < request.cpu ∧ request.cpu ≤ backgroundCpu ∧
  0 < request.workBytes ∧ request.workBytes ≤ workByteBudget ∧
  0 < request.memoryBytes ∧ request.memoryBytes ≤ memoryBudget

def runtimeQueueCapacity (backgroundCpu : Nat) : Nat := backgroundCpu

theorem runtime_queue_is_bounded_by_background_cpu (backgroundCpu : Nat) :
    runtimeQueueCapacity backgroundCpu = backgroundCpu := by
  rfl

theorem zero_byte_work_is_not_admitted
    (backgroundCpu workByteBudget memoryBudget cpu memoryBytes : Nat) :
    ¬ unifiedRuntimeResourceAdmitted backgroundCpu workByteBudget memoryBudget
      ⟨cpu, 0, memoryBytes⟩ := by
  simp [unifiedRuntimeResourceAdmitted]

def aggregateWorkBytesAdmitted
    (workByteBudget : Nat)
    (requests : List UnifiedRuntimeResourceRequest) : Prop :=
  (requests.map (fun request => request.workBytes)).sum ≤ workByteBudget

theorem concurrent_work_bytes_over_budget_are_not_admitted
    (workByteBudget : Nat)
    (requests : List UnifiedRuntimeResourceRequest)
    (over : workByteBudget < (requests.map (fun request => request.workBytes)).sum) :
    ¬ aggregateWorkBytesAdmitted workByteBudget requests := by
  simp [aggregateWorkBytesAdmitted, Nat.not_le.mpr over]

/-- Completed response history has one Search and one Query slot. In-flight
claims are task-lifetime state and are not completed-history retention. -/
structure GenerationTerminalRetention where
  searchTerminals : Nat
  queryTerminals : Nat
  deriving DecidableEq, Repr

def terminalRetentionAdmitted (retention : GenerationTerminalRetention) : Prop :=
  retention.searchTerminals ≤ 1 ∧ retention.queryTerminals ≤ 1

def retainedTerminalCount (retention : GenerationTerminalRetention) : Nat :=
  retention.searchTerminals + retention.queryTerminals

theorem admitted_generation_retains_at_most_two_terminals
    (retention : GenerationTerminalRetention)
    (admitted : terminalRetentionAdmitted retention) :
    retainedTerminalCount retention ≤ 2 := by
  rcases admitted with ⟨searchBound, queryBound⟩
  unfold retainedTerminalCount
  omega

def waiterTerminalAvailable (channelTerminal _cacheHit : Bool) : Bool :=
  channelTerminal

theorem evicted_cache_slot_does_not_erase_waiter_terminal :
    waiterTerminalAvailable true false = true := rfl

/-- Default Search projects exact grounded selectors. The complete owner
projection remains available to Query but is not expanded into Search output. -/
def defaultSearchSelectorWork (exactGroundedSelectors : Nat) : Nat :=
  exactGroundedSelectors

def fullOwnerSelectorWork (ownerSelectors : Nat) : Nat := ownerSelectors

theorem exact_grounding_never_exceeds_full_owner_projection
    (exactGroundedSelectors ownerSelectors : Nat)
    (groundedSubset : exactGroundedSelectors ≤ ownerSelectors) :
    defaultSearchSelectorWork exactGroundedSelectors ≤
      fullOwnerSelectorWork ownerSelectors := by
  exact groundedSubset

/-- A Tantivy clause waits for lexical readiness only. Request Graph state is
not part of that completion predicate. -/
structure SearchAttachmentState where
  lexicalTerminal : Bool
  requestGraphTerminal : Bool
  deriving DecidableEq, Repr

def tantivySearchAdmitted (state : SearchAttachmentState) : Bool :=
  state.lexicalTerminal

theorem lexical_ready_does_not_require_graph_terminal :
    tantivySearchAdmitted ⟨true, false⟩ = true := rfl

/-- An exact Query uses generation-admitted owner and selector indexes.  Its
logical lookup work does not contain a workspace-owner term.  Hash-table
complexity and elapsed-time qualification remain Rust obligations. -/
def indexedQueryWork
    (ownerIndexLookup selectorIndexLookup projectionWork : Nat) : Nat :=
  ownerIndexLookup + selectorIndexLookup + projectionWork

theorem indexed_query_work_is_independent_of_workspace
    (ownerIndexLookup selectorIndexLookup projectionWork
      _firstWorkspaceOwners _secondWorkspaceOwners : Nat) :
    indexedQueryWork ownerIndexLookup selectorIndexLookup projectionWork =
      indexedQueryWork ownerIndexLookup selectorIndexLookup projectionWork := by
  rfl

/-- A generation-resident exact selector/projection is already the Query read
authority.  Owner/parser materialization is a miss path, not an unconditional
prefix of every first semantic Query. -/
def exactQueryOwnerMaterializerCalls (projectionResident : Bool) : Nat :=
  if projectionResident then 0 else 1

theorem resident_exact_query_skips_owner_materializer :
    exactQueryOwnerMaterializerCalls true = 0 := rfl

/-- The resident preflight owns the projection values it proves.  Passing those
values to receipt materialization makes the complete hit path one selector read
per requested selector instead of a probe followed by a duplicate read. -/
def residentQueryProjectionReads
    (selectorCount : Nat) (handoffPreflight : Bool) : Nat :=
  if handoffPreflight then selectorCount else selectorCount + selectorCount

theorem resident_query_projection_handoff_reads_each_selector_once
    (selectorCount : Nat) :
    residentQueryProjectionReads selectorCount true = selectorCount := by
  simp [residentQueryProjectionReads]

/-- A generation-local materialized-response cache hit and a selector-index
read are distinct Runtime boundaries.  Observing the former cannot certify
that the latter executed in the same request. -/
inductive ResidentQueryObservationBoundary where
  | materializedResponse
  | selectorProjection
  deriving DecidableEq, Repr

def certifiesSelectorRead : ResidentQueryObservationBoundary → Bool
  | .materializedResponse => false
  | .selectorProjection => true

theorem materialized_response_hit_does_not_certify_selector_read :
    certifiesSelectorRead .materializedResponse = false := rfl

/-- Provider process bootstrap is a lifecycle observation, not a term in the
Runtime Search/Query data-plane measurement. Candidate parse/projection work is
still retained explicitly. -/
structure RuntimeDataPlaneWork where
  searchWork : Nat
  queryWork : Nat
  candidateProjectionWork : Nat
  deriving DecidableEq, Repr

structure ProviderLifecycleWork where
  bootstrapWork : Nat
  readinessWork : Nat
  deriving DecidableEq, Repr

def runtimeSearchQueryWork (work : RuntimeDataPlaneWork) : Nat :=
  work.searchWork + work.queryWork + work.candidateProjectionWork

theorem provider_bootstrap_is_outside_runtime_data_plane
    (runtimeWork : RuntimeDataPlaneWork)
    (_firstLifecycle _secondLifecycle : ProviderLifecycleWork) :
    runtimeSearchQueryWork runtimeWork = runtimeSearchQueryWork runtimeWork := by
  rfl

/-- The generic client owns protocol and transport, never a production link to
one language provider implementation. -/
inductive RuntimeComponent where
  | genericClient
  | providerTransport
  | languageProvider
  deriving DecidableEq, Repr

def admittedProductionDependency : RuntimeComponent → RuntimeComponent → Bool
  | .genericClient, .providerTransport => true
  | .genericClient, .languageProvider => false
  | _, _ => true

theorem generic_client_language_provider_dependency_is_forbidden :
    admittedProductionDependency .genericClient .languageProvider = false := rfl

/-- Candidate parser materialization consumes generation-admitted bytes.  Live
filesystem acquisition is build-plane work and is absent from the resident
Runtime data plane for both source and auxiliary parser inputs. -/
structure ResidentProjectionInputWork where
  residentSourceReads : Nat
  residentAuxiliaryReads : Nat
  sourceFilesystemReads : Nat
  auxiliaryFilesystemReads : Nat
  deriving DecidableEq, Repr

def residentProjectionInput (sourceReads auxiliaryReads : Nat) :
    ResidentProjectionInputWork :=
  ⟨sourceReads, auxiliaryReads, 0, 0⟩

theorem resident_candidate_projection_has_zero_filesystem_reads
    (sourceReads auxiliaryReads : Nat) :
    let work := residentProjectionInput sourceReads auxiliaryReads
    work.sourceFilesystemReads + work.auxiliaryFilesystemReads = 0 := by
  rfl

/-- Auxiliary inputs remain generation-bound without becoming searchable
source owners. -/
structure ResidentContentClasses where
  searchableOwners : Nat
  auxiliaryOwners : Nat
  deriving DecidableEq, Repr

def searchOwnerCount (content : ResidentContentClasses) : Nat :=
  content.searchableOwners

theorem auxiliary_inputs_do_not_expand_search_owner_count
    (searchable firstAuxiliary secondAuxiliary : Nat) :
    searchOwnerCount ⟨searchable, firstAuxiliary⟩ =
      searchOwnerCount ⟨searchable, secondAuxiliary⟩ := by
  rfl

/-- Restarting a compiled matcher for every physical line multiplies engine
entry work by the number of lines in one candidate owner. -/
def perLineMatcherEntrances (physicalLines : Nat) : Nat := physicalLines

/-- A generation-resident whole-owner automaton enters the matcher once. Match
offset projection is separate linear byte work and is not hidden here. -/
def perOwnerMatcherEntrances (_physicalLines : Nat) : Nat := 1

theorem whole_owner_automaton_has_one_matcher_entrance (physicalLines : Nat) :
    perOwnerMatcherEntrances physicalLines = 1 := by
  rfl

theorem whole_owner_automaton_removes_per_line_restarts
    (physicalLines : Nat)
    (nonempty : 0 < physicalLines) :
    perLineMatcherEntrances physicalLines =
      perOwnerMatcherEntrances physicalLines + (physicalLines - 1) := by
  simp [perLineMatcherEntrances, perOwnerMatcherEntrances]
  omega

end ASPProof.ResidentGrepCost
