-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteDAG

namespace SearchRouteDAGEnumeration

open SearchRouteCost SearchRouteDAG

abbrev EndpointWithHops (graph : RankedDAG) :=
  Fin graph.nodeCount × Nat

def enumerateWalkEndpoints
    (graph : RankedDAG)
    (source : Fin graph.nodeCount) :
    Nat →
    List (EndpointWithHops graph)
  | 0 => [(source, 0)]
  | fuel + 1 =>
      (source, 0) ::
        (List.finRange graph.nodeCount).flatMap fun next =>
          if graph.edge source next then
            (enumerateWalkEndpoints graph next fuel).map fun endpoint =>
              (endpoint.1, endpoint.2 + 1)
          else
            []

theorem enumerateWalkEndpoints_complete
    {graph : RankedDAG}
    {source target : Fin graph.nodeCount}
    {hops fuel : Nat}
    (walk : Walk graph source target hops)
    (withinFuel : hops ≤ fuel) :
    (target, hops) ∈
      enumerateWalkEndpoints graph source fuel := by
  induction walk generalizing fuel with
  | stop node =>
      cases fuel <;> simp [enumerateWalkEndpoints]
  | @step source next target hops edgeReceipt rest inductionHypothesis =>
      cases fuel with
      | zero =>
          omega
      | succ remainingFuel =>
          have restWithinFuel : hops ≤ remainingFuel := by
            omega
          have restMember :=
            inductionHypothesis restWithinFuel
          simp only [enumerateWalkEndpoints, List.mem_cons]
          right
          apply List.mem_flatMap.mpr
          refine ⟨next, ?_, ?_⟩
          · simp
          · simp [edgeReceipt]
            exact restMember

theorem enumerateWalkEndpoints_sound
    {graph : RankedDAG}
    {source target : Fin graph.nodeCount}
    {hops fuel : Nat}
    (member :
      (target, hops) ∈
        enumerateWalkEndpoints graph source fuel) :
    Walk graph source target hops ∧ hops ≤ fuel := by
  induction fuel generalizing source target hops with
  | zero =>
      simp [enumerateWalkEndpoints] at member
      rcases member with ⟨rfl, rfl⟩
      exact ⟨Walk.stop target, by omega⟩
  | succ remainingFuel inductionHypothesis =>
      simp only [enumerateWalkEndpoints, List.mem_cons] at member
      rcases member with isStop | isExtended
      · cases Prod.mk.inj isStop with
        | intro targetIsSource hopsIsZero =>
            subst target
            subst hops
            exact ⟨Walk.stop source, by omega⟩
      · rcases List.mem_flatMap.mp isExtended with
          ⟨next, nextInRange, extendedMember⟩
        by_cases edgeReceipt : graph.edge source next = true
        · simp [edgeReceipt] at extendedMember
          rcases extendedMember with
            ⟨restTarget, restHops, restMember,
              targetIsRestTarget, hopsIsSuccessor⟩
          subst target
          subst hops
          have restReceipt :=
            inductionHypothesis restMember
          exact
            ⟨ Walk.step edgeReceipt restReceipt.1
            , by omega
            ⟩
        · simp [edgeReceipt] at extendedMember

theorem rankedDagFuel_is_complete
    {graph : RankedDAG}
    {source target : Fin graph.nodeCount}
    {hops : Nat}
    (walk : Walk graph source target hops) :
    (target, hops) ∈
      enumerateWalkEndpoints
        graph
        source
        (graph.maxRank - graph.rank source) := by
  apply enumerateWalkEndpoints_complete walk
  have bounded := walk_hops_are_bounded walk
  omega

def enumerateGraphCandidates
    (graph : RankedDAG)
    (source target : Fin graph.nodeCount)
    (fuel : Nat)
    (routeCatalog : List SearchRoute) :
    List GraphCandidate :=
  (enumerateWalkEndpoints graph source fuel).flatMap fun endpoint =>
    if endpoint.1 = target then
      routeCatalog.map fun route =>
        { route := route
          graphHops := endpoint.2
          pathValidated := true }
    else
      []

def RouteCatalogComplete
    (requirement : RouteRequirement)
    (budget : GraphRouteBudget)
    (routeCatalog : List SearchRoute) :
    Prop :=
  ∀ route,
    transitionCount route ≤ budget.maxTransitions →
    feasibleB requirement budget.routeBudget route = true →
    route ∈ routeCatalog

def enumerateStepLists
    (stepCatalog : List RouteStep) :
    Nat → List (List RouteStep)
  | 0 => [[]]
  | fuel + 1 =>
      [] ::
        stepCatalog.flatMap fun step =>
          (enumerateStepLists stepCatalog fuel).map (List.cons step)

theorem enumerateStepLists_complete
    {stepCatalog : List RouteStep}
    {steps : List RouteStep}
    {fuel : Nat}
    (withinFuel : steps.length ≤ fuel)
    (covered : ∀ step ∈ steps, step ∈ stepCatalog) :
    steps ∈ enumerateStepLists stepCatalog fuel := by
  induction fuel generalizing steps with
  | zero =>
      cases steps with
      | nil =>
          simp [enumerateStepLists]
      | cons head tail =>
          simp at withinFuel
  | succ remaining inductionHypothesis =>
      cases steps with
      | nil =>
          simp [enumerateStepLists]
      | cons head tail =>
          have headCovered : head ∈ stepCatalog :=
            covered head (by simp)
          have tailCovered :
              ∀ step ∈ tail, step ∈ stepCatalog := by
            intro step stepInTail
            exact covered step (by simp [stepInTail])
          have tailWithinFuel : tail.length ≤ remaining := by
            simp at withinFuel
            omega
          have tailGenerated :=
            inductionHypothesis tailWithinFuel tailCovered
          simp [
            enumerateStepLists,
            headCovered,
            tailGenerated
          ]

def requirementRouteVariants
    (requirement : RouteRequirement)
    (steps : List RouteStep) :
    List SearchRoute :=
  [ { steps := steps
      domain := requirement.activeDomain
      closureDigest := requirement.expectedClosure
      closed := true
      graphEscalated := false
      semanticCacheValidated := false }
  , { steps := steps
      domain := requirement.activeDomain
      closureDigest := requirement.expectedClosure
      closed := true
      graphEscalated := false
      semanticCacheValidated := true }
  , { steps := steps
      domain := requirement.activeDomain
      closureDigest := requirement.expectedClosure
      closed := true
      graphEscalated := true
      semanticCacheValidated := false }
  , { steps := steps
      domain := requirement.activeDomain
      closureDigest := requirement.expectedClosure
      closed := true
      graphEscalated := true
      semanticCacheValidated := true }
  ]

theorem feasible_route_mem_requirement_variants
    {requirement : RouteRequirement}
    {budget : RouteBudget}
    {route : SearchRoute}
    (feasible : feasibleB requirement budget route = true) :
    route ∈ requirementRouteVariants requirement route.steps := by
  have admissible :
      admissibleB requirement route = true := by
    have feasibleParts :
        admissibleB requirement route = true ∧
          withinBudgetB budget route = true := by
      simpa [feasibleB] using feasible
    exact feasibleParts.1
  rcases route with
    ⟨steps, domain, closureDigest, closed, graphEscalated,
      semanticCacheValidated⟩
  simp [admissibleB] at admissible
  rcases admissible with
    ⟨⟨⟨⟨⟨⟨⟨domainMatches, closureMatches⟩, isClosed⟩, _⟩, _⟩,
      _⟩, _⟩, _⟩
  clear feasible
  subst domain
  subst closureDigest
  subst closed
  cases graphEscalated <;>
    cases semanticCacheValidated <;>
      simp [requirementRouteVariants]

def StepCatalogComplete
    (requirement : RouteRequirement)
    (budget : RouteBudget)
    (stepCatalog : List RouteStep) :
    Prop :=
  ∀ route,
    feasibleB requirement budget route = true →
    ∀ step ∈ route.steps, step ∈ stepCatalog

theorem foldl_add_measure_eq
    {α : Type}
    (measure : α → Nat)
    (items : List α)
    (initial : Nat) :
    items.foldl (fun total item => total + measure item) initial =
      initial +
        items.foldl (fun total item => total + measure item) 0 := by
  induction items generalizing initial with
  | nil =>
      simp
  | cons head tail inductionHypothesis =>
      rw [List.foldl_cons]
      calc
        tail.foldl
              (fun total item => total + measure item)
              (initial + measure head) =
            (initial + measure head) +
              tail.foldl
                (fun total item => total + measure item)
                0 :=
          inductionHypothesis (initial + measure head)
        _ =
            initial +
              (measure head +
                tail.foldl
                  (fun total item => total + measure item)
                  0) := by
          omega
        _ =
            initial +
              tail.foldl
                (fun total item => total + measure item)
                (measure head) := by
          rw [inductionHypothesis (measure head)]
        _ =
            initial +
              (head :: tail).foldl
                (fun total item => total + measure item)
                0 := by
          simp only [List.foldl_cons, Nat.zero_add]

theorem member_measure_le_foldl
    {α : Type}
    (measure : α → Nat)
    {item : α}
    {items : List α}
    (member : item ∈ items) :
    measure item ≤
      items.foldl (fun total current => total + measure current) 0 := by
  induction items with
  | nil =>
      simp at member
  | cons head tail inductionHypothesis =>
      simp at member
      rw [List.foldl_cons, foldl_add_measure_eq]
      rcases member with itemIsHead | itemInTail
      · subst item
        omega
      · have tailBound := inductionHypothesis itemInTail
        omega

def allStepKinds : List StepKind :=
  [ StepKind.orient
  , StepKind.discover
  , StepKind.inspect
  , StepKind.routerJump
  , StepKind.semanticCacheHit
  , StepKind.graph
  , StepKind.materialize
  , StepKind.close
  ]

theorem stepKind_mem_allStepKinds (kind : StepKind) :
    kind ∈ allStepKinds := by
  cases kind <;> simp [allStepKinds]

def enumerateRouteSteps (budget : RouteBudget) :
    List RouteStep :=
  allStepKinds.flatMap fun kind =>
    (List.range (budget.maxEvidenceTokens + 1)).flatMap fun evidence =>
      (List.range (budget.maxRawTokens + 1)).flatMap fun prompt =>
        (List.range (budget.maxRawTokens + 1)).flatMap fun cached =>
          (List.range (budget.maxRounds + 1)).map fun rounds =>
            { kind := kind
              evidenceTokens := evidence
              promptTokens := prompt
              cachedPromptTokens := cached
              roundCost := rounds }

theorem feasible_step_mem_enumerateRouteSteps
    {requirement : RouteRequirement}
    {budget : RouteBudget}
    {route : SearchRoute}
    {step : RouteStep}
    (feasible : feasibleB requirement budget route = true)
    (stepMember : step ∈ route.steps) :
    step ∈ enumerateRouteSteps budget := by
  have feasibleParts :
      admissibleB requirement route = true ∧
        withinBudgetB budget route = true := by
    simpa [feasibleB] using feasible
  have budgetParts := feasibleParts.2
  simp [withinBudgetB] at budgetParts
  rcases budgetParts with
    ⟨⟨⟨evidenceBudget, rawBudget⟩, _⟩, roundsBudget⟩
  have evidenceBound :
      step.evidenceTokens ≤ budget.maxEvidenceTokens := by
    calc
      step.evidenceTokens ≤ evidenceTokenCost route := by
        exact
          member_measure_le_foldl
            (fun current : RouteStep => current.evidenceTokens)
            stepMember
      _ ≤ budget.maxEvidenceTokens := evidenceBudget
  have promptPart :
      step.promptTokens ≤ promptTokenCost route := by
    exact
      member_measure_le_foldl
        (fun current : RouteStep => current.promptTokens)
        stepMember
  have promptBound :
      step.promptTokens ≤ budget.maxRawTokens := by
    have promptWithinRaw :
        promptTokenCost route ≤ rawTokenCost route := by
      simp [rawTokenCost]
    omega
  have roundsBound :
      step.roundCost ≤ budget.maxRounds := by
    calc
      step.roundCost ≤ roundCost route := by
        exact
          member_measure_le_foldl
            (fun current : RouteStep => current.roundCost)
            stepMember
      _ ≤ budget.maxRounds := roundsBudget
  have admissibleParts := feasibleParts.1
  simp [admissibleB] at admissibleParts
  rcases admissibleParts with
    ⟨⟨⟨⟨⟨⟨⟨_, _⟩, _⟩, _⟩, _⟩, prefixSound⟩, _⟩, _⟩
  simp [prefixCacheSoundB] at prefixSound
  have cachedWithinPrompt :
      step.cachedPromptTokens ≤ step.promptTokens :=
    prefixSound step stepMember
  have cachedBound :
      step.cachedPromptTokens ≤ budget.maxRawTokens := by
    omega
  rcases step with
    ⟨kind, evidence, prompt, cached, rounds⟩
  have evidenceBound' :
      evidence ≤ budget.maxEvidenceTokens := by
    simpa using evidenceBound
  have promptBound' :
      prompt ≤ budget.maxRawTokens := by
    simpa using promptBound
  have cachedBound' :
      cached ≤ budget.maxRawTokens := by
    simpa using cachedBound
  have roundsBound' :
      rounds ≤ budget.maxRounds := by
    simpa using roundsBound
  simp only [
    enumerateRouteSteps,
    List.mem_flatMap,
    List.mem_map,
    List.mem_range
  ]
  refine
    ⟨ kind
    , stepKind_mem_allStepKinds kind
    , evidence
    , by omega
    , prompt
    , by omega
    , cached
    , by omega
    , rounds
    , by omega
    , ?_
    ⟩
  rfl

theorem budget_step_catalog_is_complete
    (requirement : RouteRequirement)
    (budget : RouteBudget) :
    StepCatalogComplete
      requirement
      budget
      (enumerateRouteSteps budget) := by
  intro route feasible step stepMember
  exact
    feasible_step_mem_enumerateRouteSteps
      feasible
      stepMember

def enumerateFeasibleRoutes
    (requirement : RouteRequirement)
    (budget : GraphRouteBudget)
    (stepCatalog : List RouteStep) :
    List SearchRoute :=
  ((enumerateStepLists
      stepCatalog
      budget.maxTransitions).flatMap fun steps =>
        requirementRouteVariants requirement steps).filter fun route =>
          feasibleB requirement budget.routeBudget route

theorem bounded_feasible_route_catalog_is_complete
    {requirement : RouteRequirement}
    {budget : GraphRouteBudget}
    {stepCatalog : List RouteStep}
    (stepCatalogComplete :
      StepCatalogComplete
        requirement
        budget.routeBudget
        stepCatalog) :
    RouteCatalogComplete
      requirement
      budget
      (enumerateFeasibleRoutes requirement budget stepCatalog) := by
  intro route transitionBound feasible
  apply List.mem_filter.mpr
  refine ⟨?_, feasible⟩
  apply List.mem_flatMap.mpr
  refine ⟨route.steps, ?_, ?_⟩
  · apply enumerateStepLists_complete
    · simpa [transitionCount] using transitionBound
    · exact stepCatalogComplete route feasible
  · exact feasible_route_mem_requirement_variants feasible

theorem generated_candidate_mem_of_components
    {graph : RankedDAG}
    {source target : Fin graph.nodeCount}
    {fuel hops : Nat}
    {route : SearchRoute}
    {routeCatalog : List SearchRoute}
    (walkMember :
      (target, hops) ∈
        enumerateWalkEndpoints graph source fuel)
    (routeMember : route ∈ routeCatalog) :
    { route := route
      graphHops := hops
      pathValidated := true } ∈
      enumerateGraphCandidates
        graph source target fuel routeCatalog := by
  apply List.mem_flatMap.mpr
  refine ⟨(target, hops), walkMember, ?_⟩
  simp [routeMember]

theorem complete_walks_and_route_catalog_generate_complete_candidates
    {graph : RankedDAG}
    {source target : Fin graph.nodeCount}
    {requirement : RouteRequirement}
    {budget : GraphRouteBudget}
    {routeCatalog : List SearchRoute}
    (routeCatalogComplete :
      RouteCatalogComplete requirement budget routeCatalog) :
    CandidateComplete
      graph
      source
      target
      requirement
      budget
      (enumerateGraphCandidates
        graph
        source
        target
        (graph.maxRank - graph.rank source)
        routeCatalog) := by
  rintro ⟨route, hops, pathValidated⟩ realization feasible
  cases pathValidated with
  | false =>
      simp [graphFeasibleB] at feasible
  | true =>
      have walkMember :=
        rankedDagFuel_is_complete realization
      have feasibleParts :
          (hops ≤ budget.maxGraphHops ∧
            transitionCount route ≤ budget.maxTransitions) ∧
            feasibleB requirement budget.routeBudget route = true := by
        simpa [graphFeasibleB] using feasible
      exact generated_candidate_mem_of_components
        walkMember
        (routeCatalogComplete
          route
          feasibleParts.1.2
          feasibleParts.2)

theorem finite_step_catalog_generates_complete_candidates
    {graph : RankedDAG}
    {source target : Fin graph.nodeCount}
    {requirement : RouteRequirement}
    {budget : GraphRouteBudget}
    {stepCatalog : List RouteStep}
    (stepCatalogComplete :
      StepCatalogComplete
        requirement
        budget.routeBudget
        stepCatalog) :
    CandidateComplete
      graph
      source
      target
      requirement
      budget
      (enumerateGraphCandidates
        graph
        source
        target
        (graph.maxRank - graph.rank source)
        (enumerateFeasibleRoutes
          requirement
          budget
          stepCatalog)) := by
  exact
    complete_walks_and_route_catalog_generate_complete_candidates
      (bounded_feasible_route_catalog_is_complete stepCatalogComplete)

theorem budget_catalog_generates_complete_candidates
    {graph : RankedDAG}
    {source target : Fin graph.nodeCount}
    {requirement : RouteRequirement}
    {budget : GraphRouteBudget} :
    CandidateComplete
      graph
      source
      target
      requirement
      budget
      (enumerateGraphCandidates
        graph
        source
        target
        (graph.maxRank - graph.rank source)
        (enumerateFeasibleRoutes
          requirement
          budget
          (enumerateRouteSteps budget.routeBudget))) := by
  exact
    finite_step_catalog_generates_complete_candidates
      (budget_step_catalog_is_complete
        requirement
        budget.routeBudget)

def zeroCostInspectStep : RouteStep :=
  { kind := StepKind.inspect
    evidenceTokens := 0
    promptTokens := 0
    cachedPromptTokens := 0
    roundCost := 0 }

def zeroPaddedRoute (padding : Nat) : SearchRoute :=
  { steps :=
      routerJumpStep ::
        List.replicate padding zeroCostInspectStep ++
          [closeStep]
    domain := 1
    closureDigest := 99
    closed := true
    graphEscalated := false
    semanticCacheValidated := false }

theorem zero_padding_transition_receipt (padding : Nat) :
    transitionCount (zeroPaddedRoute padding) =
      padding + 2 := by
  simp [transitionCount, zeroPaddedRoute]

theorem foldl_zero_evidence
    (padding initial : Nat) :
    (List.replicate padding zeroCostInspectStep).foldl
        (fun total step => total + step.evidenceTokens)
        initial =
      initial := by
  induction padding with
  | zero =>
      simp
  | succ remaining inductionHypothesis =>
      rw [List.replicate_succ, List.foldl_cons]
      change
        (List.replicate remaining zeroCostInspectStep).foldl
            (fun total step => total + step.evidenceTokens)
            initial =
          initial
      exact inductionHypothesis

theorem foldl_zero_prompt
    (padding initial : Nat) :
    (List.replicate padding zeroCostInspectStep).foldl
        (fun total step => total + step.promptTokens)
        initial =
      initial := by
  induction padding with
  | zero =>
      simp
  | succ remaining inductionHypothesis =>
      rw [List.replicate_succ, List.foldl_cons]
      change
        (List.replicate remaining zeroCostInspectStep).foldl
            (fun total step => total + step.promptTokens)
            initial =
          initial
      exact inductionHypothesis

theorem foldl_zero_uncached
    (padding initial : Nat) :
    (List.replicate padding zeroCostInspectStep).foldl
        (fun total step =>
          total + step.evidenceTokens +
            (step.promptTokens - step.cachedPromptTokens))
        initial =
      initial := by
  induction padding with
  | zero =>
      simp
  | succ remaining inductionHypothesis =>
      rw [List.replicate_succ, List.foldl_cons]
      change
        (List.replicate remaining zeroCostInspectStep).foldl
            (fun total step =>
              total + step.evidenceTokens +
                (step.promptTokens - step.cachedPromptTokens))
            initial =
          initial
      exact inductionHypothesis

theorem foldl_zero_rounds
    (padding initial : Nat) :
    (List.replicate padding zeroCostInspectStep).foldl
        (fun total step => total + step.roundCost)
        initial =
      initial := by
  induction padding with
  | zero =>
      simp
  | succ remaining inductionHypothesis =>
      rw [List.replicate_succ, List.foldl_cons]
      change
        (List.replicate remaining zeroCostInspectStep).foldl
            (fun total step => total + step.roundCost)
            initial =
          initial
      exact inductionHypothesis

theorem zero_padding_cost_receipt (padding : Nat) :
    evidenceTokenCost (zeroPaddedRoute padding) = 5 ∧
      promptTokenCost (zeroPaddedRoute padding) = 8 ∧
      rawTokenCost (zeroPaddedRoute padding) = 13 ∧
      uncachedTokenCost (zeroPaddedRoute padding) = 13 ∧
      roundCost (zeroPaddedRoute padding) = 2 := by
  simp [
    evidenceTokenCost,
    promptTokenCost,
    rawTokenCost,
    uncachedTokenCost,
    roundCost,
    zeroPaddedRoute,
    foldl_zero_evidence,
    foldl_zero_prompt,
    foldl_zero_uncached,
    foldl_zero_rounds,
    routerJumpStep,
    closeStep
  ]

theorem route_budget_allows_unbounded_zero_cost_transitions
    (padding : Nat) :
    feasibleB
        nonGraphRequirement
        generousRouteBudget
        (zeroPaddedRoute padding) = true := by
  have promptCacheSound :
      zeroCostInspectStep.cachedPromptTokens ≤
        zeroCostInspectStep.promptTokens := by
    decide
  have notSemanticCacheHit :
      zeroCostInspectStep.kind ≠ StepKind.semanticCacheHit := by
    decide
  simp [
    feasibleB,
    admissibleB,
    withinBudgetB,
    prefixCacheSoundB,
    semanticCacheSoundB,
    hasRoutingStep,
    containsKind,
    evidenceTokenCost,
    promptTokenCost,
    rawTokenCost,
    uncachedTokenCost,
    roundCost,
    zeroPaddedRoute,
    foldl_zero_evidence,
    foldl_zero_prompt,
    foldl_zero_uncached,
    foldl_zero_rounds,
    routerJumpStep,
    closeStep,
    nonGraphRequirement,
    generousRouteBudget,
    promptCacheSound,
    notSemanticCacheHit
  ]

theorem route_budget_has_no_transition_ceiling (limit : Nat) :
    ∃ route,
      feasibleB nonGraphRequirement generousRouteBudget route = true ∧
        transitionCount route > limit := by
  refine ⟨zeroPaddedRoute (limit + 1), ?_, ?_⟩
  · exact route_budget_allows_unbounded_zero_cost_transitions (limit + 1)
  · rw [zero_padding_transition_receipt]
    omega

theorem graph_route_budget_rejects_excess_zero_padding :
    graphFeasibleB
        nonGraphRequirement
        generousGraphBudget
        { route :=
            zeroPaddedRoute (generousGraphBudget.maxTransitions + 1)
          graphHops := 0
          pathValidated := true } = false := by
  simp [
    graphFeasibleB,
    zero_padding_transition_receipt,
    generousGraphBudget
  ]

def oneNodeGraph : RankedDAG :=
  { nodeCount := 1
    edge := fun _ _ => false
    rank := fun _ => 0
    maxRank := 0
    edgeRaisesRank := by
      intro source target impossible
      simp at impossible
    rankBounded := by
      intro node
      simp }

def onlyNode : Fin oneNodeGraph.nodeCount :=
  ⟨0, by decide⟩

def generatedGraphOnlyCandidate : GraphCandidate :=
  { route := routerJumpRoute
    graphHops := 0
    pathValidated := true }

def omittedRouteRealization : GraphCandidate :=
  { route := baselineLoopRoute
    graphHops := 0
    pathValidated := true }

def graphOnlyCandidates : List GraphCandidate :=
  [generatedGraphOnlyCandidate]

theorem graph_walk_completeness_does_not_imply_route_completeness :
    Realizes
        oneNodeGraph onlyNode onlyNode omittedRouteRealization ∧
      graphFeasibleB
          nonGraphRequirement
          generousGraphBudget
          omittedRouteRealization = true ∧
      omittedRouteRealization ∉ graphOnlyCandidates := by
  refine ⟨?_, by decide, by decide⟩
  exact Walk.stop onlyNode

theorem graph_only_candidates_are_not_candidate_complete :
    ¬CandidateComplete
      oneNodeGraph
      onlyNode
      onlyNode
      nonGraphRequirement
      generousGraphBudget
      graphOnlyCandidates := by
  intro claimedComplete
  have omittedMustBePresent :=
    claimedComplete
      omittedRouteRealization
      (Walk.stop onlyNode)
      (by decide)
  exact (by decide : omittedRouteRealization ∉ graphOnlyCandidates)
    omittedMustBePresent

end SearchRouteDAGEnumeration
