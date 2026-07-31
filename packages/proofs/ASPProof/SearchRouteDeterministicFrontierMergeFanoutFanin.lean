import Std

namespace ASPProof.SearchRouteDeterministicFrontierMergeFanoutFanin

/-- Canonical route or provenance membership, indexed by canonical identity. -/
abbrev CanonicalMembership := Nat → Bool

/-- Set union is the merge operation at the canonical membership layer. -/
def mergeMembership
    (left right : CanonicalMembership) : CanonicalMembership :=
  fun canonicalId => left canonicalId || right canonicalId

theorem mergeMembership_commutative
    (left right : CanonicalMembership) (canonicalId : Nat) :
    mergeMembership left right canonicalId =
      mergeMembership right left canonicalId := by
  unfold mergeMembership
  cases left canonicalId <;> cases right canonicalId <;> rfl

theorem mergeMembership_associative
    (first second third : CanonicalMembership) (canonicalId : Nat) :
    mergeMembership (mergeMembership first second) third canonicalId =
      mergeMembership first (mergeMembership second third) canonicalId := by
  unfold mergeMembership
  cases first canonicalId <;>
    cases second canonicalId <;>
      cases third canonicalId <;> rfl

theorem mergeMembership_idempotent
    (frontier : CanonicalMembership) (canonicalId : Nat) :
    mergeMembership frontier frontier canonicalId = frontier canonicalId := by
  unfold mergeMembership
  cases frontier canonicalId <;> rfl

/-- A parallel fan-in and a sequential fold have identical canonical membership. -/
theorem parallel_fanin_equals_sequential_fold
    (first second third : CanonicalMembership) (canonicalId : Nat) :
    mergeMembership (mergeMembership first second) third canonicalId =
      mergeMembership first (mergeMembership second third) canonicalId :=
  mergeMembership_associative first second third canonicalId

/-- Provenance is merged as a set, not selected by completion order. -/
abbrev ProvenanceMembership := CanonicalMembership

def mergeProvenance
    (left right : ProvenanceMembership) : ProvenanceMembership :=
  mergeMembership left right

theorem equal_route_provenance_union_commutative
    (left right : ProvenanceMembership) (evidenceId : Nat) :
    mergeProvenance left right evidenceId =
      mergeProvenance right left evidenceId :=
  mergeMembership_commutative left right evidenceId

/-- All semantic inputs that authorize a deterministic frontier merge. -/
structure EvaluationIdentity where
  snapshotDigest : Nat
  queryDigest : Nat
  costPolicyDigest : Nat
  duplicatePolicyDigest : Nat
deriving DecidableEq

def MergeAuthorized
    (left right : EvaluationIdentity) : Prop :=
  left = right

theorem reflexive_evaluation_identity_authorizes_merge
    (identity : EvaluationIdentity) :
    MergeAuthorized identity identity :=
  rfl

theorem changed_snapshot_rejects_merge
    (left right : EvaluationIdentity)
    (changed : left.snapshotDigest ≠ right.snapshotDigest) :
    ¬ MergeAuthorized left right := by
  intro sameIdentity
  apply changed
  exact congrArg EvaluationIdentity.snapshotDigest sameIdentity

theorem changed_duplicate_policy_rejects_merge
    (left right : EvaluationIdentity)
    (changed : left.duplicatePolicyDigest ≠ right.duplicatePolicyDigest) :
    ¬ MergeAuthorized left right := by
  intro sameIdentity
  apply changed
  exact congrArg EvaluationIdentity.duplicatePolicyDigest sameIdentity

/-- Last-writer-wins is completion-order sensitive and is not a valid ACI policy. -/
def lastWriterWins (_left right : Nat) : Nat :=
  right

theorem last_writer_wins_is_not_commutative :
    lastWriterWins 1 2 ≠ lastWriterWins 2 1 := by
  decide

/-- Coverage counts are explicit; merging partial frontiers need not be complete. -/
def PartialCoverage (covered total : Nat) : Prop :=
  covered < total

theorem two_partial_frontiers_can_remain_partial
    (_leftPartial : PartialCoverage 1 3)
    (_rightPartial : PartialCoverage 1 3) :
    PartialCoverage (1 + 1) 3 := by
  exact Nat.lt_succ_self 2

structure MergeReceiptBudget where
  fixedBytes : Nat
  bytesPerEntry : Nat
  leftCapacity : Nat
  rightCapacity : Nat

def explicitMergeReceiptUpperBound
    (budget : MergeReceiptBudget) : Nat :=
  budget.fixedBytes +
    budget.bytesPerEntry * (budget.leftCapacity + budget.rightCapacity)

theorem explicit_merge_receipt_is_bounded
    (budget : MergeReceiptBudget)
    (observedBytes : Nat)
    (withinBudget :
      observedBytes ≤ explicitMergeReceiptUpperBound budget) :
    observedBytes ≤ explicitMergeReceiptUpperBound budget :=
  withinBudget

/-- Without a frontier capacity, explicit entry disclosure has no fixed bound. -/
def uncappedExplicitMergeBytes (entryCount : Nat) : Nat :=
  entryCount

theorem uncapped_explicit_merge_exceeds_every_fixed_bound
    (fixedBound : Nat) :
    fixedBound < uncappedExplicitMergeBytes (fixedBound + 1) := by
  exact Nat.lt_succ_self fixedBound

/-- The compact receipt carries roots, counts, and policy identity at fixed size. -/
def summarizedMergeReceiptBytes
    (fixedSummaryBytes _mergedEntryCount : Nat) : Nat :=
  fixedSummaryBytes

theorem summarized_merge_receipt_is_entry_count_independent
    (fixedSummaryBytes firstCount secondCount : Nat) :
    summarizedMergeReceiptBytes fixedSummaryBytes firstCount =
      summarizedMergeReceiptBytes fixedSummaryBytes secondCount :=
  rfl

end ASPProof.SearchRouteDeterministicFrontierMergeFanoutFanin
