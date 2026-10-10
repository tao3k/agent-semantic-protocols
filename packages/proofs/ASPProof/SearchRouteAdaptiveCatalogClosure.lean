-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.SearchRouteAdaptiveCatalogClosure

universe u v

inductive Reachable
    {Route : Type u}
    {Evidence : Type v}
    (initial : Route → Prop)
    (emits : Route → Evidence → Prop)
    (enables : Evidence → Route → Prop) : Route → Prop where
  | initial {route} :
      initial route →
      Reachable initial emits enables route
  | step {route evidence next} :
      Reachable initial emits enables route →
      emits route evidence →
      enables evidence next →
      Reachable initial emits enables next

def CoversInitial
    {Route : Type u}
    (catalog initial : Route → Prop) : Prop :=
  ∀ ⦃route⦄, initial route → catalog route

def SuccessorClosed
    {Route : Type u}
    {Evidence : Type v}
    (catalog : Route → Prop)
    (emits : Route → Evidence → Prop)
    (enables : Evidence → Route → Prop) : Prop :=
  ∀ ⦃route evidence next⦄,
    catalog route →
    emits route evidence →
    enables evidence next →
    catalog next

def CatalogSound
    {Route : Type u}
    {Evidence : Type v}
    (catalog initial : Route → Prop)
    (emits : Route → Evidence → Prop)
    (enables : Evidence → Route → Prop) : Prop :=
  ∀ ⦃route⦄, catalog route → Reachable initial emits enables route

def FrontierCovers
    {Route : Type u}
    (catalog retained : Route → Prop)
    (dominates : Route → Route → Prop) : Prop :=
  ∀ ⦃route⦄,
    catalog route →
    retained route ∨
      ∃ dominator,
        catalog dominator ∧
        retained dominator ∧
        dominates dominator route

def ReachablyUndominated
    {Route : Type u}
    {Evidence : Type v}
    (initial : Route → Prop)
    (emits : Route → Evidence → Prop)
    (enables : Evidence → Route → Prop)
    (dominates : Route → Route → Prop)
    (route : Route) : Prop :=
  Reachable initial emits enables route ∧
  ∀ ⦃other⦄,
    Reachable initial emits enables other →
    ¬ dominates other route

theorem reachable_route_is_enumerated
    {Route : Type u}
    {Evidence : Type v}
    {catalog initial : Route → Prop}
    {emits : Route → Evidence → Prop}
    {enables : Evidence → Route → Prop}
    (coversInitial : CoversInitial catalog initial)
    (closed : SuccessorClosed catalog emits enables)
    {route : Route}
    (reachable : Reachable initial emits enables route) :
    catalog route := by
  induction reachable with
  | initial initialRoute =>
      exact coversInitial initialRoute
  | step _ emitted enabled enumerated =>
      exact closed enumerated emitted enabled

theorem adaptive_frontier_covers_reachable_route
    {Route : Type u}
    {Evidence : Type v}
    {catalog initial retained : Route → Prop}
    {emits : Route → Evidence → Prop}
    {enables : Evidence → Route → Prop}
    {dominates : Route → Route → Prop}
    (coversInitial : CoversInitial catalog initial)
    (closed : SuccessorClosed catalog emits enables)
    (frontier : FrontierCovers catalog retained dominates)
    {route : Route}
    (reachable : Reachable initial emits enables route) :
    retained route ∨
      ∃ dominator,
        catalog dominator ∧
        retained dominator ∧
        dominates dominator route :=
  frontier (reachable_route_is_enumerated coversInitial closed reachable)

theorem reachably_undominated_route_is_retained
    {Route : Type u}
    {Evidence : Type v}
    {catalog initial retained : Route → Prop}
    {emits : Route → Evidence → Prop}
    {enables : Evidence → Route → Prop}
    {dominates : Route → Route → Prop}
    (coversInitial : CoversInitial catalog initial)
    (closed : SuccessorClosed catalog emits enables)
    (sound : CatalogSound catalog initial emits enables)
    (frontier : FrontierCovers catalog retained dominates)
    {route : Route}
    (undominated :
      ReachablyUndominated initial emits enables dominates route) :
    retained route := by
  rcases adaptive_frontier_covers_reachable_route
      coversInitial closed frontier undominated.1 with
    retainedRoute | ⟨dominator, inCatalog, retainedDominator, dominatesRoute⟩
  · exact retainedRoute
  · exact False.elim (undominated.2 (sound inCatalog) dominatesRoute)

inductive ExampleRoute where
  | seed
  | discovered
  deriving DecidableEq

inductive ExampleEvidence where
  | unlock
  deriving DecidableEq

def exampleInitial : ExampleRoute → Prop
  | .seed => True
  | .discovered => False

def exampleEmits : ExampleRoute → ExampleEvidence → Prop
  | .seed, .unlock => True
  | .discovered, .unlock => False

def exampleEnables : ExampleEvidence → ExampleRoute → Prop
  | .unlock, .seed => False
  | .unlock, .discovered => True

def incompleteCatalog : ExampleRoute → Prop
  | .seed => True
  | .discovered => False

def staticFrontier : ExampleRoute → Prop
  | .seed => True
  | .discovered => False

def exampleDominates (_ _ : ExampleRoute) : Prop :=
  False

theorem discovered_route_is_reachable :
    Reachable exampleInitial exampleEmits exampleEnables .discovered :=
  Reachable.step
    (route := ExampleRoute.seed)
    (evidence := ExampleEvidence.unlock)
    (next := ExampleRoute.discovered)
    (Reachable.initial (initial := exampleInitial) (emits := exampleEmits)
      (enables := exampleEnables) (route := ExampleRoute.seed) trivial)
    trivial
    trivial

theorem static_frontier_covers_incomplete_catalog :
    FrontierCovers incompleteCatalog staticFrontier exampleDominates := by
  intro route inCatalog
  cases route with
  | seed => exact Or.inl trivial
  | discovered => exact False.elim inCatalog

theorem static_frontier_misses_reachable_route :
    Reachable exampleInitial exampleEmits exampleEnables .discovered ∧
    ¬ staticFrontier .discovered :=
  ⟨discovered_route_is_reachable, id⟩

theorem incomplete_catalog_is_not_successor_closed :
    ¬ SuccessorClosed incompleteCatalog exampleEmits exampleEnables := by
  intro closed
  have discoveredInCatalog : incompleteCatalog .discovered :=
    closed (route := .seed) (evidence := .unlock) (next := .discovered)
      trivial trivial trivial
  exact discoveredInCatalog

theorem initial_coverage_does_not_imply_adaptive_coverage :
    CoversInitial incompleteCatalog exampleInitial ∧
    ∃ route,
      Reachable exampleInitial exampleEmits exampleEnables route ∧
      ¬ incompleteCatalog route := by
  constructor
  · intro route initialRoute
    cases route with
    | seed => trivial
    | discovered => exact False.elim initialRoute
  · exact
      ⟨.discovered, discovered_route_is_reachable, id⟩

end ASPProof.SearchRouteAdaptiveCatalogClosure
