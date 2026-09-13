-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

universe u

namespace ASPProof.SearchRouteCertifiedFrontierCoverage

structure CatalogIdentity where
  generation : Nat
  graphDigest : Nat
  completionDigest : Nat
  costPolicyDigest : Nat
  cacheContextDigest : Nat
  initialPotential : Nat
  finalPotential : Nat
  deriving DecidableEq, Repr

structure EnumerationCertificate
    {Route : Type u}
    (admissible enumerated : Route → Prop) where
  identity : CatalogIdentity
  coversEveryAdmissible :
    ∀ route, admissible route → enumerated route

structure FrontierCertificate
    {Route : Type u}
    (dominates : Route → Route → Prop)
    (admissible enumerated frontier : Route → Prop) where
  identity : CatalogIdentity
  frontierEnumerated :
    ∀ route, frontier route → enumerated route
  frontierAdmissible :
    ∀ route, frontier route → admissible route
  frontierUndominated :
    ∀ route,
      frontier route →
      ∀ alternative,
        enumerated alternative →
        admissible alternative →
        ¬ dominates alternative route
  coversEnumerated :
    ∀ route,
      enumerated route →
      admissible route →
      frontier route
        ∨ ∃ better, frontier better ∧ dominates better route

def CertificatesAligned
    {Route : Type u}
    {admissible enumerated frontier : Route → Prop}
    {dominates : Route → Route → Prop}
    (current : CatalogIdentity)
    (enumeration : EnumerationCertificate admissible enumerated)
    (pareto :
      FrontierCertificate dominates admissible enumerated frontier) : Prop :=
  enumeration.identity = current
    ∧ pareto.identity = current

def GloballyUndominated
    {Route : Type u}
    (dominates : Route → Route → Prop)
    (admissible : Route → Prop)
    (route : Route) : Prop :=
  admissible route
    ∧ ∀ alternative,
        admissible alternative →
        ¬ dominates alternative route

theorem global_frontier_coverage
    {Route : Type u}
    {dominates : Route → Route → Prop}
    {admissible enumerated frontier : Route → Prop}
    (current : CatalogIdentity)
    (enumeration : EnumerationCertificate admissible enumerated)
    (pareto :
      FrontierCertificate dominates admissible enumerated frontier)
    (_aligned : CertificatesAligned current enumeration pareto)
    (route : Route)
    (routeAdmissible : admissible route) :
    frontier route
      ∨ ∃ better, frontier better ∧ dominates better route := by
  exact pareto.coversEnumerated
    route
    (enumeration.coversEveryAdmissible route routeAdmissible)
    routeAdmissible

theorem globally_undominated_route_is_retained
    {Route : Type u}
    {dominates : Route → Route → Prop}
    {admissible enumerated frontier : Route → Prop}
    (current : CatalogIdentity)
    (enumeration : EnumerationCertificate admissible enumerated)
    (pareto :
      FrontierCertificate dominates admissible enumerated frontier)
    (aligned : CertificatesAligned current enumeration pareto)
    (route : Route)
    (undominated : GloballyUndominated dominates admissible route) :
    frontier route := by
  rcases global_frontier_coverage
      current enumeration pareto aligned route undominated.1 with
    retained | ⟨better, betterFrontier, betterDominates⟩
  · exact retained
  · exact False.elim
      (undominated.2
        better
        (pareto.frontierAdmissible better betterFrontier)
        betterDominates)

theorem frontier_route_is_globally_undominated
    {Route : Type u}
    {dominates : Route → Route → Prop}
    {admissible enumerated frontier : Route → Prop}
    (enumeration : EnumerationCertificate admissible enumerated)
    (pareto :
      FrontierCertificate dominates admissible enumerated frontier)
    (route : Route)
    (routeFrontier : frontier route) :
    GloballyUndominated dominates admissible route := by
  constructor
  · exact pareto.frontierAdmissible route routeFrontier
  · intro alternative alternativeAdmissible
    exact pareto.frontierUndominated
      route
      routeFrontier
      alternative
      (enumeration.coversEveryAdmissible
        alternative alternativeAdmissible)
      alternativeAdmissible

theorem certified_frontier_is_exactly_global_undominated
    {Route : Type u}
    {dominates : Route → Route → Prop}
    {admissible enumerated frontier : Route → Prop}
    (current : CatalogIdentity)
    (enumeration : EnumerationCertificate admissible enumerated)
    (pareto :
      FrontierCertificate dominates admissible enumerated frontier)
    (aligned : CertificatesAligned current enumeration pareto)
    (route : Route) :
    frontier route
      ↔ GloballyUndominated dominates admissible route := by
  constructor
  · exact frontier_route_is_globally_undominated
      enumeration pareto route
  · exact globally_undominated_route_is_retained
      current enumeration pareto aligned route

theorem aligned_certificates_use_current_generation
    {Route : Type u}
    {dominates : Route → Route → Prop}
    {admissible enumerated frontier : Route → Prop}
    (current : CatalogIdentity)
    (enumeration : EnumerationCertificate admissible enumerated)
    (pareto :
      FrontierCertificate dominates admissible enumerated frontier)
    (aligned : CertificatesAligned current enumeration pareto) :
    enumeration.identity.generation = current.generation
      ∧ pareto.identity.generation = current.generation := by
  exact
    ⟨ congrArg CatalogIdentity.generation aligned.1
    , congrArg CatalogIdentity.generation aligned.2
    ⟩

def currentIdentity : CatalogIdentity :=
  ⟨8, 41, 42, 9, 17, 3, 0⟩

def staleIdentity : CatalogIdentity :=
  { currentIdentity with generation := 7 }

theorem stale_catalog_identity_is_rejected :
    ¬ (staleIdentity = currentIdentity) := by
  simp [staleIdentity, currentIdentity]

inductive ExampleRoute
  | missed
  | listed
  deriving DecidableEq, Repr

def exampleAdmissible : ExampleRoute → Prop :=
  fun _ => True

def exampleEnumerated : ExampleRoute → Prop :=
  fun route => route = .listed

def exampleFrontier : ExampleRoute → Prop :=
  fun route => route = .listed

def exampleDominates : ExampleRoute → ExampleRoute → Prop :=
  fun _ _ => False

def localFrontierCertificate :
    FrontierCertificate
      exampleDominates
      exampleAdmissible
      exampleEnumerated
      exampleFrontier where
  identity := currentIdentity
  frontierEnumerated := by
    intro route frontier
    exact frontier
  frontierAdmissible := by
    intro _ _
    trivial
  frontierUndominated := by
    intro _ _ _ _ _ dominates
    exact dominates
  coversEnumerated := by
    intro route enumerated _
    exact Or.inl enumerated

theorem local_frontier_can_miss_global_undominated_route :
    exampleAdmissible .missed
      ∧ ¬ exampleFrontier .missed
      ∧ GloballyUndominated
          exampleDominates exampleAdmissible .missed := by
  simp
    [ exampleAdmissible
    , exampleFrontier
    , GloballyUndominated
    , exampleDominates
    ]

theorem incomplete_catalog_has_no_global_enumeration_certificate :
    ¬ Nonempty
      (EnumerationCertificate exampleAdmissible exampleEnumerated) := by
  rintro ⟨certificate⟩
  have enumeratedMissed :=
    certificate.coversEveryAdmissible .missed trivial
  simp [exampleEnumerated] at enumeratedMissed

end ASPProof.SearchRouteCertifiedFrontierCoverage
