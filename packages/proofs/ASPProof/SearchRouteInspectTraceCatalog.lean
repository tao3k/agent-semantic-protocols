import ASPProof.SearchRouteInspectTraceCost

namespace SearchRouteInspectTraceCatalog

open SearchRouteCost
open SearchRouteDAG
open SearchRouteInspectLoop
open SearchRouteInspectTraceCost

theorem graph_candidate_lex_refl (candidate : GraphCandidate) :
    graphLexNoWorse candidate candidate := by
  simp [graphLexNoWorse]

theorem graph_candidate_lex_trans
    {first second third : GraphCandidate}
    (firstSecond : graphLexNoWorse first second)
    (secondThird : graphLexNoWorse second third) :
    graphLexNoWorse first third := by
  unfold graphLexNoWorse at *
  omega

theorem preferGraphCandidate_no_worse_left
    (left right : GraphCandidate) :
    graphLexNoWorse (preferGraphCandidate left right) left := by
  by_cases graphLeft : left.graphHops < right.graphHops
  · simp [preferGraphCandidate, graphLeft, graphLexNoWorse]
  · by_cases graphRight : right.graphHops < left.graphHops
    · simp [preferGraphCandidate, graphLeft, graphRight, graphLexNoWorse]
    · by_cases tokenLeft :
        uncachedTokenCost left.route < uncachedTokenCost right.route
      · simp [
          preferGraphCandidate,
          graphLeft,
          graphRight,
          tokenLeft,
          graphLexNoWorse
        ]
      · by_cases tokenRight :
          uncachedTokenCost right.route < uncachedTokenCost left.route
        · simp [
            preferGraphCandidate,
            graphLeft,
            graphRight,
            tokenLeft,
            tokenRight,
            graphLexNoWorse
          ]
          omega
        · by_cases roundLeft :
            roundCost left.route < roundCost right.route
          · simp [
              preferGraphCandidate,
              graphLeft,
              graphRight,
              tokenLeft,
              tokenRight,
              roundLeft,
              graphLexNoWorse
            ]
          · by_cases roundRight :
              roundCost right.route < roundCost left.route
            · simp [
                preferGraphCandidate,
                graphLeft,
                graphRight,
                tokenLeft,
                tokenRight,
                roundLeft,
                roundRight,
                graphLexNoWorse
              ]
              omega
            · by_cases transitionLeft :
                transitionCount left.route ≤ transitionCount right.route
              · simp [
                  preferGraphCandidate,
                  graphLeft,
                  graphRight,
                  tokenLeft,
                  tokenRight,
                  roundLeft,
                  roundRight,
                  transitionLeft,
                  graphLexNoWorse
                ]
              · simp [
                  preferGraphCandidate,
                  graphLeft,
                  graphRight,
                  tokenLeft,
                  tokenRight,
                  roundLeft,
                  roundRight,
                  transitionLeft,
                  graphLexNoWorse
                ]
                omega

theorem preferGraphCandidate_no_worse_right
    (left right : GraphCandidate) :
    graphLexNoWorse (preferGraphCandidate left right) right := by
  by_cases graphLeft : left.graphHops < right.graphHops
  · simp [preferGraphCandidate, graphLeft, graphLexNoWorse]
  · by_cases graphRight : right.graphHops < left.graphHops
    · simp [preferGraphCandidate, graphLeft, graphRight, graphLexNoWorse]
    · by_cases tokenLeft :
        uncachedTokenCost left.route < uncachedTokenCost right.route
      · simp [
          preferGraphCandidate,
          graphLeft,
          graphRight,
          tokenLeft,
          graphLexNoWorse
        ]
        omega
      · by_cases tokenRight :
          uncachedTokenCost right.route < uncachedTokenCost left.route
        · simp [
            preferGraphCandidate,
            graphLeft,
            graphRight,
            tokenLeft,
            tokenRight,
            graphLexNoWorse
          ]
        · by_cases roundLeft :
            roundCost left.route < roundCost right.route
          · simp [
              preferGraphCandidate,
              graphLeft,
              graphRight,
              tokenLeft,
              tokenRight,
              roundLeft,
              graphLexNoWorse
            ]
            omega
          · by_cases roundRight :
              roundCost right.route < roundCost left.route
            · simp [
                preferGraphCandidate,
                graphLeft,
                graphRight,
                tokenLeft,
                tokenRight,
                roundLeft,
                roundRight,
                graphLexNoWorse
              ]
            · by_cases transitionLeft :
                transitionCount left.route ≤ transitionCount right.route
              · simp [
                  preferGraphCandidate,
                  graphLeft,
                  graphRight,
                  tokenLeft,
                  tokenRight,
                  roundLeft,
                  roundRight,
                  transitionLeft,
                  graphLexNoWorse
                ]
                omega
              · simp [
                  preferGraphCandidate,
                  graphLeft,
                  graphRight,
                  tokenLeft,
                  tokenRight,
                  roundLeft,
                  roundRight,
                  transitionLeft,
                  graphLexNoWorse
                ]

theorem preferGraphCandidate_eq_left_or_right
    (left right : GraphCandidate) :
    preferGraphCandidate left right = left ∨
      preferGraphCandidate left right = right := by
  by_cases graphLeft : left.graphHops < right.graphHops
  · simp [preferGraphCandidate, graphLeft]
  · by_cases graphRight : right.graphHops < left.graphHops
    · simp [preferGraphCandidate, graphLeft, graphRight]
    · by_cases tokenLeft :
        uncachedTokenCost left.route < uncachedTokenCost right.route
      · simp [preferGraphCandidate, graphLeft, graphRight, tokenLeft]
      · by_cases tokenRight :
          uncachedTokenCost right.route < uncachedTokenCost left.route
        · simp [
            preferGraphCandidate,
            graphLeft,
            graphRight,
            tokenLeft,
            tokenRight
          ]
        · by_cases roundLeft :
            roundCost left.route < roundCost right.route
          · simp [
              preferGraphCandidate,
              graphLeft,
              graphRight,
              tokenLeft,
              tokenRight,
              roundLeft
            ]
          · by_cases roundRight :
              roundCost right.route < roundCost left.route
            · simp [
                preferGraphCandidate,
                graphLeft,
                graphRight,
                tokenLeft,
                tokenRight,
                roundLeft,
                roundRight
              ]
            · by_cases transitionLeft :
                transitionCount left.route ≤ transitionCount right.route
              · simp [
                  preferGraphCandidate,
                  graphLeft,
                  graphRight,
                  tokenLeft,
                  tokenRight,
                  roundLeft,
                  roundRight,
                  transitionLeft
                ]
              · simp [
                  preferGraphCandidate,
                  graphLeft,
                  graphRight,
                  tokenLeft,
                  tokenRight,
                  roundLeft,
                  roundRight,
                  transitionLeft
                ]

theorem chooseBestGraphCandidate_none_implies_infeasible
    (requirement : RouteRequirement)
    (budget : GraphRouteBudget)
    (candidates : List GraphCandidate)
    (chosenNone :
      chooseBestGraphCandidate requirement budget candidates = none) :
    ∀ candidate ∈ candidates,
      graphFeasibleB requirement budget candidate = false := by
  induction candidates with
  | nil =>
      simp
  | cons head tail inductionHypothesis =>
      cases headFeasible :
          graphFeasibleB requirement budget head with
      | false =>
          have tailNone :
              chooseBestGraphCandidate requirement budget tail = none := by
            simpa [chooseBestGraphCandidate, headFeasible] using chosenNone
          intro candidate candidateMember
          simp only [List.mem_cons] at candidateMember
          cases candidateMember with
          | inl candidateIsHead =>
              simpa [candidateIsHead] using headFeasible
          | inr candidateInTail =>
              exact inductionHypothesis tailNone candidate candidateInTail
      | true =>
          cases tailChosen :
              chooseBestGraphCandidate requirement budget tail with
          | none =>
              simp [
                chooseBestGraphCandidate,
                headFeasible,
                tailChosen
              ] at chosenNone
          | some current =>
              simp [
                chooseBestGraphCandidate,
                headFeasible,
                tailChosen
              ] at chosenNone

theorem chooseBestGraphCandidate_mem_and_feasible
    (requirement : RouteRequirement)
    (budget : GraphRouteBudget)
    (candidates : List GraphCandidate)
    (selected : GraphCandidate)
    (selectedByRouter :
      chooseBestGraphCandidate requirement budget candidates =
        some selected) :
    selected ∈ candidates ∧
      graphFeasibleB requirement budget selected = true := by
  induction candidates generalizing selected with
  | nil =>
      simp [chooseBestGraphCandidate] at selectedByRouter
  | cons head tail inductionHypothesis =>
      cases headFeasible :
          graphFeasibleB requirement budget head with
      | false =>
          have selectedFromTail :
              chooseBestGraphCandidate requirement budget tail =
                some selected := by
            simpa [chooseBestGraphCandidate, headFeasible] using
              selectedByRouter
          have tailReceipt :=
            inductionHypothesis selected selectedFromTail
          exact ⟨by simp [tailReceipt.1], tailReceipt.2⟩
      | true =>
          cases tailChosen :
              chooseBestGraphCandidate requirement budget tail with
          | none =>
              have selectedIsHead : selected = head := by
                simpa [
                  chooseBestGraphCandidate,
                  headFeasible,
                  tailChosen
                ] using selectedByRouter.symm
              subst selected
              exact ⟨by simp, headFeasible⟩
          | some current =>
              have selectedIsPreferred :
                  selected = preferGraphCandidate head current := by
                simpa [
                  chooseBestGraphCandidate,
                  headFeasible,
                  tailChosen
                ] using selectedByRouter.symm
              have currentReceipt :=
                inductionHypothesis current tailChosen
              subst selected
              cases preferGraphCandidate_eq_left_or_right head current with
              | inl preferredIsHead =>
                  rw [preferredIsHead]
                  exact ⟨by simp, headFeasible⟩
              | inr preferredIsCurrent =>
                  rw [preferredIsCurrent]
                  exact ⟨by simp [currentReceipt.1], currentReceipt.2⟩

theorem chooseBestGraphCandidate_is_lex_optimal
    (requirement : RouteRequirement)
    (budget : GraphRouteBudget)
    (candidates : List GraphCandidate)
    (selected : GraphCandidate)
    (selectedByRouter :
      chooseBestGraphCandidate requirement budget candidates =
        some selected) :
    ∀ candidate ∈ candidates,
      graphFeasibleB requirement budget candidate = true →
        graphLexNoWorse selected candidate := by
  induction candidates generalizing selected with
  | nil =>
      simp [chooseBestGraphCandidate] at selectedByRouter
  | cons head tail inductionHypothesis =>
      cases headFeasible :
          graphFeasibleB requirement budget head with
      | false =>
          have selectedFromTail :
              chooseBestGraphCandidate requirement budget tail =
                some selected := by
            simpa [chooseBestGraphCandidate, headFeasible] using
              selectedByRouter
          intro candidate candidateMember candidateFeasible
          simp only [List.mem_cons] at candidateMember
          cases candidateMember with
          | inl candidateIsHead =>
              subst candidate
              simp [headFeasible] at candidateFeasible
          | inr candidateInTail =>
              exact inductionHypothesis selected selectedFromTail
                candidate candidateInTail candidateFeasible
      | true =>
          cases tailChosen :
              chooseBestGraphCandidate requirement budget tail with
          | none =>
              have selectedIsHead : selected = head := by
                simpa [
                  chooseBestGraphCandidate,
                  headFeasible,
                  tailChosen
                ] using selectedByRouter.symm
              subst selected
              intro candidate candidateMember candidateFeasible
              simp only [List.mem_cons] at candidateMember
              cases candidateMember with
              | inl candidateIsHead =>
                  subst candidate
                  exact graph_candidate_lex_refl head
              | inr candidateInTail =>
                  have candidateInfeasible :=
                    chooseBestGraphCandidate_none_implies_infeasible
                      requirement budget tail tailChosen candidate
                      candidateInTail
                  simp [candidateInfeasible] at candidateFeasible
          | some current =>
              have selectedIsPreferred :
                  selected = preferGraphCandidate head current := by
                simpa [
                  chooseBestGraphCandidate,
                  headFeasible,
                  tailChosen
                ] using selectedByRouter.symm
              subst selected
              intro candidate candidateMember candidateFeasible
              simp only [List.mem_cons] at candidateMember
              cases candidateMember with
              | inl candidateIsHead =>
                  subst candidate
                  exact preferGraphCandidate_no_worse_left head current
              | inr candidateInTail =>
                  exact graph_candidate_lex_trans
                    (preferGraphCandidate_no_worse_right head current)
                    (inductionHypothesis current tailChosen candidate
                      candidateInTail candidateFeasible)

structure RealizedTraceCatalogEntry
    (initial : InspectLoopState)
    (graph : RankedDAG)
    (source target : Fin graph.nodeCount) where
  context : TraceRouteContext
  receipt : CostedClosedTraceReceipt initial
  realization :
    Realizes graph source target (receipt.toGraphCandidate context)

def RealizedTraceCatalogEntry.candidate
    {initial : InspectLoopState}
    {graph : RankedDAG}
    {source target : Fin graph.nodeCount}
    (entry : RealizedTraceCatalogEntry initial graph source target) :
    GraphCandidate :=
  entry.receipt.toGraphCandidate entry.context

def projectTraceCatalog
    {initial : InspectLoopState}
    {graph : RankedDAG}
    {source target : Fin graph.nodeCount}
    (catalog :
      List (RealizedTraceCatalogEntry initial graph source target)) :
    List GraphCandidate :=
  catalog.map RealizedTraceCatalogEntry.candidate

def RealizedTraceCatalogComplete
    {initial : InspectLoopState}
    (graph : RankedDAG)
    (source target : Fin graph.nodeCount)
    (requirement : RouteRequirement)
    (budget : GraphRouteBudget)
    (catalog :
      List (RealizedTraceCatalogEntry initial graph source target)) :
    Prop :=
  CandidateComplete
    graph source target requirement budget (projectTraceCatalog catalog)

theorem selected_candidate_has_trace_provenance
    {initial : InspectLoopState}
    {graph : RankedDAG}
    {source target : Fin graph.nodeCount}
    {requirement : RouteRequirement}
    {budget : GraphRouteBudget}
    {catalog :
      List (RealizedTraceCatalogEntry initial graph source target)}
    {selected : GraphCandidate}
    (selectedByRouter :
      chooseBestGraphCandidate
          requirement
          budget
          (projectTraceCatalog catalog) =
        some selected) :
    ∃ entry ∈ catalog, entry.candidate = selected := by
  have selectedMember :=
    (chooseBestGraphCandidate_mem_and_feasible
      requirement
      budget
      (projectTraceCatalog catalog)
      selected
      selectedByRouter).1
  rcases List.mem_map.1 selectedMember with
    ⟨entry, entryMember, entryProjectsToSelected⟩
  exact ⟨entry, entryMember, entryProjectsToSelected⟩

theorem selected_catalog_candidate_is_locally_optimal
    {initial : InspectLoopState}
    {graph : RankedDAG}
    {source target : Fin graph.nodeCount}
    {requirement : RouteRequirement}
    {budget : GraphRouteBudget}
    {catalog :
      List (RealizedTraceCatalogEntry initial graph source target)}
    {selected : GraphCandidate}
    (selectedByRouter :
      chooseBestGraphCandidate
          requirement
          budget
          (projectTraceCatalog catalog) =
        some selected) :
    LocallyOptimal
      graph
      source
      target
      requirement
      budget
      (projectTraceCatalog catalog)
      selected := by
  have selectedReceipt :=
    chooseBestGraphCandidate_mem_and_feasible
      requirement budget (projectTraceCatalog catalog)
      selected selectedByRouter
  rcases selected_candidate_has_trace_provenance selectedByRouter with
    ⟨entry, entryMember, entryProjectsToSelected⟩
  have realization : Realizes graph source target selected := by
    rw [← entryProjectsToSelected]
    exact entry.realization
  exact ⟨
    realization,
    selectedReceipt.1,
    selectedReceipt.2,
    chooseBestGraphCandidate_is_lex_optimal
      requirement budget (projectTraceCatalog catalog)
      selected selectedByRouter
  ⟩

theorem complete_trace_catalog_selects_global_driver
    {initial : InspectLoopState}
    {graph : RankedDAG}
    {source target : Fin graph.nodeCount}
    {requirement : RouteRequirement}
    {budget : GraphRouteBudget}
    {catalog :
      List (RealizedTraceCatalogEntry initial graph source target)}
    {selected : GraphCandidate}
    (complete :
      RealizedTraceCatalogComplete
        graph source target requirement budget catalog)
    (selectedByRouter :
      chooseBestGraphCandidate
          requirement
          budget
          (projectTraceCatalog catalog) =
        some selected) :
    (∃ entry ∈ catalog, entry.candidate = selected) ∧
      Realizes graph source target selected ∧
      ∀ candidate,
        Realizes graph source target candidate →
          graphFeasibleB requirement budget candidate = true →
            graphLexNoWorse selected candidate := by
  have provenance :=
    selected_candidate_has_trace_provenance selectedByRouter
  have global :=
    complete_generation_lifts_local_to_global
      complete
      (selected_catalog_candidate_is_locally_optimal selectedByRouter)
  exact ⟨provenance, global⟩

theorem incomplete_catalog_can_hide_cheaper_driver :
    chooseBestGraphCandidate
        nonGraphRequirement
        generousGraphBudget
        [shortInteractionLongGraph] =
      some shortInteractionLongGraph ∧
    graphFeasibleB
        nonGraphRequirement
        generousGraphBudget
        longInteractionShortGraph = true ∧
    ¬graphLexNoWorse
        shortInteractionLongGraph
        longInteractionShortGraph := by
  constructor
  · rfl
  constructor
  · rfl
  · simp [
      graphLexNoWorse,
      shortInteractionLongGraph,
      longInteractionShortGraph
    ]

end SearchRouteInspectTraceCatalog
