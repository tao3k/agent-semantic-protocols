-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteDominanceTransitiveShortcut

namespace ASPProof.SearchRouteShortcutCommitmentReplay

open SearchRouteFiniteMultiobjectiveParetoMaskWitness
open SearchRouteDominanceTransitiveShortcut
open SearchRouteDerivedCapacityRank

def natEq : Nat → Nat → Bool
  | 0, 0 => true
  | 0, Nat.succ _ => false
  | Nat.succ _, 0 => false
  | Nat.succ left, Nat.succ right => natEq left right

theorem natEq_true_implies_eq :
    ∀ left right,
      natEq left right = true →
        left = right
  | 0, 0, _ => rfl
  | 0, Nat.succ _, impossible => by
      cases impossible
  | Nat.succ _, 0, impossible => by
      cases impossible
  | Nat.succ left, Nat.succ right, matched => by
      change natEq left right = true at matched
      exact congrArg Nat.succ
        (natEq_true_implies_eq left right matched)

theorem natEq_is_reflexive :
    ∀ value,
      natEq value value = true
  | 0 => rfl
  | Nat.succ value =>
      natEq_is_reflexive value

structure ShortcutReplayContext where
  candidateUniverseDigest : Nat
  costSemanticsDigest : Nat
  selectionPolicyDigest : Nat
  explicitChainAuditDigest : Nat
  theoremFamilyDigest : Nat
  auditReceiptDigest : Nat

structure ShortcutCommitment where
  candidateUniverseDigest : Nat
  startCanonicalRouteId : Nat
  endpointCanonicalRouteId : Nat
  costSemanticsDigest : Nat
  selectionPolicyDigest : Nat
  chainLength : Nat
  explicitChainAuditDigest : Nat
  theoremFamilyDigest : Nat
  auditReceiptDigest : Nat

def expectedShortcutCommitment
    (context : ShortcutReplayContext)
    (start endpoint : Candidate)
    (chainLength : Nat) : ShortcutCommitment := {
  candidateUniverseDigest := context.candidateUniverseDigest
  startCanonicalRouteId := start.canonicalRouteId
  endpointCanonicalRouteId := endpoint.canonicalRouteId
  costSemanticsDigest := context.costSemanticsDigest
  selectionPolicyDigest := context.selectionPolicyDigest
  chainLength := chainLength
  explicitChainAuditDigest := context.explicitChainAuditDigest
  theoremFamilyDigest := context.theoremFamilyDigest
  auditReceiptDigest := context.auditReceiptDigest
}

def shortcutCommitmentMatches
    (expected actual : ShortcutCommitment) : Bool :=
  natEq
      expected.candidateUniverseDigest
      actual.candidateUniverseDigest &&
    natEq
      expected.startCanonicalRouteId
      actual.startCanonicalRouteId &&
    natEq
      expected.endpointCanonicalRouteId
      actual.endpointCanonicalRouteId &&
    natEq
      expected.costSemanticsDigest
      actual.costSemanticsDigest &&
    natEq
      expected.selectionPolicyDigest
      actual.selectionPolicyDigest &&
    natEq expected.chainLength actual.chainLength &&
    natEq
      expected.explicitChainAuditDigest
      actual.explicitChainAuditDigest &&
    natEq
      expected.theoremFamilyDigest
      actual.theoremFamilyDigest &&
    natEq
      expected.auditReceiptDigest
      actual.auditReceiptDigest

theorem commitment_match_true_implies_equal
    (expected actual : ShortcutCommitment)
    (matched : shortcutCommitmentMatches expected actual = true) :
    expected = actual := by
  cases expected with
  | mk expectedUniverse expectedStart expectedEndpoint
      expectedCost expectedSelection expectedLength
      expectedChainAudit expectedTheorem expectedAudit =>
    cases actual with
    | mk actualUniverse actualStart actualEndpoint
        actualCost actualSelection actualLength
        actualChainAudit actualTheorem actualAudit =>
      unfold shortcutCommitmentMatches at matched
      have parts9 := bool_and_true_parts _ _ matched
      have parts8 := bool_and_true_parts _ _ parts9.1
      have parts7 := bool_and_true_parts _ _ parts8.1
      have parts6 := bool_and_true_parts _ _ parts7.1
      have parts5 := bool_and_true_parts _ _ parts6.1
      have parts4 := bool_and_true_parts _ _ parts5.1
      have parts3 := bool_and_true_parts _ _ parts4.1
      have parts2 := bool_and_true_parts _ _ parts3.1
      have universeEq := natEq_true_implies_eq _ _ parts2.1
      have startEq := natEq_true_implies_eq _ _ parts2.2
      have endpointEq := natEq_true_implies_eq _ _ parts3.2
      have costEq := natEq_true_implies_eq _ _ parts4.2
      have selectionEq := natEq_true_implies_eq _ _ parts5.2
      have lengthEq := natEq_true_implies_eq _ _ parts6.2
      have chainAuditEq := natEq_true_implies_eq _ _ parts7.2
      have theoremEq := natEq_true_implies_eq _ _ parts8.2
      have auditEq := natEq_true_implies_eq _ _ parts9.2
      cases universeEq
      cases startEq
      cases endpointEq
      cases costEq
      cases selectionEq
      cases lengthEq
      cases chainAuditEq
      cases theoremEq
      cases auditEq
      rfl

theorem equal_commitments_match
    (commitment : ShortcutCommitment) :
    shortcutCommitmentMatches commitment commitment = true := by
  cases commitment
  unfold shortcutCommitmentMatches
  repeat' apply bool_and_true_of_parts
  all_goals exact natEq_is_reflexive _

theorem commitment_mismatch_fails_closed
    (expected actual : ShortcutCommitment)
    (different : expected ≠ actual) :
    shortcutCommitmentMatches expected actual = false := by
  cases matched : shortcutCommitmentMatches expected actual with
  | false =>
      rfl
  | true =>
      exact False.elim
        (different
          (commitment_match_true_implies_equal
            expected actual matched))

structure CachedShortcut
    (start endpoint : Candidate) where
  commitment : ShortcutCommitment
  endpointDominatesStart :
    StrictDominates endpoint start = true

def cacheTransitiveShortcut
    (context : ShortcutReplayContext)
    (start endpoint : Candidate)
    (shortcut : TransitiveShortcut start endpoint) :
    CachedShortcut start endpoint := {
  commitment := expectedShortcutCommitment
    context start endpoint (Nat.succ shortcut.chainLength)
  endpointDominatesStart := shortcut.endpointDominatesStart
}

def replayAccepts
    (expected : ShortcutCommitment)
    {start endpoint : Candidate}
    (cached : CachedShortcut start endpoint) : Bool :=
  shortcutCommitmentMatches expected cached.commitment

theorem freshly_cached_shortcut_is_accepted
    (context : ShortcutReplayContext)
    (start endpoint : Candidate)
    (shortcut : TransitiveShortcut start endpoint) :
    replayAccepts
      (expectedShortcutCommitment
        context start endpoint (Nat.succ shortcut.chainLength))
      (cacheTransitiveShortcut context start endpoint shortcut) = true := by
  apply equal_commitments_match

theorem accepted_replay_has_exact_commitment
    (expected : ShortcutCommitment)
    (start endpoint : Candidate)
    (cached : CachedShortcut start endpoint)
    (accepted : replayAccepts expected cached = true) :
    expected = cached.commitment :=
  commitment_match_true_implies_equal
    expected cached.commitment accepted

theorem mismatched_replay_is_rejected
    (expected : ShortcutCommitment)
    (start endpoint : Candidate)
    (cached : CachedShortcut start endpoint)
    (different : expected ≠ cached.commitment) :
    replayAccepts expected cached = false :=
  commitment_mismatch_fails_closed
    expected cached.commitment different

theorem accepted_replay_preserves_removal_soundness
    (expected : ShortcutCommitment)
    (start endpoint : Candidate)
    (cached : CachedShortcut start endpoint)
    (_accepted : replayAccepts expected cached = true) :
    StrictDominates endpoint start = true :=
  cached.endpointDominatesStart

theorem universe_digest_mismatch_is_rejected
    (expected : ShortcutCommitment)
    (start endpoint : Candidate)
    (cached : CachedShortcut start endpoint)
    (different :
      expected.candidateUniverseDigest ≠
        cached.commitment.candidateUniverseDigest) :
    replayAccepts expected cached = false := by
  apply mismatched_replay_is_rejected
  intro equalCommitment
  exact different
    (congrArg
      ShortcutCommitment.candidateUniverseDigest
      equalCommitment)

theorem cost_semantics_mismatch_is_rejected
    (expected : ShortcutCommitment)
    (start endpoint : Candidate)
    (cached : CachedShortcut start endpoint)
    (different :
      expected.costSemanticsDigest ≠
        cached.commitment.costSemanticsDigest) :
    replayAccepts expected cached = false := by
  apply mismatched_replay_is_rejected
  intro equalCommitment
  exact different
    (congrArg
      ShortcutCommitment.costSemanticsDigest
      equalCommitment)

theorem selection_policy_mismatch_is_rejected
    (expected : ShortcutCommitment)
    (start endpoint : Candidate)
    (cached : CachedShortcut start endpoint)
    (different :
      expected.selectionPolicyDigest ≠
        cached.commitment.selectionPolicyDigest) :
    replayAccepts expected cached = false := by
  apply mismatched_replay_is_rejected
  intro equalCommitment
  exact different
    (congrArg
      ShortcutCommitment.selectionPolicyDigest
      equalCommitment)

theorem audit_receipt_mismatch_is_rejected
    (expected : ShortcutCommitment)
    (start endpoint : Candidate)
    (cached : CachedShortcut start endpoint)
    (different :
      expected.auditReceiptDigest ≠
        cached.commitment.auditReceiptDigest) :
    replayAccepts expected cached = false := by
  apply mismatched_replay_is_rejected
  intro equalCommitment
  exact different
    (congrArg
      ShortcutCommitment.auditReceiptDigest
      equalCommitment)

def explicitReplayTokens (chainLength : Nat) : Nat :=
  9 + chainLength

def cachedReplayTokens : Nat :=
  9

theorem cached_replay_receipt_is_constant_shape :
    cachedReplayTokens = 9 := by
  rfl

theorem cached_replay_beats_nonempty_explicit_replay
    (chainLength : Nat) :
    cachedReplayTokens <
      explicitReplayTokens (Nat.succ chainLength) := by
  unfold cachedReplayTokens
  unfold explicitReplayTokens
  exact Nat.add_lt_add_left
    (Nat.zero_lt_succ chainLength) 9

end ASPProof.SearchRouteShortcutCommitmentReplay
