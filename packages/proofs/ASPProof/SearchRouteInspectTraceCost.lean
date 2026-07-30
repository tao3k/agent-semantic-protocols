import ASPProof.SearchRouteInspectDriver

namespace SearchRouteInspectTraceCost

open SearchRouteCost
open SearchRouteDAG
open SearchRouteInspectLoop

/-!
The repeated inspect loop proves semantic progress but carries no operational
cost.  This module adds a proof-relevant cost annotation to every decision and
projects a closed costed trace into the existing `SearchRoute` /
`GraphCandidate` cost model.
-/

structure CostedInspectStep where
  decision : LoopDecision
  routeStep : RouteStep
  graphHops : Nat
  semanticBaselineEvidenceTokens : Nat
  semanticSearchCacheSavedTokens : Nat
  semanticSearchCacheHit : Bool
  semanticSearchCacheValidated : Bool

def CostSound (cost : CostedInspectStep) : Prop :=
  cost.routeStep.cachedPromptTokens ≤ cost.routeStep.promptTokens ∧
    cost.semanticBaselineEvidenceTokens =
      cost.routeStep.evidenceTokens +
        cost.semanticSearchCacheSavedTokens ∧
    (cost.semanticSearchCacheHit = false →
      cost.semanticSearchCacheSavedTokens = 0) ∧
    (0 < cost.semanticSearchCacheSavedTokens →
      cost.semanticSearchCacheHit = true) ∧
    (cost.semanticSearchCacheHit = true →
      cost.semanticSearchCacheValidated = true) ∧
    (cost.semanticSearchCacheHit = true ↔
      cost.routeStep.kind = .semanticCacheHit)

inductive CostedInspectTrace :
    InspectLoopState →
      InspectLoopState →
        List CostedInspectStep →
          Prop where
  | stop (state : InspectLoopState) :
      CostedInspectTrace state state []
  | step
      {current finalState : InspectLoopState}
      {costs : List CostedInspectStep}
      (cost : CostedInspectStep)
      (valid : DecisionValid current cost.decision)
      (sound : CostSound cost)
      (rest :
        CostedInspectTrace
          (applyDecision current cost.decision)
          finalState
          costs) :
      CostedInspectTrace current finalState (cost :: costs)

structure CostedClosedTraceReceipt (initial : InspectLoopState) where
  finalState : InspectLoopState
  costs : List CostedInspectStep
  trace : CostedInspectTrace initial finalState costs
  closed : LoopClosed finalState
  pathValidated : Bool
  pathValidationReceipt : pathValidated = true

structure TraceRouteContext where
  domain : DomainId
  closureDigest : ClosureDigest

def traceGraphHops : List CostedInspectStep → Nat
  | [] => 0
  | cost :: costs => cost.graphHops + traceGraphHops costs

def traceActualEvidenceTokens : List CostedInspectStep → Nat
  | [] => 0
  | cost :: costs =>
      cost.routeStep.evidenceTokens + traceActualEvidenceTokens costs

def traceSemanticBaselineTokens : List CostedInspectStep → Nat
  | [] => 0
  | cost :: costs =>
      cost.semanticBaselineEvidenceTokens +
        traceSemanticBaselineTokens costs

def traceSemanticSearchCacheSavings : List CostedInspectStep → Nat
  | [] => 0
  | cost :: costs =>
      cost.semanticSearchCacheSavedTokens +
        traceSemanticSearchCacheSavings costs

def tracePromptTokens : List CostedInspectStep → Nat
  | [] => 0
  | cost :: costs =>
      cost.routeStep.promptTokens + tracePromptTokens costs

def tracePrefixCacheSavings : List CostedInspectStep → Nat
  | [] => 0
  | cost :: costs =>
      cost.routeStep.cachedPromptTokens + tracePrefixCacheSavings costs

def traceUncachedPromptTokens : List CostedInspectStep → Nat
  | [] => 0
  | cost :: costs =>
      (cost.routeStep.promptTokens -
        cost.routeStep.cachedPromptTokens) +
          traceUncachedPromptTokens costs

def traceUncachedTokenCost (costs : List CostedInspectStep) : Nat :=
  costs.foldl
    (fun total cost =>
      total + cost.routeStep.evidenceTokens +
        (cost.routeStep.promptTokens -
          cost.routeStep.cachedPromptTokens))
    0

def traceRoundCost (costs : List CostedInspectStep) : Nat :=
  costs.foldl
    (fun total cost => total + cost.routeStep.roundCost)
    0

def traceGraphEscalatedB (costs : List CostedInspectStep) : Bool :=
  costs.any fun cost => 0 < cost.graphHops

def traceSemanticCacheValidatedB
    (costs : List CostedInspectStep) : Bool :=
  costs.all fun cost =>
    !cost.semanticSearchCacheHit ||
      cost.semanticSearchCacheValidated

def projectRoute
    (context : TraceRouteContext)
    (costs : List CostedInspectStep)
    (closed : Bool) :
    SearchRoute :=
  {
    steps := costs.map CostedInspectStep.routeStep
    domain := context.domain
    closureDigest := context.closureDigest
    closed := closed
    graphEscalated := traceGraphEscalatedB costs
    semanticCacheValidated := traceSemanticCacheValidatedB costs
  }

def projectGraphCandidate
    (context : TraceRouteContext)
    (costs : List CostedInspectStep)
    (closed pathValidated : Bool) :
    GraphCandidate :=
  {
    route := projectRoute context costs closed
    graphHops := traceGraphHops costs
    pathValidated := pathValidated
  }

def CostedClosedTraceReceipt.toGraphCandidate
    {initial : InspectLoopState}
    (context : TraceRouteContext)
    (receipt : CostedClosedTraceReceipt initial) :
    GraphCandidate :=
  projectGraphCandidate context receipt.costs true receipt.pathValidated

theorem costed_trace_length_matches_decisions
    {initial finalState : InspectLoopState}
    {costs : List CostedInspectStep}
    (trace : CostedInspectTrace initial finalState costs) :
    ∃ steps,
      InspectLoopTrace initial finalState steps ∧
        costs.length = steps := by
  induction trace with
  | stop state =>
      exact ⟨0, .stop state, rfl⟩
  | step cost valid sound rest inductionHypothesis =>
      rcases inductionHypothesis with ⟨steps, plainTrace, lengthMatches⟩
      exact ⟨
        steps + 1,
        .step cost.decision valid plainTrace,
        by simp [lengthMatches]
      ⟩

theorem projected_transition_count_is_trace_length
    {initial : InspectLoopState}
    (context : TraceRouteContext)
    (receipt : CostedClosedTraceReceipt initial) :
    transitionCount (receipt.toGraphCandidate context).route =
      receipt.costs.length := by
  simp [
    CostedClosedTraceReceipt.toGraphCandidate,
    projectGraphCandidate,
    projectRoute,
    transitionCount
  ]

private theorem evidence_foldl_matches_recursive
    (costs : List CostedInspectStep)
    (accumulator : Nat) :
    costs.foldl
        (fun total cost =>
          total + cost.routeStep.evidenceTokens)
        accumulator =
      accumulator + traceActualEvidenceTokens costs := by
  induction costs generalizing accumulator with
  | nil =>
      simp [traceActualEvidenceTokens]
  | cons cost costs inductionHypothesis =>
      simp only [List.foldl_cons, traceActualEvidenceTokens]
      rw [inductionHypothesis]
      omega

private theorem prompt_foldl_matches_recursive
    (costs : List CostedInspectStep)
    (accumulator : Nat) :
    costs.foldl
        (fun total cost =>
          total + cost.routeStep.promptTokens)
        accumulator =
      accumulator + tracePromptTokens costs := by
  induction costs generalizing accumulator with
  | nil =>
      simp [tracePromptTokens]
  | cons cost costs inductionHypothesis =>
      simp only [List.foldl_cons, tracePromptTokens]
      rw [inductionHypothesis]
      omega

theorem projected_evidence_tokens_are_trace_actual
    (context : TraceRouteContext)
    (costs : List CostedInspectStep)
    (closed : Bool) :
    evidenceTokenCost (projectRoute context costs closed) =
      traceActualEvidenceTokens costs := by
  simp only [evidenceTokenCost, projectRoute, List.foldl_map]
  simpa using evidence_foldl_matches_recursive costs 0

theorem projected_prompt_tokens_are_trace_prompt
    (context : TraceRouteContext)
    (costs : List CostedInspectStep)
    (closed : Bool) :
    promptTokenCost (projectRoute context costs closed) =
      tracePromptTokens costs := by
  simp only [promptTokenCost, projectRoute, List.foldl_map]
  simpa using prompt_foldl_matches_recursive costs 0

theorem projected_uncached_tokens_are_trace_cost
    (context : TraceRouteContext)
    (costs : List CostedInspectStep)
    (closed : Bool) :
    uncachedTokenCost (projectRoute context costs closed) =
      traceUncachedTokenCost costs := by
  simp [
    uncachedTokenCost,
    projectRoute,
    traceUncachedTokenCost,
    List.foldl_map
  ]

theorem projected_rounds_are_trace_cost
    (context : TraceRouteContext)
    (costs : List CostedInspectStep)
    (closed : Bool) :
    roundCost (projectRoute context costs closed) =
      traceRoundCost costs := by
  simp [
    roundCost,
    projectRoute,
    traceRoundCost,
    List.foldl_map
  ]

theorem semantic_search_cache_accounting
    {initial finalState : InspectLoopState}
    {costs : List CostedInspectStep}
    (trace : CostedInspectTrace initial finalState costs) :
    traceSemanticBaselineTokens costs =
      traceActualEvidenceTokens costs +
        traceSemanticSearchCacheSavings costs := by
  induction trace with
  | stop state =>
      rfl
  | step cost valid sound rest inductionHypothesis =>
      simp only [
        traceSemanticBaselineTokens,
        traceActualEvidenceTokens,
        traceSemanticSearchCacheSavings
      ]
      have costAccounting := sound.2.1
      omega

theorem model_prefix_cache_accounting
    {initial finalState : InspectLoopState}
    {costs : List CostedInspectStep}
    (trace : CostedInspectTrace initial finalState costs) :
    tracePromptTokens costs =
      traceUncachedPromptTokens costs +
        tracePrefixCacheSavings costs := by
  induction trace with
  | stop state =>
      rfl
  | step cost valid sound rest inductionHypothesis =>
      simp only [
        tracePromptTokens,
        traceUncachedPromptTokens,
        tracePrefixCacheSavings
      ]
      have prefixSound := sound.1
      omega

theorem projected_semantic_search_cache_accounting
    {initial finalState : InspectLoopState}
    {costs : List CostedInspectStep}
    (trace : CostedInspectTrace initial finalState costs)
    (context : TraceRouteContext)
    (closed : Bool) :
    traceSemanticBaselineTokens costs =
      evidenceTokenCost (projectRoute context costs closed) +
        traceSemanticSearchCacheSavings costs := by
  rw [projected_evidence_tokens_are_trace_actual]
  exact semantic_search_cache_accounting trace

theorem projected_model_prefix_cache_accounting
    {initial finalState : InspectLoopState}
    {costs : List CostedInspectStep}
    (trace : CostedInspectTrace initial finalState costs)
    (context : TraceRouteContext)
    (closed : Bool) :
    promptTokenCost (projectRoute context costs closed) =
      traceUncachedPromptTokens costs +
        tracePrefixCacheSavings costs := by
  rw [projected_prompt_tokens_are_trace_prompt]
  exact model_prefix_cache_accounting trace

theorem projected_closed_receipt_is_closed_and_path_validated
    {initial : InspectLoopState}
    (context : TraceRouteContext)
    (receipt : CostedClosedTraceReceipt initial) :
    (receipt.toGraphCandidate context).route.closed = true ∧
      (receipt.toGraphCandidate context).pathValidated = true := by
  constructor
  · rfl
  · exact receipt.pathValidationReceipt

theorem costed_trace_all_steps_are_sound
    {initial finalState : InspectLoopState}
    {costs : List CostedInspectStep}
    (trace : CostedInspectTrace initial finalState costs) :
    ∀ cost ∈ costs, CostSound cost := by
  induction trace with
  | stop state =>
      simp
  | step cost valid sound rest inductionHypothesis =>
      intro candidate candidateMember
      simp only [List.mem_cons] at candidateMember
      cases candidateMember with
      | inl isHead =>
          subst candidate
          exact sound
      | inr isRest =>
          exact inductionHypothesis candidate isRest

theorem projected_prefix_cache_is_sound
    {initial finalState : InspectLoopState}
    {costs : List CostedInspectStep}
    (trace : CostedInspectTrace initial finalState costs)
    (context : TraceRouteContext)
    (closed : Bool) :
    prefixCacheSoundB (projectRoute context costs closed) = true := by
  have allSound := costed_trace_all_steps_are_sound trace
  simpa [prefixCacheSoundB, projectRoute] using
    (fun cost costMember => (allSound cost costMember).1)

theorem trace_semantic_cache_validation_is_sound
    {initial finalState : InspectLoopState}
    {costs : List CostedInspectStep}
    (trace : CostedInspectTrace initial finalState costs) :
    traceSemanticCacheValidatedB costs = true := by
  have allSound := costed_trace_all_steps_are_sound trace
  simp [traceSemanticCacheValidatedB]
  intro cost costMember
  have sound := allSound cost costMember
  cases cacheHit : cost.semanticSearchCacheHit with
  | false =>
      exact Or.inl rfl
  | true =>
      exact Or.inr (sound.2.2.2.2.1 cacheHit)

theorem projected_semantic_cache_is_sound
    {initial finalState : InspectLoopState}
    {costs : List CostedInspectStep}
    (trace : CostedInspectTrace initial finalState costs)
    (context : TraceRouteContext)
    (closed : Bool) :
    semanticCacheSoundB (projectRoute context costs closed) = true := by
  have validated := trace_semantic_cache_validation_is_sound trace
  simp [
    semanticCacheSoundB,
    projectRoute,
    validated
  ]

def semanticOnlyCacheStep (decision : LoopDecision) : CostedInspectStep :=
  {
    decision := decision
    routeStep :=
      {
        kind := .semanticCacheHit
        evidenceTokens := 2
        promptTokens := 0
        cachedPromptTokens := 0
        roundCost := 0
      }
    graphHops := 0
    semanticBaselineEvidenceTokens := 10
    semanticSearchCacheSavedTokens := 8
    semanticSearchCacheHit := true
    semanticSearchCacheValidated := true
  }

def prefixOnlyCacheStep (decision : LoopDecision) : CostedInspectStep :=
  {
    decision := decision
    routeStep :=
      {
        kind := .inspect
        evidenceTokens := 0
        promptTokens := 10
        cachedPromptTokens := 8
        roundCost := 0
      }
    graphHops := 0
    semanticBaselineEvidenceTokens := 0
    semanticSearchCacheSavedTokens := 0
    semanticSearchCacheHit := false
    semanticSearchCacheValidated := false
  }

theorem cache_layer_examples_are_sound (decision : LoopDecision) :
    CostSound (semanticOnlyCacheStep decision) ∧
      CostSound (prefixOnlyCacheStep decision) := by
  simp [CostSound, semanticOnlyCacheStep, prefixOnlyCacheStep]

def cacheCollisionContext : TraceRouteContext :=
  { domain := 0, closureDigest := 0 }

theorem equal_total_cost_does_not_identify_cache_layer
    (decision : LoopDecision) :
    let semanticCandidate :=
      projectGraphCandidate
        cacheCollisionContext
        [semanticOnlyCacheStep decision]
        true
        true
    let prefixCandidate :=
      projectGraphCandidate
        cacheCollisionContext
        [prefixOnlyCacheStep decision]
        true
        true
    graphLexNoWorse semanticCandidate prefixCandidate ∧
      graphLexNoWorse prefixCandidate semanticCandidate ∧
      traceSemanticSearchCacheSavings [semanticOnlyCacheStep decision] = 8 ∧
      tracePrefixCacheSavings [semanticOnlyCacheStep decision] = 0 ∧
      traceSemanticSearchCacheSavings [prefixOnlyCacheStep decision] = 0 ∧
      tracePrefixCacheSavings [prefixOnlyCacheStep decision] = 8 := by
  simp [
    graphLexNoWorse,
    projectGraphCandidate,
    projectRoute,
    cacheCollisionContext,
    semanticOnlyCacheStep,
    prefixOnlyCacheStep,
    traceGraphHops,
    traceSemanticSearchCacheSavings,
    tracePrefixCacheSavings,
    uncachedTokenCost,
    roundCost,
    transitionCount
  ]

end SearchRouteInspectTraceCost
