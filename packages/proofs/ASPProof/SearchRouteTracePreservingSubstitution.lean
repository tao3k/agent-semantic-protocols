-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.SearchRouteTracePreservingSubstitution

inductive EvidenceAtom where
  | answer
  | provenance
  | sourceDigest
  deriving DecidableEq

structure CertifiedRoute where
  graphGeneration : Nat
  completionDigest : Nat
  initialPotential : Nat
  finalPotential : Nat
  evidenceTrace : List EvidenceAtom
  searchCacheBefore : Nat
  searchCacheAfter : Nat
  modelPrefixBefore : Nat
  modelPrefixAfter : Nat
  tokenCost : Nat
  roundTrips : Nat

def SameCompletionBoundary (candidate original : CertifiedRoute) : Prop :=
  candidate.graphGeneration = original.graphGeneration ∧
  candidate.completionDigest = original.completionDigest ∧
  candidate.initialPotential = original.initialPotential ∧
  candidate.finalPotential = original.finalPotential

def CacheTransitionAligned (candidate original : CertifiedRoute) : Prop :=
  candidate.searchCacheBefore = original.searchCacheBefore ∧
  candidate.searchCacheAfter = original.searchCacheAfter ∧
  candidate.modelPrefixBefore = original.modelPrefixBefore ∧
  candidate.modelPrefixAfter = original.modelPrefixAfter

def TraceRefines (candidate original : CertifiedRoute) : Prop :=
  ∀ atom, atom ∈ original.evidenceTrace → atom ∈ candidate.evidenceTrace

def ParetoNoWorse (candidate original : CertifiedRoute) : Prop :=
  candidate.tokenCost ≤ original.tokenCost ∧
  candidate.roundTrips ≤ original.roundTrips

def SafeSubstitution (candidate original : CertifiedRoute) : Prop :=
  SameCompletionBoundary candidate original ∧
  CacheTransitionAligned candidate original ∧
  TraceRefines candidate original ∧
  ParetoNoWorse candidate original

def Realizes (required : List EvidenceAtom) (route : CertifiedRoute) : Prop :=
  ∀ atom, atom ∈ required → atom ∈ route.evidenceTrace

theorem safe_substitution_preserves_completion
    {candidate original : CertifiedRoute}
    (safe : SafeSubstitution candidate original) :
    SameCompletionBoundary candidate original :=
  safe.1

theorem safe_substitution_preserves_cache_transitions
    {candidate original : CertifiedRoute}
    (safe : SafeSubstitution candidate original) :
    CacheTransitionAligned candidate original :=
  safe.2.1

theorem safe_substitution_is_pareto_no_worse
    {candidate original : CertifiedRoute}
    (safe : SafeSubstitution candidate original) :
    ParetoNoWorse candidate original :=
  safe.2.2.2

theorem trace_refinement_preserves_requirements
    {candidate original : CertifiedRoute}
    {required : List EvidenceAtom}
    (refines : TraceRefines candidate original)
    (realizes : Realizes required original) :
    Realizes required candidate := by
  intro atom requiredAtom
  exact refines atom (realizes atom requiredAtom)

theorem safe_substitution_preserves_requirements
    {candidate original : CertifiedRoute}
    {required : List EvidenceAtom}
    (safe : SafeSubstitution candidate original)
    (realizes : Realizes required original) :
    Realizes required candidate :=
  trace_refinement_preserves_requirements safe.2.2.1 realizes

def auditedRoute : CertifiedRoute where
  graphGeneration := 12
  completionDigest := 41
  initialPotential := 3
  finalPotential := 0
  evidenceTrace := [.answer, .provenance, .sourceDigest]
  searchCacheBefore := 7
  searchCacheAfter := 8
  modelPrefixBefore := 9
  modelPrefixAfter := 10
  tokenCost := 8
  roundTrips := 2

def shortRoute : CertifiedRoute where
  graphGeneration := 12
  completionDigest := 41
  initialPotential := 3
  finalPotential := 0
  evidenceTrace := [.answer]
  searchCacheBefore := 7
  searchCacheAfter := 8
  modelPrefixBefore := 9
  modelPrefixAfter := 10
  tokenCost := 4
  roundTrips := 1

def cacheDivergentRoute : CertifiedRoute where
  graphGeneration := 12
  completionDigest := 41
  initialPotential := 3
  finalPotential := 0
  evidenceTrace := [.answer, .provenance, .sourceDigest]
  searchCacheBefore := 7
  searchCacheAfter := 99
  modelPrefixBefore := 9
  modelPrefixAfter := 98
  tokenCost := 4
  roundTrips := 1

theorem endpoint_pareto_does_not_preserve_trace :
    SameCompletionBoundary shortRoute auditedRoute ∧
    ParetoNoWorse shortRoute auditedRoute ∧
    ¬ TraceRefines shortRoute auditedRoute := by
  constructor
  · exact ⟨rfl, rfl, rfl, rfl⟩
  constructor
  · exact ⟨by decide, by decide⟩
  · intro refines
    have provenancePresent :=
      refines EvidenceAtom.provenance (by simp [auditedRoute])
    simp [shortRoute] at provenancePresent

theorem trace_and_endpoint_do_not_preserve_cache_transition :
    SameCompletionBoundary cacheDivergentRoute auditedRoute ∧
    TraceRefines cacheDivergentRoute auditedRoute ∧
    ParetoNoWorse cacheDivergentRoute auditedRoute ∧
    ¬ CacheTransitionAligned cacheDivergentRoute auditedRoute := by
  constructor
  · exact ⟨rfl, rfl, rfl, rfl⟩
  constructor
  · intro atom present
    simpa [cacheDivergentRoute, auditedRoute] using present
  constructor
  · exact ⟨by decide, by decide⟩
  · simp [CacheTransitionAligned, cacheDivergentRoute, auditedRoute]

theorem missing_provenance_breaks_requirement :
    Realizes [.provenance] auditedRoute ∧
    ¬ Realizes [.provenance] shortRoute := by
  constructor
  · intro atom required
    simp only [List.mem_singleton] at required
    subst atom
    simp [auditedRoute]
  · intro realizes
    have provenancePresent :=
      realizes EvidenceAtom.provenance (by simp)
    simp [shortRoute] at provenancePresent

theorem safe_substitution_rejects_short_route :
    ¬ SafeSubstitution shortRoute auditedRoute := by
  intro safe
  exact endpoint_pareto_does_not_preserve_trace.2.2 safe.2.2.1

end ASPProof.SearchRouteTracePreservingSubstitution
