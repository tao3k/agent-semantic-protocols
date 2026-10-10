-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.SearchRouteAndOrEvidenceGraph

inductive EvidenceAtom where
  | providerResolved
  | selectorExecutable
  | projectionCompatible
  deriving DecidableEq, Repr

structure EvidenceState where
  holds : EvidenceAtom → Bool

def SatisfiesAtom
    (state : EvidenceState)
    (atom : EvidenceAtom) : Prop :=
  state.holds atom = true

inductive Obligation where
  | atom (evidence : EvidenceAtom)
  | all (left right : Obligation)
  | any (left right : Obligation)
  deriving Repr

def Realizes
    (state : EvidenceState) :
    Obligation → Prop
  | .atom atom => SatisfiesAtom state atom
  | .all left right => Realizes state left ∧ Realizes state right
  | .any left right => Realizes state left ∨ Realizes state right

theorem realizes_all_iff
    (state : EvidenceState)
    (left right : Obligation) :
    Realizes state (.all left right) ↔
      Realizes state left ∧ Realizes state right :=
  Iff.rfl

theorem realizes_any_iff
    (state : EvidenceState)
    (left right : Obligation) :
    Realizes state (.any left right) ↔
      Realizes state left ∨ Realizes state right :=
  Iff.rfl

inductive EvidencePlan where
  | atom (evidence : EvidenceAtom)
  | both (left right : EvidencePlan)
  | chooseLeft (plan : EvidencePlan)
  | chooseRight (plan : EvidencePlan)
  deriving Repr

inductive PlanCertifies
    (state : EvidenceState) :
    EvidencePlan → Obligation → Prop where
  | atom
      (evidence : EvidenceAtom)
      (satisfied : SatisfiesAtom state evidence) :
      PlanCertifies state (.atom evidence) (.atom evidence)
  | both
      (leftPlan rightPlan : EvidencePlan)
      (leftObligation rightObligation : Obligation)
      (leftCertificate :
        PlanCertifies state leftPlan leftObligation)
      (rightCertificate :
        PlanCertifies state rightPlan rightObligation) :
      PlanCertifies
        state
        (.both leftPlan rightPlan)
        (.all leftObligation rightObligation)
  | chooseLeft
      (plan : EvidencePlan)
      (left right : Obligation)
      (certificate : PlanCertifies state plan left) :
      PlanCertifies state (.chooseLeft plan) (.any left right)
  | chooseRight
      (plan : EvidencePlan)
      (left right : Obligation)
      (certificate : PlanCertifies state plan right) :
      PlanCertifies state (.chooseRight plan) (.any left right)

theorem plan_certifies_implies_realizes
    (state : EvidenceState)
    (plan : EvidencePlan)
    (obligation : Obligation)
    (certificate : PlanCertifies state plan obligation) :
    Realizes state obligation := by
  induction certificate with
  | atom evidence satisfied =>
      exact satisfied
  | both leftPlan rightPlan leftObligation rightObligation
      leftCertificate rightCertificate leftSound rightSound =>
      exact ⟨leftSound, rightSound⟩
  | chooseLeft plan left right certificate sound =>
      exact Or.inl sound
  | chooseRight plan left right certificate sound =>
      exact Or.inr sound

def providerOnlyState : EvidenceState :=
  {
    holds := fun
      | .providerResolved => true
      | .selectorExecutable => false
      | .projectionCompatible => false
  }

def providerObligation : Obligation :=
  .atom .providerResolved

def selectorObligation : Obligation :=
  .atom .selectorExecutable

def providerAndSelector : Obligation :=
  .all providerObligation selectorObligation

def providerOrSelector : Obligation :=
  .any providerObligation selectorObligation

theorem provider_atom_is_reachable :
    Realizes providerOnlyState providerObligation := by
  unfold Realizes providerObligation SatisfiesAtom providerOnlyState
  decide

theorem provider_only_realizes_alternative :
    Realizes providerOnlyState providerOrSelector := by
  exact Or.inl provider_atom_is_reachable

theorem provider_only_does_not_realize_conjunction :
    ¬ Realizes providerOnlyState providerAndSelector := by
  simp [Realizes, providerAndSelector, providerObligation,
    selectorObligation, SatisfiesAtom, providerOnlyState]

theorem atomic_reachability_does_not_realize_required_evidence :
    Realizes providerOnlyState providerObligation ∧
      ¬ Realizes providerOnlyState providerAndSelector := by
  exact
    ⟨
      provider_atom_is_reachable,
      provider_only_does_not_realize_conjunction
    ⟩

theorem no_plan_certifies_missing_conjunct
    (plan : EvidencePlan) :
    ¬ PlanCertifies providerOnlyState plan providerAndSelector := by
  intro certificate
  exact
    provider_only_does_not_realize_conjunction
      (plan_certifies_implies_realizes
        providerOnlyState
        plan
        providerAndSelector
        certificate)

end ASPProof.SearchRouteAndOrEvidenceGraph
