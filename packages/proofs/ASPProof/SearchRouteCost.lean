-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchLoopTrace

namespace SearchRouteCost

abbrev DomainId := Nat
abbrev ClosureDigest := Nat

inductive StepKind where
  | orient
  | discover
  | inspect
  | routerJump
  | semanticCacheHit
  | graph
  | materialize
  | close
  deriving DecidableEq, Repr

structure RouteStep where
  kind : StepKind
  evidenceTokens : Nat
  promptTokens : Nat
  cachedPromptTokens : Nat
  roundCost : Nat
  deriving DecidableEq, Repr

structure SearchRoute where
  steps : List RouteStep
  domain : DomainId
  closureDigest : ClosureDigest
  closed : Bool
  graphEscalated : Bool
  semanticCacheValidated : Bool
  deriving DecidableEq, Repr

structure RouteRequirement where
  activeDomain : DomainId
  expectedClosure : ClosureDigest
  graphMandatory : Bool
  deriving DecidableEq, Repr

structure RouteBudget where
  maxEvidenceTokens : Nat
  maxRawTokens : Nat
  maxUncachedTokens : Nat
  maxRounds : Nat
  deriving DecidableEq, Repr

def transitionCount (route : SearchRoute) : Nat :=
  route.steps.length

abbrev hopCount := transitionCount

def evidenceTokenCost : SearchRoute → Nat
  | route =>
      route.steps.foldl (fun total step => total + step.evidenceTokens) 0

def promptTokenCost : SearchRoute → Nat
  | route =>
      route.steps.foldl (fun total step => total + step.promptTokens) 0

def cachedPromptTokenCost : SearchRoute → Nat
  | route =>
      route.steps.foldl (fun total step => total + step.cachedPromptTokens) 0

def rawTokenCost (route : SearchRoute) : Nat :=
  evidenceTokenCost route + promptTokenCost route

def uncachedTokenCost : SearchRoute → Nat
  | route =>
      route.steps.foldl
        (fun total step =>
          total + step.evidenceTokens +
            (step.promptTokens - step.cachedPromptTokens))
        0

def roundCost : SearchRoute → Nat
  | route => route.steps.foldl (fun total step => total + step.roundCost) 0

def containsKind (route : SearchRoute) (kind : StepKind) : Bool :=
  route.steps.any fun step => step.kind == kind

def hasRoutingStep (route : SearchRoute) : Bool :=
  containsKind route StepKind.orient ||
    containsKind route StepKind.routerJump

def prefixCacheSoundB (route : SearchRoute) : Bool :=
  route.steps.all fun step => step.cachedPromptTokens ≤ step.promptTokens

def semanticCacheSoundB (route : SearchRoute) : Bool :=
  !containsKind route StepKind.semanticCacheHit ||
    route.semanticCacheValidated

def admissibleB
    (requirement : RouteRequirement)
    (route : SearchRoute) :
    Bool :=
  route.domain == requirement.activeDomain &&
    route.closureDigest == requirement.expectedClosure &&
    route.closed &&
    hasRoutingStep route &&
    containsKind route StepKind.close &&
    prefixCacheSoundB route &&
    semanticCacheSoundB route &&
    (!requirement.graphMandatory ||
      (route.graphEscalated && containsKind route StepKind.graph))

def withinBudgetB (budget : RouteBudget) (route : SearchRoute) : Bool :=
  evidenceTokenCost route ≤ budget.maxEvidenceTokens &&
    rawTokenCost route ≤ budget.maxRawTokens &&
    uncachedTokenCost route ≤ budget.maxUncachedTokens &&
    roundCost route ≤ budget.maxRounds

def feasibleB
    (requirement : RouteRequirement)
    (budget : RouteBudget)
    (route : SearchRoute) :
    Bool :=
  admissibleB requirement route && withinBudgetB budget route

def noWorse (improved original : SearchRoute) : Prop :=
  hopCount improved ≤ hopCount original ∧
    evidenceTokenCost improved ≤ evidenceTokenCost original ∧
    rawTokenCost improved ≤ rawTokenCost original ∧
    uncachedTokenCost improved ≤ uncachedTokenCost original ∧
    roundCost improved ≤ roundCost original

def strictlyDominates (improved original : SearchRoute) : Prop :=
  noWorse improved original ∧
    (hopCount improved < hopCount original ∨
      evidenceTokenCost improved < evidenceTokenCost original ∨
      rawTokenCost improved < rawTokenCost original ∨
      uncachedTokenCost improved < uncachedTokenCost original ∨
      roundCost improved < roundCost original)

def preferRoute (left right : SearchRoute) : SearchRoute :=
  if hopCount left < hopCount right then
    left
  else if hopCount right < hopCount left then
    right
  else if uncachedTokenCost left < uncachedTokenCost right then
    left
  else if uncachedTokenCost right < uncachedTokenCost left then
    right
  else if roundCost left ≤ roundCost right then
    left
  else
    right

def lexNoWorse (left right : SearchRoute) : Prop :=
  hopCount left < hopCount right ∨
    (hopCount left = hopCount right ∧
      (uncachedTokenCost left < uncachedTokenCost right ∨
        (uncachedTokenCost left = uncachedTokenCost right ∧
          roundCost left ≤ roundCost right)))

theorem lexNoWorse_refl (route : SearchRoute) :
    lexNoWorse route route := by
  simp [lexNoWorse]

theorem lexNoWorse_trans
    {left middle right : SearchRoute}
    (leftMiddle : lexNoWorse left middle)
    (middleRight : lexNoWorse middle right) :
    lexNoWorse left right := by
  unfold lexNoWorse at *
  omega

theorem preferRoute_lexNoWorse_left (left right : SearchRoute) :
    lexNoWorse (preferRoute left right) left := by
  by_cases hopLeft : hopCount left < hopCount right
  · simp [preferRoute, hopLeft, lexNoWorse]
  · by_cases hopRight : hopCount right < hopCount left
    · simp [preferRoute, hopLeft, hopRight, lexNoWorse]
      <;> omega
    · by_cases tokenLeft :
        uncachedTokenCost left < uncachedTokenCost right
      · simp [preferRoute, hopLeft, hopRight, tokenLeft, lexNoWorse]
        <;> omega
      · by_cases tokenRight :
          uncachedTokenCost right < uncachedTokenCost left
        · simp [preferRoute, hopLeft, hopRight, tokenLeft, tokenRight,
            lexNoWorse]
          omega
        · by_cases roundLeft : roundCost left ≤ roundCost right
          · simp [preferRoute, hopLeft, hopRight, tokenLeft, tokenRight,
              roundLeft, lexNoWorse]
            <;> omega
          · simp [preferRoute, hopLeft, hopRight, tokenLeft, tokenRight,
              roundLeft, lexNoWorse]
            omega

theorem preferRoute_lexNoWorse_right (left right : SearchRoute) :
    lexNoWorse (preferRoute left right) right := by
  by_cases hopLeft : hopCount left < hopCount right
  · simp [preferRoute, hopLeft, lexNoWorse]
  · by_cases hopRight : hopCount right < hopCount left
    · simp [preferRoute, hopLeft, hopRight, lexNoWorse]
    · by_cases tokenLeft :
        uncachedTokenCost left < uncachedTokenCost right
      · simp [preferRoute, hopLeft, hopRight, tokenLeft, lexNoWorse]
        omega
      · by_cases tokenRight :
          uncachedTokenCost right < uncachedTokenCost left
        · simp [preferRoute, hopLeft, hopRight, tokenLeft, tokenRight,
            lexNoWorse]
        · by_cases roundLeft : roundCost left ≤ roundCost right
          · simp [preferRoute, hopLeft, hopRight, tokenLeft, tokenRight,
              roundLeft, lexNoWorse]
            omega
          · simp [preferRoute, hopLeft, hopRight, tokenLeft, tokenRight,
              roundLeft, lexNoWorse]

def chooseShortestFeasible
    (requirement : RouteRequirement)
    (budget : RouteBudget) :
    List SearchRoute → Option SearchRoute
  | [] => none
  | route :: rest =>
      let chosenRest := chooseShortestFeasible requirement budget rest
      if feasibleB requirement budget route then
        match chosenRest with
        | none => some route
        | some current => some (preferRoute route current)
      else
        chosenRest

theorem preferRoute_eq_left_or_right (left right : SearchRoute) :
    preferRoute left right = left ∨ preferRoute left right = right := by
  by_cases hopLeft : hopCount left < hopCount right
  · simp [preferRoute, hopLeft]
  · by_cases hopRight : hopCount right < hopCount left
    · simp [preferRoute, hopLeft, hopRight]
    · by_cases tokenLeft :
        uncachedTokenCost left < uncachedTokenCost right
      · simp [preferRoute, hopLeft, hopRight, tokenLeft]
      · by_cases tokenRight :
          uncachedTokenCost right < uncachedTokenCost left
        · simp [preferRoute, hopLeft, hopRight, tokenLeft, tokenRight]
        · by_cases roundLeft : roundCost left ≤ roundCost right
          · simp [preferRoute, hopLeft, hopRight, tokenLeft, tokenRight,
              roundLeft]
          · simp [preferRoute, hopLeft, hopRight, tokenLeft, tokenRight,
              roundLeft]

theorem chooseShortestFeasible_none_implies_infeasible
    (requirement : RouteRequirement)
    (budget : RouteBudget)
    (routes : List SearchRoute)
    (chosenNone :
      chooseShortestFeasible requirement budget routes = none) :
    ∀ route ∈ routes, feasibleB requirement budget route = false := by
  induction routes with
  | nil =>
      simp
  | cons head tail inductionHypothesis =>
      cases headFeasible : feasibleB requirement budget head with
      | false =>
          have tailNone :
              chooseShortestFeasible requirement budget tail = none := by
            simpa [chooseShortestFeasible, headFeasible] using chosenNone
          intro route routeMember
          simp only [List.mem_cons] at routeMember
          rcases routeMember with routeIsHead | routeInTail
          · simpa [routeIsHead] using headFeasible
          · exact inductionHypothesis tailNone route routeInTail
      | true =>
          cases tailChosen :
              chooseShortestFeasible requirement budget tail with
          | none =>
              simp [chooseShortestFeasible, headFeasible, tailChosen] at chosenNone
          | some current =>
              simp [chooseShortestFeasible, headFeasible, tailChosen] at chosenNone

theorem chooseShortestFeasible_mem_and_feasible
    (requirement : RouteRequirement)
    (budget : RouteBudget)
    (routes : List SearchRoute)
    (selected : SearchRoute)
    (selectedByRouter :
      chooseShortestFeasible requirement budget routes = some selected) :
    selected ∈ routes ∧ feasibleB requirement budget selected = true := by
  induction routes generalizing selected with
  | nil =>
      simp [chooseShortestFeasible] at selectedByRouter
  | cons head tail inductionHypothesis =>
      cases headFeasible : feasibleB requirement budget head with
      | false =>
          have selectedFromTail :
              chooseShortestFeasible requirement budget tail =
                some selected := by
            simpa [chooseShortestFeasible, headFeasible] using
              selectedByRouter
          have tailReceipt :=
            inductionHypothesis selected selectedFromTail
          exact ⟨by simp [tailReceipt.1], tailReceipt.2⟩
      | true =>
          cases tailChosen :
              chooseShortestFeasible requirement budget tail with
          | none =>
              have selectedIsHead : selected = head := by
                simpa [chooseShortestFeasible, headFeasible, tailChosen] using
                  selectedByRouter.symm
              subst selected
              exact ⟨by simp, headFeasible⟩
          | some current =>
              have selectedIsPreferred :
                  selected = preferRoute head current := by
                simpa [chooseShortestFeasible, headFeasible, tailChosen] using
                  selectedByRouter.symm
              have currentReceipt :=
                inductionHypothesis current tailChosen
              subst selected
              rcases preferRoute_eq_left_or_right head current with
                preferredIsHead | preferredIsCurrent
              · rw [preferredIsHead]
                exact ⟨by simp, headFeasible⟩
              · rw [preferredIsCurrent]
                exact ⟨by simp [currentReceipt.1], currentReceipt.2⟩

theorem chooseShortestFeasible_is_lex_optimal
    (requirement : RouteRequirement)
    (budget : RouteBudget)
    (routes : List SearchRoute)
    (selected : SearchRoute)
    (selectedByRouter :
      chooseShortestFeasible requirement budget routes = some selected) :
    ∀ candidate ∈ routes,
      feasibleB requirement budget candidate = true →
      lexNoWorse selected candidate := by
  induction routes generalizing selected with
  | nil =>
      simp [chooseShortestFeasible] at selectedByRouter
  | cons head tail inductionHypothesis =>
      cases headFeasible : feasibleB requirement budget head with
      | false =>
          have selectedFromTail :
              chooseShortestFeasible requirement budget tail =
                some selected := by
            simpa [chooseShortestFeasible, headFeasible] using
              selectedByRouter
          intro candidate candidateMember candidateFeasible
          simp only [List.mem_cons] at candidateMember
          rcases candidateMember with candidateIsHead | candidateInTail
          · subst candidate
            simp [headFeasible] at candidateFeasible
          · exact inductionHypothesis selected selectedFromTail candidate
              candidateInTail candidateFeasible
      | true =>
          cases tailChosen :
              chooseShortestFeasible requirement budget tail with
          | none =>
              have selectedIsHead : selected = head := by
                simpa [chooseShortestFeasible, headFeasible, tailChosen] using
                  selectedByRouter.symm
              subst selected
              intro candidate candidateMember candidateFeasible
              simp only [List.mem_cons] at candidateMember
              rcases candidateMember with candidateIsHead | candidateInTail
              · subst candidate
                exact lexNoWorse_refl head
              · have candidateInfeasible :=
                  chooseShortestFeasible_none_implies_infeasible
                    requirement budget tail tailChosen candidate
                    candidateInTail
                simp [candidateInfeasible] at candidateFeasible
          | some current =>
              have selectedIsPreferred :
                  selected = preferRoute head current := by
                simpa [chooseShortestFeasible, headFeasible, tailChosen] using
                  selectedByRouter.symm
              subst selected
              intro candidate candidateMember candidateFeasible
              simp only [List.mem_cons] at candidateMember
              rcases candidateMember with candidateIsHead | candidateInTail
              · subst candidate
                exact preferRoute_lexNoWorse_left head current
              · exact lexNoWorse_trans
                  (preferRoute_lexNoWorse_right head current)
                  (inductionHypothesis current tailChosen candidate candidateInTail
                    candidateFeasible)

def orientStep : RouteStep :=
  { kind := StepKind.orient
    evidenceTokens := 2
    promptTokens := 4
    cachedPromptTokens := 0
    roundCost := 1 }

def discoverStep : RouteStep :=
  { kind := StepKind.discover
    evidenceTokens := 8
    promptTokens := 6
    cachedPromptTokens := 0
    roundCost := 1 }

def inspectStep : RouteStep :=
  { kind := StepKind.inspect
    evidenceTokens := 10
    promptTokens := 6
    cachedPromptTokens := 0
    roundCost := 1 }

def cachedInspectStep : RouteStep :=
  { inspectStep with cachedPromptTokens := 4 }

def routerJumpStep : RouteStep :=
  { kind := StepKind.routerJump
    evidenceTokens := 3
    promptTokens := 4
    cachedPromptTokens := 0
    roundCost := 1 }

def cachedRouterJumpStep : RouteStep :=
  { routerJumpStep with cachedPromptTokens := 4 }

def semanticCacheHitStep : RouteStep :=
  { kind := StepKind.semanticCacheHit
    evidenceTokens := 2
    promptTokens := 6
    cachedPromptTokens := 4
    roundCost := 1 }

def graphStep : RouteStep :=
  { kind := StepKind.graph
    evidenceTokens := 10
    promptTokens := 6
    cachedPromptTokens := 4
    roundCost := 1 }

def fullGraphDumpStep : RouteStep :=
  { graphStep with evidenceTokens := 100 }

def materializeStep : RouteStep :=
  { kind := StepKind.materialize
    evidenceTokens := 15
    promptTokens := 6
    cachedPromptTokens := 0
    roundCost := 1 }

def compactMaterializeStep : RouteStep :=
  { materializeStep with evidenceTokens := 10 }

def cachedCompactMaterializeStep : RouteStep :=
  { compactMaterializeStep with cachedPromptTokens := 4 }

def closeStep : RouteStep :=
  { kind := StepKind.close
    evidenceTokens := 2
    promptTokens := 4
    cachedPromptTokens := 0
    roundCost := 1 }

def cachedCloseStep : RouteStep :=
  { closeStep with cachedPromptTokens := 4 }

def invalidPrefixClaimStep : RouteStep :=
  { kind := StepKind.materialize
    evidenceTokens := 1
    promptTokens := 2
    cachedPromptTokens := 3
    roundCost := 1 }

def nonGraphRequirement : RouteRequirement :=
  { activeDomain := 1
    expectedClosure := 99
    graphMandatory := false }

def graphRequirement : RouteRequirement :=
  { activeDomain := 1
    expectedClosure := 99
    graphMandatory := true }

def compactBudget : RouteBudget :=
  { maxEvidenceTokens := 20
    maxRawTokens := 40
    maxUncachedTokens := 20
    maxRounds := 3 }

def baselineLoopRoute : SearchRoute :=
  { steps :=
      [ orientStep
      , discoverStep
      , inspectStep
      , materializeStep
      , closeStep
      ]
    domain := 1
    closureDigest := 99
    closed := true
    graphEscalated := false
    semanticCacheValidated := false }

def routerJumpRoute : SearchRoute :=
  { steps := [routerJumpStep, compactMaterializeStep, closeStep]
    domain := 1
    closureDigest := 99
    closed := true
    graphEscalated := false
    semanticCacheValidated := false }

def modelCachedRouterRoute : SearchRoute :=
  { steps :=
      [cachedRouterJumpStep, cachedCompactMaterializeStep, cachedCloseStep]
    domain := 1
    closureDigest := 99
    closed := true
    graphEscalated := false
    semanticCacheValidated := false }

def semanticCacheMissRoute : SearchRoute :=
  { steps :=
      [ cachedRouterJumpStep
      , cachedInspectStep
      , cachedCompactMaterializeStep
      , cachedCloseStep
      ]
    domain := 1
    closureDigest := 99
    closed := true
    graphEscalated := false
    semanticCacheValidated := false }

def jointCacheHitRoute : SearchRoute :=
  { steps := [cachedRouterJumpStep, semanticCacheHitStep, cachedCloseStep]
    domain := 1
    closureDigest := 99
    closed := true
    graphEscalated := false
    semanticCacheValidated := true }

def unsafeGraphShortcut : SearchRoute :=
  { steps := [cachedRouterJumpStep, cachedCloseStep]
    domain := 1
    closureDigest := 99
    closed := true
    graphEscalated := false
    semanticCacheValidated := false }

def unvalidatedSemanticCacheRoute : SearchRoute :=
  { jointCacheHitRoute with semanticCacheValidated := false }

def impossiblePrefixCacheRoute : SearchRoute :=
  { steps := [cachedRouterJumpStep, invalidPrefixClaimStep, cachedCloseStep]
    domain := 1
    closureDigest := 99
    closed := true
    graphEscalated := false
    semanticCacheValidated := false }

def fullGraphDumpRoute : SearchRoute :=
  { steps := [cachedRouterJumpStep, fullGraphDumpStep, cachedCloseStep]
    domain := 1
    closureDigest := 99
    closed := true
    graphEscalated := true
    semanticCacheValidated := false }

def boundedGraphRoute : SearchRoute :=
  { steps := [cachedRouterJumpStep, graphStep, cachedCloseStep]
    domain := 1
    closureDigest := 99
    closed := true
    graphEscalated := true
    semanticCacheValidated := false }

theorem router_jump_preserves_admissibility :
    admissibleB nonGraphRequirement baselineLoopRoute = true ∧
      admissibleB nonGraphRequirement routerJumpRoute = true ∧
      admissibleB nonGraphRequirement modelCachedRouterRoute = true ∧
      admissibleB nonGraphRequirement jointCacheHitRoute = true := by
  decide

theorem router_jump_strictly_dominates_baseline :
    strictlyDominates routerJumpRoute baselineLoopRoute := by
  unfold strictlyDominates noWorse
  decide

theorem router_jump_cost_receipt :
    hopCount baselineLoopRoute = 5 ∧
      hopCount routerJumpRoute = 3 ∧
      evidenceTokenCost baselineLoopRoute = 37 ∧
      evidenceTokenCost routerJumpRoute = 15 ∧
      rawTokenCost baselineLoopRoute = 63 ∧
      rawTokenCost routerJumpRoute = 29 ∧
      uncachedTokenCost baselineLoopRoute = 63 ∧
      uncachedTokenCost routerJumpRoute = 29 ∧
      roundCost baselineLoopRoute = 5 ∧
      roundCost routerJumpRoute = 3 := by
  decide

theorem model_prefix_cache_reduces_uncached_tokens :
    hopCount modelCachedRouterRoute = hopCount routerJumpRoute ∧
      rawTokenCost modelCachedRouterRoute = rawTokenCost routerJumpRoute ∧
      roundCost modelCachedRouterRoute = roundCost routerJumpRoute ∧
      cachedPromptTokenCost modelCachedRouterRoute = 12 ∧
      uncachedTokenCost routerJumpRoute = 29 ∧
      uncachedTokenCost modelCachedRouterRoute = 17 := by
  decide

theorem semantic_search_cache_reduces_route_tokens :
    hopCount semanticCacheMissRoute = 4 ∧
      hopCount jointCacheHitRoute = 3 ∧
      roundCost semanticCacheMissRoute = 4 ∧
      roundCost jointCacheHitRoute = 3 ∧
      evidenceTokenCost semanticCacheMissRoute = 25 ∧
      evidenceTokenCost jointCacheHitRoute = 7 ∧
      rawTokenCost semanticCacheMissRoute = 45 ∧
      rawTokenCost jointCacheHitRoute = 21 ∧
      uncachedTokenCost semanticCacheMissRoute = 29 ∧
      uncachedTokenCost jointCacheHitRoute = 9 := by
  decide

theorem joint_cache_route_strictly_dominates_baseline :
    strictlyDominates jointCacheHitRoute baselineLoopRoute := by
  unfold strictlyDominates noWorse
  decide

theorem unconstrained_shortest_path_is_unsound :
    hopCount unsafeGraphShortcut < hopCount boundedGraphRoute ∧
      admissibleB graphRequirement unsafeGraphShortcut = false ∧
      admissibleB graphRequirement boundedGraphRoute = true := by
  decide

theorem full_graph_dump_violates_token_budget :
    admissibleB graphRequirement fullGraphDumpRoute = true ∧
      withinBudgetB compactBudget fullGraphDumpRoute = false ∧
      feasibleB graphRequirement compactBudget boundedGraphRoute = true := by
  decide

theorem unvalidated_search_cache_is_rejected :
    hopCount unvalidatedSemanticCacheRoute = hopCount jointCacheHitRoute ∧
      admissibleB nonGraphRequirement unvalidatedSemanticCacheRoute = false ∧
      admissibleB nonGraphRequirement jointCacheHitRoute = true := by
  decide

theorem impossible_model_cache_claim_is_rejected :
    cachedPromptTokenCost impossiblePrefixCacheRoute >
        promptTokenCost impossiblePrefixCacheRoute ∧
      prefixCacheSoundB impossiblePrefixCacheRoute = false ∧
      admissibleB nonGraphRequirement impossiblePrefixCacheRoute = false := by
  decide

theorem bounded_router_selects_graph_route :
    chooseShortestFeasible
        graphRequirement
        compactBudget
        [unsafeGraphShortcut, fullGraphDumpRoute, boundedGraphRoute] =
      some boundedGraphRoute := by
  decide

theorem router_jump_is_selected_over_loop :
    chooseShortestFeasible
        nonGraphRequirement
        { maxEvidenceTokens := 50
          maxRawTokens := 80
          maxUncachedTokens := 80
          maxRounds := 6 }
        [baselineLoopRoute, routerJumpRoute] =
      some routerJumpRoute := by
  decide

theorem joint_cache_route_is_selected :
    chooseShortestFeasible
        nonGraphRequirement
        { maxEvidenceTokens := 50
          maxRawTokens := 80
          maxUncachedTokens := 80
          maxRounds := 6 }
        [baselineLoopRoute, routerJumpRoute, modelCachedRouterRoute,
          jointCacheHitRoute] =
      some jointCacheHitRoute := by
  decide

theorem router_jump_saves_two_rounds :
    roundCost baselineLoopRoute =
      roundCost routerJumpRoute + 2 := by
  decide

end SearchRouteCost
