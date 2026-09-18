-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteGraphRouterParetoCostSelection

namespace ASPProof.SearchRouteParetoFrontierVectorCompleteness

open ASPProof.SearchRouteGraphRouterParetoCostSelection

def HopCovers
    {Candidate : Type}
    (cost : Candidate → RouteCost)
    (frontier globalCatalog : Candidate → Prop) : Prop :=
  ∀ candidate,
    globalCatalog candidate →
    ∃ visible,
      frontier visible ∧
      (cost visible).graphHops ≤ (cost candidate).graphHops

def VectorCovers
    {Candidate : Type}
    (cost : Candidate → RouteCost)
    (frontier globalCatalog : Candidate → Prop) : Prop :=
  ∀ candidate,
    globalCatalog candidate →
    ∃ visible,
      frontier visible ∧
      NoWorse (cost visible) (cost candidate)

def ParetoMinimal
    {Candidate : Type}
    (cost : Candidate → RouteCost)
    (catalog : Candidate → Prop)
    (chosen : Candidate) : Prop :=
  catalog chosen ∧
    ∀ alternative,
      catalog alternative →
      ¬ StrictlyDominates (cost alternative) (cost chosen)

theorem vector_coverage_implies_hop_coverage
    {Candidate : Type}
    (cost : Candidate → RouteCost)
    (frontier globalCatalog : Candidate → Prop)
    (coverage : VectorCovers cost frontier globalCatalog) :
    HopCovers cost frontier globalCatalog := by
  intro candidate inUniverse
  obtain ⟨visible, inFrontier, noWorse⟩ :=
    coverage candidate inUniverse
  exact ⟨visible, inFrontier, noWorse.1⟩

theorem no_worse_before_strict_dominance
    (first second third : RouteCost)
    (firstNoWorse : NoWorse first second)
    (secondDominates : StrictlyDominates second third) :
    StrictlyDominates first third := by
  refine
    ⟨ no_worse_is_transitive
        first second third firstNoWorse secondDominates.1
    , ?_
    ⟩
  rcases secondDominates.2 with
    graphHopsBetter
    | interactionRoundsBetter
    | projectedTokensBetter
    | verificationOpsBetter
    | searchExecutionsBetter
    | modelPrefixRecomputationsBetter
  · exact Or.inl
      (Nat.lt_of_le_of_lt firstNoWorse.1 graphHopsBetter)
  · exact Or.inr
      (Or.inl
        (Nat.lt_of_le_of_lt
          firstNoWorse.2.1 interactionRoundsBetter))
  · exact Or.inr
      (Or.inr
        (Or.inl
          (Nat.lt_of_le_of_lt
            firstNoWorse.2.2.1 projectedTokensBetter)))
  · exact Or.inr
      (Or.inr
        (Or.inr
          (Or.inl
            (Nat.lt_of_le_of_lt
              firstNoWorse.2.2.2.1 verificationOpsBetter))))
  · exact Or.inr
      (Or.inr
        (Or.inr
          (Or.inr
            (Or.inl
              (Nat.lt_of_le_of_lt
                firstNoWorse.2.2.2.2.1 searchExecutionsBetter)))))
  · exact Or.inr
      (Or.inr
        (Or.inr
          (Or.inr
            (Or.inr
              (Nat.lt_of_le_of_lt
                firstNoWorse.2.2.2.2.2
                modelPrefixRecomputationsBetter)))))

theorem vector_complete_frontier_lifts_pareto_minimality
    {Candidate : Type}
    (cost : Candidate → RouteCost)
    (frontier globalCatalog : Candidate → Prop)
    (frontierSubset :
      ∀ candidate,
        frontier candidate → globalCatalog candidate)
    (coverage : VectorCovers cost frontier globalCatalog)
    (chosen : Candidate)
    (frontierMinimal : ParetoMinimal cost frontier chosen) :
    ParetoMinimal cost globalCatalog chosen := by
  constructor
  · exact frontierSubset chosen frontierMinimal.1
  · intro globalAlternative inUniverse globalDominates
    obtain ⟨visible, inFrontier, visibleNoWorse⟩ :=
      coverage globalAlternative inUniverse
    exact
      (frontierMinimal.2 visible inFrontier)
        (no_worse_before_strict_dominance
          (cost visible)
          (cost globalAlternative)
          (cost chosen)
          visibleNoWorse
          globalDominates)

inductive ExampleRoute where
  | visible
  | omitted
  deriving DecidableEq, Repr

def exampleCost : ExampleRoute → RouteCost
  | .visible =>
      ⟨1, 5, 100, 2, 1, 1⟩
  | .omitted =>
      ⟨1, 1, 10, 1, 1, 1⟩

def exampleFrontier (candidate : ExampleRoute) : Prop :=
  candidate = .visible

def exampleUniverse (_candidate : ExampleRoute) : Prop :=
  True

theorem example_frontier_is_hop_complete :
    HopCovers exampleCost exampleFrontier exampleUniverse := by
  intro candidate _inUniverse
  refine ⟨.visible, rfl, ?_⟩
  cases candidate <;> simp [exampleCost]

theorem example_frontier_is_not_vector_complete :
    ¬ VectorCovers exampleCost exampleFrontier exampleUniverse := by
  intro coverage
  obtain ⟨frontierCandidate, inFrontier, noWorse⟩ :=
    coverage .omitted trivial
  have visibleIdentity : frontierCandidate = .visible :=
    inFrontier
  subst frontierCandidate
  exact
    (by
      simp [NoWorse, exampleCost] :
        ¬ NoWorse
          (exampleCost .visible)
          (exampleCost .omitted))
      noWorse

theorem hop_coverage_does_not_imply_vector_coverage :
    HopCovers exampleCost exampleFrontier exampleUniverse ∧
      ¬ VectorCovers exampleCost exampleFrontier exampleUniverse :=
  ⟨example_frontier_is_hop_complete,
    example_frontier_is_not_vector_complete⟩

theorem visible_route_is_frontier_pareto_minimal :
    ParetoMinimal exampleCost exampleFrontier .visible := by
  constructor
  · rfl
  · intro alternative inFrontier
    have alternativeIdentity : alternative = .visible :=
      inFrontier
    subst alternative
    exact strict_dominance_is_irreflexive (exampleCost .visible)

theorem visible_route_is_not_global_pareto_minimal :
    ¬ ParetoMinimal exampleCost exampleUniverse .visible := by
  intro globalMinimal
  exact
    (globalMinimal.2 .omitted trivial)
      (by
        simp [StrictlyDominates, NoWorse, exampleCost])

theorem hop_complete_frontier_can_hide_global_pareto_dominator :
    ParetoMinimal exampleCost exampleFrontier .visible ∧
      HopCovers exampleCost exampleFrontier exampleUniverse ∧
      ¬ ParetoMinimal exampleCost exampleUniverse .visible :=
  ⟨visible_route_is_frontier_pareto_minimal,
    example_frontier_is_hop_complete,
    visible_route_is_not_global_pareto_minimal⟩

end ASPProof.SearchRouteParetoFrontierVectorCompleteness
