import ASPProof.SearchRouteRiskFeasibleGraphSelection
import ASPProof.SearchRouteGraphRouterParetoCostSelection

namespace ASPProof.SearchRouteExecutableRiskAdmittedParetoFrontier

/-!
RFC 01.27 keeps the executable finite-catalog algorithm outside the proof
kernel.  This module states the certificate that such an implementation must
produce.  Admission and completion equivalence are inputs owned by the earlier
RFCs; a frontier implementation may not redefine either predicate.
-/

/-- Abstract semantics shared by a finite executable catalog and its proof
certificate.  `costNoWorse` is deliberately separate from admission. -/
structure FrontierSemantics (Candidate : Type) where
  inCatalog : Candidate → Prop
  admitted : Candidate → Prop
  completionEquivalent : Candidate → Candidate → Prop
  costNoWorse : Candidate → Candidate → Prop

/-- Strict Pareto dominance is meaningful only inside the admitted domain and
one completion-equivalence class.  Mutual cost equality is not strict. -/
def StrictlyDominates {Candidate : Type}
    (semantics : FrontierSemantics Candidate)
    (better worse : Candidate) : Prop :=
  semantics.admitted better ∧
  semantics.admitted worse ∧
  semantics.completionEquivalent better worse ∧
  semantics.costNoWorse better worse ∧
  ¬semantics.costNoWorse worse better

/-- A proof-producing implementation returns a frontier predicate together
with membership, antichain, and admitted-coverage evidence. -/
structure CertifiedFrontier {Candidate : Type}
    (semantics : FrontierSemantics Candidate)
    (frontier : Candidate → Prop) : Prop where
  memberInCatalog : ∀ {candidate}, frontier candidate → semantics.inCatalog candidate
  memberAdmitted : ∀ {candidate}, frontier candidate → semantics.admitted candidate
  antichain : ∀ {left right},
    frontier left →
    frontier right →
    left ≠ right →
    ¬StrictlyDominates semantics left right
  coversAdmitted : ∀ {candidate},
    semantics.inCatalog candidate →
    semantics.admitted candidate →
    frontier candidate ∨
      ∃ retained, frontier retained ∧ StrictlyDominates semantics retained candidate

theorem certified_member_is_admitted {Candidate : Type}
    {semantics : FrontierSemantics Candidate}
    {frontier : Candidate → Prop}
    (certificate : CertifiedFrontier semantics frontier)
    {candidate : Candidate}
    (member : frontier candidate) :
    semantics.admitted candidate :=
  certificate.memberAdmitted member

theorem certified_member_is_catalogued {Candidate : Type}
    {semantics : FrontierSemantics Candidate}
    {frontier : Candidate → Prop}
    (certificate : CertifiedFrontier semantics frontier)
    {candidate : Candidate}
    (member : frontier candidate) :
    semantics.inCatalog candidate :=
  certificate.memberInCatalog member

theorem omitted_admitted_candidate_has_retained_dominator {Candidate : Type}
    {semantics : FrontierSemantics Candidate}
    {frontier : Candidate → Prop}
    (certificate : CertifiedFrontier semantics frontier)
    {candidate : Candidate}
    (catalogued : semantics.inCatalog candidate)
    (admitted : semantics.admitted candidate)
    (omitted : ¬frontier candidate) :
    ∃ retained, frontier retained ∧ StrictlyDominates semantics retained candidate := by
  rcases certificate.coversAdmitted catalogued admitted with retained | dominated
  · exact False.elim (omitted retained)
  · exact dominated

theorem non_equivalent_candidates_do_not_dominate {Candidate : Type}
    {semantics : FrontierSemantics Candidate}
    {left right : Candidate}
    (notEquivalent : ¬semantics.completionEquivalent left right) :
    ¬StrictlyDominates semantics left right := by
  intro dominates
  exact notEquivalent dominates.2.2.1

theorem inadmissible_candidate_does_not_dominate {Candidate : Type}
    {semantics : FrontierSemantics Candidate}
    {left right : Candidate}
    (notAdmitted : ¬semantics.admitted left) :
    ¬StrictlyDominates semantics left right := by
  intro dominates
  exact notAdmitted dominates.1

theorem reverse_no_worse_blocks_strict_dominance {Candidate : Type}
    {semantics : FrontierSemantics Candidate}
    {left right : Candidate}
    (reverseNoWorse : semantics.costNoWorse right left) :
    ¬StrictlyDominates semantics left right := by
  intro dominates
  exact dominates.2.2.2.2 reverseNoWorse

theorem equal_cost_candidates_do_not_strictly_dominate {Candidate : Type}
    {semantics : FrontierSemantics Candidate}
    {left right : Candidate}
    (_leftNoWorse : semantics.costNoWorse left right)
    (rightNoWorse : semantics.costNoWorse right left) :
    ¬StrictlyDominates semantics left right :=
  reverse_no_worse_blocks_strict_dominance rightNoWorse

theorem certified_frontier_is_empty_when_admission_is_empty {Candidate : Type}
    {semantics : FrontierSemantics Candidate}
    {frontier : Candidate → Prop}
    (certificate : CertifiedFrontier semantics frontier)
    (noAdmission : ∀ candidate, ¬semantics.admitted candidate) :
    ∀ candidate, ¬frontier candidate := by
  intro candidate member
  exact noAdmission candidate (certificate.memberAdmitted member)

theorem retained_candidates_do_not_strictly_dominate_each_other {Candidate : Type}
    {semantics : FrontierSemantics Candidate}
    {frontier : Candidate → Prop}
    (certificate : CertifiedFrontier semantics frontier)
    {left right : Candidate}
    (leftMember : frontier left)
    (rightMember : frontier right)
    (distinct : left ≠ right) :
    ¬StrictlyDominates semantics left right :=
  certificate.antichain leftMember rightMember distinct

end ASPProof.SearchRouteExecutableRiskAdmittedParetoFrontier
