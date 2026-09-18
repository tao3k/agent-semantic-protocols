-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchEvidenceDerivation

namespace ASPProof.SearchResultOwnerSupportIndex

structure CandidateAdmission where
  acquisitionSupported : Bool
  graphNamed : Bool

def admitted (candidate : CandidateAdmission) : Bool :=
  candidate.acquisitionSupported

def legacySupportMembershipChecks
    (candidateCount ownerOccurrenceCount : Nat) : Nat :=
  candidateCount * ownerOccurrenceCount

def indexedSupportWork
    (candidateCount ownerOccurrenceCount : Nat) : Nat :=
  ownerOccurrenceCount + candidateCount

theorem graph_cannot_admit_unsupported_owner
    (graphNamed : Bool) :
    admitted {
      acquisitionSupported := false
      graphNamed := graphNamed
    } = false := by
  rfl

theorem graph_preserves_acquisition_supported_owner
    (graphNamed : Bool) :
    admitted {
      acquisitionSupported := true
      graphNamed := graphNamed
    } = true := by
  rfl

theorem indexed_support_work_does_not_exceed_nested_scan
    (candidateCount ownerOccurrenceCount : Nat)
    (candidateNontrivial : 2 ≤ candidateCount)
    (ownerOccurrencesNontrivial : 2 ≤ ownerOccurrenceCount) :
    indexedSupportWork candidateCount ownerOccurrenceCount ≤
      legacySupportMembershipChecks candidateCount ownerOccurrenceCount := by
  unfold indexedSupportWork legacySupportMembershipChecks
  by_cases candidateLeOccurrences : candidateCount ≤ ownerOccurrenceCount
  · have doubledLeProduct :=
      Nat.mul_le_mul_right ownerOccurrenceCount candidateNontrivial
    calc
      ownerOccurrenceCount + candidateCount
          ≤ ownerOccurrenceCount + ownerOccurrenceCount :=
        Nat.add_le_add_left candidateLeOccurrences ownerOccurrenceCount
      _ = 2 * ownerOccurrenceCount := (Nat.two_mul ownerOccurrenceCount).symm
      _ ≤ candidateCount * ownerOccurrenceCount := doubledLeProduct
  · have occurrencesLeCandidate : ownerOccurrenceCount ≤ candidateCount :=
      Nat.le_of_lt (Nat.lt_of_not_ge candidateLeOccurrences)
    have doubledLeProduct :=
      Nat.mul_le_mul_left candidateCount ownerOccurrencesNontrivial
    calc
      ownerOccurrenceCount + candidateCount
          ≤ candidateCount + candidateCount :=
        Nat.add_le_add_right occurrencesLeCandidate candidateCount
      _ = candidateCount * 2 := (Nat.mul_two candidateCount).symm
      _ ≤ candidateCount * ownerOccurrenceCount := doubledLeProduct

end ASPProof.SearchResultOwnerSupportIndex
