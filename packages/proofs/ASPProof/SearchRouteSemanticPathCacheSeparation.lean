-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteOrthogonalCacheCredits

namespace ASPProof.SearchRouteSemanticPathCacheSeparation

open ASPProof.SearchRouteOrthogonalCacheCredits

/--
The semantic evidence path and the work used to materialize it are different
objects. A cache may reduce execution work, but it cannot shorten the evidence
chain certified by the route.
-/
structure RouteMeasure where
  semanticGraphHops : Nat
  executedGraphHops : Nat
  toolRounds : Nat
  searchTokens : Nat
  uncachedModelTokens : Nat
  deriving DecidableEq, Repr

structure SearchExecutionCredit where
  executedGraphHops : Nat
  toolRounds : Nat
  searchTokens : Nat
  deriving DecidableEq, Repr

structure ModelExecutionCredit where
  uncachedModelTokens : Nat
  deriving DecidableEq, Repr

def CreditsWithinMeasure
    (planned : RouteMeasure)
    (search : SearchExecutionCredit)
    (model : ModelExecutionCredit) : Prop :=
  search.executedGraphHops ≤ planned.executedGraphHops ∧
    search.toolRounds ≤ planned.toolRounds ∧
    search.searchTokens ≤ planned.searchTokens ∧
    model.uncachedModelTokens ≤ planned.uncachedModelTokens

def applyExecutionCredits
    (planned : RouteMeasure)
    (search : SearchExecutionCredit)
    (model : ModelExecutionCredit) : RouteMeasure :=
  { semanticGraphHops := planned.semanticGraphHops
  , executedGraphHops :=
      planned.executedGraphHops - search.executedGraphHops
  , toolRounds := planned.toolRounds - search.toolRounds
  , searchTokens := planned.searchTokens - search.searchTokens
  , uncachedModelTokens :=
      planned.uncachedModelTokens - model.uncachedModelTokens
  }

theorem cache_preserves_semantic_graph_hops
    (planned : RouteMeasure)
    (search : SearchExecutionCredit)
    (model : ModelExecutionCredit) :
    (applyExecutionCredits planned search model).semanticGraphHops =
      planned.semanticGraphHops :=
  rfl

theorem execution_credit_conservation
    (planned : RouteMeasure)
    (search : SearchExecutionCredit)
    (model : ModelExecutionCredit)
    (valid : CreditsWithinMeasure planned search model) :
    let realized := applyExecutionCredits planned search model
    realized.executedGraphHops + search.executedGraphHops =
        planned.executedGraphHops ∧
      realized.toolRounds + search.toolRounds = planned.toolRounds ∧
      realized.searchTokens + search.searchTokens = planned.searchTokens ∧
      realized.uncachedModelTokens + model.uncachedModelTokens =
        planned.uncachedModelTokens := by
  simp [CreditsWithinMeasure, applyExecutionCredits] at valid ⊢
  omega

def zeroSearchExecutionCredit : SearchExecutionCredit :=
  ⟨0, 0, 0⟩

def zeroModelExecutionCredit : ModelExecutionCredit :=
  ⟨0⟩

theorem search_cache_is_model_cost_orthogonal
    (planned : RouteMeasure)
    (search : SearchExecutionCredit)
    (model : ModelExecutionCredit) :
    (applyExecutionCredits planned search model).uncachedModelTokens =
      (applyExecutionCredits
        planned zeroSearchExecutionCredit model).uncachedModelTokens :=
  rfl

theorem model_cache_is_search_execution_orthogonal
    (planned : RouteMeasure)
    (search : SearchExecutionCredit)
    (model : ModelExecutionCredit) :
    let withModel := applyExecutionCredits planned search model
    let withoutModel :=
      applyExecutionCredits planned search zeroModelExecutionCredit
    withModel.semanticGraphHops = withoutModel.semanticGraphHops ∧
      withModel.executedGraphHops = withoutModel.executedGraphHops ∧
      withModel.toolRounds = withoutModel.toolRounds ∧
      withModel.searchTokens = withoutModel.searchTokens := by
  simp [applyExecutionCredits, zeroModelExecutionCredit]

/--
Countermodel for the ambiguous legacy name: the old cache-credit projection
can reduce its `graphHops` field. That field therefore denotes executed graph
work and cannot also certify semantic evidence-path length.
-/
def legacyPlanned : SearchRouteOrthogonalCacheCredits.RouteCost :=
  ⟨4, 2, 100, 1000⟩

def legacySearchCredit : SearchRouteOrthogonalCacheCredits.SearchCacheCredit :=
  ⟨3, 1, 80⟩

theorem legacy_cache_projection_changes_declared_graph_hops :
    legacyPlanned.graphHops = 4 ∧
      (SearchRouteOrthogonalCacheCredits.applyCacheCredits
        legacyPlanned
        legacySearchCredit
        SearchRouteOrthogonalCacheCredits.zeroModelCredit).graphHops = 1 := by
  decide

def shortSemanticRoute : RouteMeasure :=
  ⟨2, 2, 1, 20, 100⟩

def longSemanticRoute : RouteMeasure :=
  ⟨5, 5, 1, 20, 100⟩

def shortSearchCredit : SearchExecutionCredit :=
  ⟨2, 0, 0⟩

def longSearchCredit : SearchExecutionCredit :=
  ⟨5, 0, 0⟩

/--
Equal realized traversal work does not identify the semantic evidence path.
The router must rank semantic path length from the certificate, not reconstruct
it from cache-adjusted execution cost.
-/
theorem equal_realized_traversal_does_not_identify_semantic_path :
    (applyExecutionCredits
        shortSemanticRoute shortSearchCredit zeroModelExecutionCredit
      ).executedGraphHops =
      (applyExecutionCredits
        longSemanticRoute longSearchCredit zeroModelExecutionCredit
      ).executedGraphHops ∧
      shortSemanticRoute.semanticGraphHops ≠
        longSemanticRoute.semanticGraphHops := by
  decide

end ASPProof.SearchRouteSemanticPathCacheSeparation
