-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.GenerationQualifiedSelectorStaleness

inductive OwnerHistory where
  | neverExisted
  | deleted
  | wrongWorkspace
  | olderGeneration
  deriving DecidableEq

structure UnqualifiedObservation where
  ownerPresent : Bool
  deriving DecidableEq

def observe : OwnerHistory → UnqualifiedObservation
  | .neverExisted | .deleted | .wrongWorkspace | .olderGeneration => ⟨false⟩

theorem unqualified_absence_cannot_identify_staleness :
    observe .neverExisted = observe .olderGeneration := by
  rfl

structure QualifiedObservation where
  requestedGeneration : Nat
  activeGeneration : Nat

def stale (observation : QualifiedObservation) : Prop :=
  observation.requestedGeneration ≠ observation.activeGeneration

theorem qualified_mismatch_proves_staleness
    (observation : QualifiedObservation)
    (mismatch : observation.requestedGeneration ≠ observation.activeGeneration) :
    stale observation := by
  exact mismatch

end ASPProof.GenerationQualifiedSelectorStaleness
