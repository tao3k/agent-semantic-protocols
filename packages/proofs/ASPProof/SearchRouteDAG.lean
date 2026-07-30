import ASPProof.SearchRouteCost

namespace SearchRouteDAG

open SearchRouteCost

structure RankedDAG where
  nodeCount : Nat
  edge : Fin nodeCount → Fin nodeCount → Bool
  rank : Fin nodeCount → Nat
  maxRank : Nat
  edgeRaisesRank :
    ∀ {source target},
      edge source target = true →
      rank source < rank target
  rankBounded :
    ∀ node, rank node ≤ maxRank

inductive Walk
    (graph : RankedDAG) :
    Fin graph.nodeCount →
    Fin graph.nodeCount →
    Nat →
    Prop where
  | stop (node) :
      Walk graph node node 0
  | step
      {source next target hops}
      (edgeReceipt : graph.edge source next = true)
      (rest : Walk graph next target hops) :
      Walk graph source target (hops + 1)

theorem walk_rank_progress
    {graph : RankedDAG}
    {source target : Fin graph.nodeCount}
    {hops : Nat}
    (walk : Walk graph source target hops) :
    graph.rank source + hops ≤ graph.rank target := by
  induction walk with
  | stop =>
      simp
  | step edgeReceipt rest inductionHypothesis =>
      have edgeProgress := graph.edgeRaisesRank edgeReceipt
      omega

theorem ranked_dag_has_no_positive_cycle
    {graph : RankedDAG}
    {node : Fin graph.nodeCount}
    {hops : Nat}
    (cycle : Walk graph node node hops) :
    hops = 0 := by
  have progress := walk_rank_progress cycle
  omega

theorem walk_hops_are_bounded
    {graph : RankedDAG}
    {source target : Fin graph.nodeCount}
    {hops : Nat}
    (walk : Walk graph source target hops) :
    hops + graph.rank source ≤ graph.maxRank := by
  have progress := walk_rank_progress walk
  have targetBound := graph.rankBounded target
  omega

structure GraphCandidate where
  route : SearchRoute
  graphHops : Nat
  pathValidated : Bool
  deriving DecidableEq, Repr

structure GraphRouteBudget where
  routeBudget : RouteBudget
  maxGraphHops : Nat
  maxTransitions : Nat
  deriving DecidableEq, Repr

def graphFeasibleB
    (requirement : RouteRequirement)
    (budget : GraphRouteBudget)
    (candidate : GraphCandidate) :
    Bool :=
  candidate.pathValidated &&
    candidate.graphHops ≤ budget.maxGraphHops &&
    transitionCount candidate.route ≤ budget.maxTransitions &&
    feasibleB requirement budget.routeBudget candidate.route

def graphLexNoWorse
    (left right : GraphCandidate) :
    Prop :=
  left.graphHops < right.graphHops ∨
    (left.graphHops = right.graphHops ∧
      (uncachedTokenCost left.route < uncachedTokenCost right.route ∨
        (uncachedTokenCost left.route =
            uncachedTokenCost right.route ∧
          (roundCost left.route < roundCost right.route ∨
            (roundCost left.route = roundCost right.route ∧
              transitionCount left.route ≤
                transitionCount right.route)))))

def preferGraphCandidate
    (left right : GraphCandidate) :
    GraphCandidate :=
  if left.graphHops < right.graphHops then
    left
  else if right.graphHops < left.graphHops then
    right
  else if uncachedTokenCost left.route <
      uncachedTokenCost right.route then
    left
  else if uncachedTokenCost right.route <
      uncachedTokenCost left.route then
    right
  else if roundCost left.route < roundCost right.route then
    left
  else if roundCost right.route < roundCost left.route then
    right
  else if transitionCount left.route ≤ transitionCount right.route then
    left
  else
    right

def chooseBestGraphCandidate
    (requirement : RouteRequirement)
    (budget : GraphRouteBudget) :
    List GraphCandidate →
    Option GraphCandidate
  | [] => none
  | candidate :: rest =>
      let chosenRest :=
        chooseBestGraphCandidate requirement budget rest
      if graphFeasibleB requirement budget candidate then
        match chosenRest with
        | none => some candidate
        | some current =>
            some (preferGraphCandidate candidate current)
      else
        chosenRest

def Realizes
    (graph : RankedDAG)
    (source target : Fin graph.nodeCount)
    (candidate : GraphCandidate) :
    Prop :=
  Walk graph source target candidate.graphHops

def CandidateComplete
    (graph : RankedDAG)
    (source target : Fin graph.nodeCount)
    (requirement : RouteRequirement)
    (budget : GraphRouteBudget)
    (generated : List GraphCandidate) :
    Prop :=
  ∀ candidate,
    Realizes graph source target candidate →
    graphFeasibleB requirement budget candidate = true →
    candidate ∈ generated

def LocallyOptimal
    (graph : RankedDAG)
    (source target : Fin graph.nodeCount)
    (requirement : RouteRequirement)
    (budget : GraphRouteBudget)
    (generated : List GraphCandidate)
    (selected : GraphCandidate) :
    Prop :=
  Realizes graph source target selected ∧
    selected ∈ generated ∧
    graphFeasibleB requirement budget selected = true ∧
    ∀ candidate ∈ generated,
      graphFeasibleB requirement budget candidate = true →
      graphLexNoWorse selected candidate

theorem complete_generation_lifts_local_to_global
    {graph : RankedDAG}
    {source target : Fin graph.nodeCount}
    {requirement : RouteRequirement}
    {budget : GraphRouteBudget}
    {generated : List GraphCandidate}
    {selected : GraphCandidate}
    (complete :
      CandidateComplete
        graph source target requirement budget generated)
    (localOptimal :
      LocallyOptimal
        graph source target requirement budget generated selected) :
    Realizes graph source target selected ∧
      ∀ candidate,
        Realizes graph source target candidate →
        graphFeasibleB requirement budget candidate = true →
        graphLexNoWorse selected candidate := by
  refine ⟨localOptimal.1, ?_⟩
  intro candidate realization feasible
  exact localOptimal.2.2.2 candidate
    (complete candidate realization feasible)
    feasible

def generousRouteBudget : RouteBudget :=
  { maxEvidenceTokens := 100
    maxRawTokens := 120
    maxUncachedTokens := 120
    maxRounds := 8 }

def generousGraphBudget : GraphRouteBudget :=
  { routeBudget := generousRouteBudget
    maxGraphHops := 8
    maxTransitions := 8 }

def shortInteractionLongGraph : GraphCandidate :=
  { route := routerJumpRoute
    graphHops := 5
    pathValidated := true }

def longInteractionShortGraph : GraphCandidate :=
  { route := baselineLoopRoute
    graphHops := 2
    pathValidated := true }

def unvalidatedShortestGraph : GraphCandidate :=
  { route := jointCacheHitRoute
    graphHops := 1
    pathValidated := false }

theorem transition_minimization_can_choose_longer_graph_path :
    preferRoute
        shortInteractionLongGraph.route
        longInteractionShortGraph.route =
      shortInteractionLongGraph.route ∧
    preferGraphCandidate
        shortInteractionLongGraph
        longInteractionShortGraph =
      longInteractionShortGraph ∧
    longInteractionShortGraph.graphHops <
      shortInteractionLongGraph.graphHops := by
  decide

theorem corrected_router_selects_shorter_validated_graph_path :
    chooseBestGraphCandidate
        nonGraphRequirement
        generousGraphBudget
        [shortInteractionLongGraph, longInteractionShortGraph] =
      some longInteractionShortGraph := by
  decide

theorem unvalidated_graph_path_is_not_a_shortcut :
    unvalidatedShortestGraph.graphHops <
        longInteractionShortGraph.graphHops ∧
      graphFeasibleB
          nonGraphRequirement
          generousGraphBudget
          unvalidatedShortestGraph = false ∧
      chooseBestGraphCandidate
          nonGraphRequirement
          generousGraphBudget
          [unvalidatedShortestGraph, longInteractionShortGraph] =
        some longInteractionShortGraph := by
  decide

end SearchRouteDAG
