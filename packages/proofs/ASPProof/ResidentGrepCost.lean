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

end ASPProof.ResidentGrepCost
