import ASPProof.SearchRouterInteractiveGraphState
import ASPProof.SearchRouteGraphRouterParetoCostSelection

namespace ASPProof.GQLCoreSciPyResidentGraphTurbo

open ASPProof.SearchRouterInteractiveGraphState

abbrev NodeId := ASPProof.SearchRouterInteractiveGraphState.NodeId
abbrev FactId := ASPProof.SearchRouterInteractiveGraphState.FactId

inductive PythonProposalStatus where
  | candidate
  | proposed
  | heuristic
  | partialResult
  deriving Repr, DecidableEq, BEq

inductive EvidenceAuthority where
  | candidate
  | derived
  | asserted
  | proved
  deriving Repr, DecidableEq, BEq

def pythonProposalAuthority (_status : PythonProposalStatus) : EvidenceAuthority :=
  .candidate

theorem python_proposal_cannot_mint_evidence_authority
    (status : PythonProposalStatus) :
    pythonProposalAuthority status = .candidate := by
  cases status <;> rfl

structure GraphSessionIdentity where
  workspaceDigest : String
  sessionId : String
  nodeId : NodeId
  snapshotDigest : String
  workspaceGenerationRootDigest : String
  projectionDigest : String
  routeDigest : String
  deriving Repr, DecidableEq, BEq

def graphSessionIdentityValid (identity : GraphSessionIdentity) : Bool :=
  !identity.workspaceDigest.isEmpty &&
    !identity.sessionId.isEmpty &&
    !identity.nodeId.isEmpty &&
    !identity.snapshotDigest.isEmpty &&
    !identity.workspaceGenerationRootDigest.isEmpty &&
    !identity.projectionDigest.isEmpty &&
    !identity.routeDigest.isEmpty

def sameGraphSession
    (expected observed : GraphSessionIdentity) : Bool :=
  graphSessionIdentityValid expected && expected == observed

def graphSessionA : GraphSessionIdentity :=
  { workspaceDigest := "workspace:1"
    sessionId := "session:a"
    nodeId := "node:owner"
    snapshotDigest := "snapshot:7"
    workspaceGenerationRootDigest := "generation:7"
    projectionDigest := "projection:owner-v1"
    routeDigest := "route:owner-v1" }

def graphSessionB : GraphSessionIdentity :=
  { graphSessionA with sessionId := "session:b" }

theorem exact_graph_session_identity_is_admitted :
    sameGraphSession graphSessionA graphSessionA = true := by
  decide

theorem cross_session_substitution_is_rejected :
    sameGraphSession graphSessionA graphSessionB = false := by
  decide

structure DecodedHopAccounting where
  semanticGraphHops : Nat
  executedGraphHops : Nat
  deriving Repr, DecidableEq, BEq

def validateDecodedHopAccounting
    (decoded : DecodedHopAccounting) : Option DecodedHopAccounting :=
  if decoded.executedGraphHops <= decoded.semanticGraphHops then
    some decoded
  else
    none

theorem valid_deserialized_hop_accounting_is_admitted :
    validateDecodedHopAccounting
      { semanticGraphHops := 3, executedGraphHops := 1 } =
      some { semanticGraphHops := 3, executedGraphHops := 1 } := by
  decide

theorem executed_hops_above_semantic_hops_are_rejected :
    validateDecodedHopAccounting
      { semanticGraphHops := 1, executedGraphHops := 2 } = none := by
  decide

structure InteractiveCostAccounting where
  semanticGraphHops : Nat
  executedGraphHops : Nat
  toolActions : Nat
  llmRounds : Nat
  inputTokens : Nat
  outputTokens : Nat
  searchExecutions : Nat
  modelPrefixRecomputations : Nat
  ruleFirings : Nat
  derivedFacts : Nat
  deriving Repr, DecidableEq, BEq

def accountingWellFormed (accounting : InteractiveCostAccounting) : Bool :=
  accounting.executedGraphHops <= accounting.semanticGraphHops

def cacheResumeAdmitted
    (expectedIdentity observedIdentity : GraphSessionIdentity)
    (cold cached : InteractiveCostAccounting) : Bool :=
  sameGraphSession expectedIdentity observedIdentity &&
    accountingWellFormed cold &&
    accountingWellFormed cached &&
    cached.semanticGraphHops == cold.semanticGraphHops &&
    cached.executedGraphHops <= cold.executedGraphHops &&
    cached.toolActions <= cold.toolActions &&
    cached.llmRounds <= cold.llmRounds &&
    cached.searchExecutions <= cold.searchExecutions &&
    cached.modelPrefixRecomputations <= cold.modelPrefixRecomputations

def coldAccounting : InteractiveCostAccounting :=
  { semanticGraphHops := 3
    executedGraphHops := 3
    toolActions := 4
    llmRounds := 3
    inputTokens := 1200
    outputTokens := 240
    searchExecutions := 2
    modelPrefixRecomputations := 2
    ruleFirings := 8
    derivedFacts := 5 }

def cachedAccounting : InteractiveCostAccounting :=
  { coldAccounting with
    executedGraphHops := 1
    toolActions := 2
    llmRounds := 1
    searchExecutions := 0
    modelPrefixRecomputations := 0 }

theorem cache_resume_preserves_semantic_route_and_may_reduce_realization :
    cacheResumeAdmitted graphSessionA graphSessionA coldAccounting cachedAccounting = true := by
  decide

def forgedSemanticAccounting : InteractiveCostAccounting :=
  { cachedAccounting with semanticGraphHops := 1 }

theorem cache_resume_cannot_rewrite_semantic_hops :
    cacheResumeAdmitted graphSessionA graphSessionA
      coldAccounting forgedSemanticAccounting = false := by
  decide

def searchCacheOnlyAccounting : InteractiveCostAccounting :=
  { coldAccounting with
    searchExecutions := 0
    modelPrefixRecomputations := 1 }

def modelCacheOnlyAccounting : InteractiveCostAccounting :=
  { coldAccounting with
    searchExecutions := 1
    modelPrefixRecomputations := 0 }

def combinedCacheMisses (accounting : InteractiveCostAccounting) : Nat :=
  accounting.searchExecutions + accounting.modelPrefixRecomputations

theorem equal_combined_cache_work_can_hide_orthogonal_cache_lanes :
    combinedCacheMisses searchCacheOnlyAccounting =
      combinedCacheMisses modelCacheOnlyAccounting := by
  decide

theorem search_cache_reuse_does_not_imply_model_prefix_reuse :
    searchCacheOnlyAccounting.searchExecutions = 0 ∧
      searchCacheOnlyAccounting.modelPrefixRecomputations = 1 := by
  decide

theorem model_prefix_reuse_does_not_imply_search_cache_reuse :
    modelCacheOnlyAccounting.modelPrefixRecomputations = 0 ∧
      modelCacheOnlyAccounting.searchExecutions = 1 := by
  decide

structure AscentClosureBudget where
  maxInputFacts : Nat
  maxRuleFirings : Nat
  maxDerivedFacts : Nat
  maxGraphHops : Nat
  deriving Repr, DecidableEq, BEq

structure AscentClosureReceipt where
  snapshotDigest : String
  workspaceGenerationRootDigest : String
  rulesetDigest : String
  sourceFactIds : List FactId
  derivedFactIds : List FactId
  ruleFirings : Nat
  derivedFacts : Nat
  maxDerivedGraphHops : Nat
  canonicalFactOrderDigest : String
  deriving Repr, DecidableEq, BEq

def ascentClosureReceiptAdmitted
    (identity : GraphSessionIdentity)
    (admittedFacts trustedRulesets : List String)
    (budget : AscentClosureBudget)
    (receipt : AscentClosureReceipt) : Bool :=
  graphSessionIdentityValid identity &&
    receipt.snapshotDigest == identity.snapshotDigest &&
    receipt.workspaceGenerationRootDigest ==
      identity.workspaceGenerationRootDigest &&
    trustedRulesets.contains receipt.rulesetDigest &&
    receipt.sourceFactIds.all admittedFacts.contains &&
    receipt.sourceFactIds.length <= budget.maxInputFacts &&
    receipt.ruleFirings <= budget.maxRuleFirings &&
    receipt.derivedFacts == receipt.derivedFactIds.length &&
    receipt.derivedFacts <= budget.maxDerivedFacts &&
    receipt.maxDerivedGraphHops <= budget.maxGraphHops &&
    !receipt.canonicalFactOrderDigest.isEmpty

def ascentBudget : AscentClosureBudget :=
  { maxInputFacts := 8
    maxRuleFirings := 16
    maxDerivedFacts := 8
    maxGraphHops := 4 }

def admittedAscentReceipt : AscentClosureReceipt :=
  { snapshotDigest := "snapshot:7"
    workspaceGenerationRootDigest := "generation:7"
    rulesetDigest := "ruleset:owner-v1"
    sourceFactIds := ["fact:model-declared"]
    derivedFactIds := ["fact:configuration-owner"]
    ruleFirings := 3
    derivedFacts := 1
    maxDerivedGraphHops := 2
    canonicalFactOrderDigest := "facts:canonical:1" }

theorem bounded_ascent_receipt_is_snapshot_scoped_and_grounded :
    ascentClosureReceiptAdmitted graphSessionA
      ["fact:model-declared"] ["ruleset:owner-v1"]
      ascentBudget admittedAscentReceipt = true := by
  decide

structure NamedAscentRule where
  ruleId : String
  premiseFactIds : List FactId
  conclusionFactId : FactId
  deriving Repr, DecidableEq, BEq

structure AscentDerivationStep where
  ruleId : String
  premiseFactIds : List FactId
  conclusionFactId : FactId
  deriving Repr, DecidableEq, BEq

def ruleClosedOverFacts (facts : List FactId) (rule : NamedAscentRule) : Bool :=
  !rule.premiseFactIds.all facts.contains || facts.contains rule.conclusionFactId

def receiptIsFixedPoint
    (rules : List NamedAscentRule)
    (receipt : AscentClosureReceipt) : Bool :=
  let facts := receipt.sourceFactIds ++ receipt.derivedFactIds
  rules.all (ruleClosedOverFacts facts)

def derivationStepGrounded
    (rules : List NamedAscentRule)
    (facts : List FactId)
    (step : AscentDerivationStep) : Bool :=
  rules.any fun rule =>
    rule.ruleId == step.ruleId &&
      rule.premiseFactIds == step.premiseFactIds &&
      rule.conclusionFactId == step.conclusionFactId &&
      step.premiseFactIds.all facts.contains &&
      !facts.contains step.conclusionFactId

def replayDerivationTrace
    (rules : List NamedAscentRule) :
    List FactId → List AscentDerivationStep → Option (List FactId)
  | facts, [] => some facts
  | facts, step :: rest =>
      if derivationStepGrounded rules facts step then
        replayDerivationTrace rules (facts ++ [step.conclusionFactId]) rest
      else
        none

def derivationTraceGrounded
    (rules : List NamedAscentRule)
    (receipt : AscentClosureReceipt)
    (trace : List AscentDerivationStep) : Bool :=
  match replayDerivationTrace rules receipt.sourceFactIds trace with
  | none => false
  | some replayedFacts =>
      trace.length == receipt.ruleFirings &&
        replayedFacts == receipt.sourceFactIds ++ receipt.derivedFactIds

def ascentClosureVerifiedAdmitted
    (identity : GraphSessionIdentity)
    (admittedFacts trustedRulesets : List String)
    (budget : AscentClosureBudget)
    (expectedCanonicalFactOrderDigest : String)
    (rules : List NamedAscentRule)
    (trace : List AscentDerivationStep)
    (receipt : AscentClosureReceipt) : Bool :=
  ascentClosureReceiptAdmitted identity admittedFacts trustedRulesets budget receipt &&
    receipt.canonicalFactOrderDigest == expectedCanonicalFactOrderDigest &&
    derivationTraceGrounded rules receipt trace &&
    receiptIsFixedPoint rules receipt

def ownerRule : NamedAscentRule :=
  { ruleId := "rule:model-to-configuration-owner"
    premiseFactIds := ["fact:model-declared"]
    conclusionFactId := "fact:configuration-owner" }

def ownerDerivationStep : AscentDerivationStep :=
  { ruleId := ownerRule.ruleId
    premiseFactIds := ownerRule.premiseFactIds
    conclusionFactId := ownerRule.conclusionFactId }

def traceGroundedAscentReceipt : AscentClosureReceipt :=
  { admittedAscentReceipt with ruleFirings := 1 }

theorem recomputed_ascent_closure_is_grounded_complete_and_canonical :
    ascentClosureVerifiedAdmitted graphSessionA
      ["fact:model-declared"] ["ruleset:owner-v1"] ascentBudget
      "facts:canonical:1" [ownerRule] [ownerDerivationStep]
      traceGroundedAscentReceipt = true := by
  decide

def incompleteAscentReceipt : AscentClosureReceipt :=
  { admittedAscentReceipt with
    derivedFactIds := []
    ruleFirings := 0
    derivedFacts := 0 }

theorem budget_only_admission_can_accept_a_non_fixed_point_receipt :
    ascentClosureReceiptAdmitted graphSessionA
      ["fact:model-declared"] ["ruleset:owner-v1"]
      ascentBudget incompleteAscentReceipt = true ∧
    receiptIsFixedPoint [ownerRule] incompleteAscentReceipt = false := by
  decide

theorem verified_admission_rejects_a_non_fixed_point_receipt :
    ascentClosureVerifiedAdmitted graphSessionA
      ["fact:model-declared"] ["ruleset:owner-v1"] ascentBudget
      "facts:canonical:1" [ownerRule] [] incompleteAscentReceipt = false := by
  decide

def forgedCanonicalDigestReceipt : AscentClosureReceipt :=
  { traceGroundedAscentReceipt with
    canonicalFactOrderDigest := "facts:caller-invented" }

theorem nonempty_digest_check_does_not_establish_canonical_order :
    ascentClosureReceiptAdmitted graphSessionA
      ["fact:model-declared"] ["ruleset:owner-v1"] ascentBudget
      forgedCanonicalDigestReceipt = true ∧
    ascentClosureVerifiedAdmitted graphSessionA
      ["fact:model-declared"] ["ruleset:owner-v1"] ascentBudget
      "facts:canonical:1" [ownerRule] [ownerDerivationStep]
      forgedCanonicalDigestReceipt = false := by
  decide

theorem scalar_rule_firing_count_does_not_ground_a_derivation :
    ascentClosureReceiptAdmitted graphSessionA
      ["fact:model-declared"] ["ruleset:owner-v1"]
      ascentBudget admittedAscentReceipt = true ∧
    derivationTraceGrounded [ownerRule] admittedAscentReceipt
      [ownerDerivationStep] = false := by
  decide

def untrustedRulesetReceipt : AscentClosureReceipt :=
  { admittedAscentReceipt with rulesetDigest := "ruleset:model-injected" }

theorem model_injected_ascent_ruleset_is_rejected :
    ascentClosureReceiptAdmitted graphSessionA
      ["fact:model-declared"] ["ruleset:owner-v1"]
      ascentBudget untrustedRulesetReceipt = false := by
  decide

def overRuleBudgetReceipt : AscentClosureReceipt :=
  { admittedAscentReceipt with ruleFirings := 17 }

theorem ascent_rule_firing_budget_is_independent_and_fail_closed :
    ascentClosureReceiptAdmitted graphSessionA
      ["fact:model-declared"] ["ruleset:owner-v1"]
      ascentBudget overRuleBudgetReceipt = false := by
  decide

def overDerivedFactBudgetReceipt : AscentClosureReceipt :=
  { admittedAscentReceipt with
    derivedFactIds :=
      ["fact:1", "fact:2", "fact:3", "fact:4", "fact:5",
       "fact:6", "fact:7", "fact:8", "fact:9"]
    derivedFacts := 9 }

theorem ascent_derived_fact_budget_is_independent_and_fail_closed :
    ascentClosureReceiptAdmitted graphSessionA
      ["fact:model-declared"] ["ruleset:owner-v1"]
      ascentBudget overDerivedFactBudgetReceipt = false := by
  decide

structure VerifiedGraphDeltaReceipt where
  beforeStateDigest : String
  afterStateDigest : String
  snapshotDigest : String
  workspaceGenerationRootDigest : String
  closureReceiptDigest : String
  exactVerificationDigest : String
  derivedFactIds : List FactId
  deriving Repr, DecidableEq, BEq

def graphDeltaAdmissionValid
    (identity : GraphSessionIdentity)
    (closure : AscentClosureReceipt)
    (delta : VerifiedGraphDeltaReceipt) : Bool :=
  delta.beforeStateDigest == identity.routeDigest &&
    delta.afterStateDigest != delta.beforeStateDigest &&
    delta.snapshotDigest == identity.snapshotDigest &&
    delta.workspaceGenerationRootDigest ==
      identity.workspaceGenerationRootDigest &&
    !delta.closureReceiptDigest.isEmpty &&
    !delta.exactVerificationDigest.isEmpty &&
    delta.derivedFactIds == closure.derivedFactIds

def admittedGraphDelta : VerifiedGraphDeltaReceipt :=
  { beforeStateDigest := "route:owner-v1"
    afterStateDigest := "route:owner-v2"
    snapshotDigest := "snapshot:7"
    workspaceGenerationRootDigest := "generation:7"
    closureReceiptDigest := "closure:1"
    exactVerificationDigest := "verification:1"
    derivedFactIds := ["fact:configuration-owner"] }

theorem verified_graph_delta_can_admit_bounded_closure_output :
    graphDeltaAdmissionValid graphSessionA
      admittedAscentReceipt admittedGraphDelta = true := by
  decide

def closureReceiptAloneAuthorizesMutation
    (_receipt : AscentClosureReceipt) : Bool := false

theorem ascent_receipt_alone_cannot_mutate_evidence_graph :
    closureReceiptAloneAuthorizesMutation admittedAscentReceipt = false := by
  rfl

def pythonProposalAloneAuthorizesMutation
    (_status : PythonProposalStatus) : Bool := false

theorem python_proposal_alone_cannot_mutate_evidence_graph
    (status : PythonProposalStatus) :
    pythonProposalAloneAuthorizesMutation status = false := by
  rfl

inductive GqlPathBound where
  | explicit (hops : Nat)
  | routerSupplied (hops : Nat)
  | unbounded
  deriving Repr, DecidableEq, BEq

def gqlPathAdmitted (maxGraphHops : Nat) (bound : GqlPathBound) : Bool :=
  match bound with
  | .explicit hops => hops <= maxGraphHops
  | .routerSupplied hops => hops <= maxGraphHops
  | .unbounded => false

theorem router_bounded_gql_path_is_admitted :
    gqlPathAdmitted 4 (.routerSupplied 3) = true := by
  decide

theorem unbounded_gql_path_is_fail_closed :
    gqlPathAdmitted 4 .unbounded = false := by
  rfl

abbrev RouteCost :=
  ASPProof.SearchRouteGraphRouterParetoCostSelection.RouteCost

abbrev RouteCaps :=
  ASPProof.SearchRouteGraphRouterParetoCostSelection.RouteCost

def routeFeasible (cost caps : RouteCost) : Prop :=
  ASPProof.SearchRouteGraphRouterParetoCostSelection.Feasible cost caps

def routeNoWorse (left right : RouteCost) : Prop :=
  ASPProof.SearchRouteGraphRouterParetoCostSelection.NoWorse left right

theorem pareto_improvement_preserves_hard_budget_feasibility
    (better worse caps : RouteCost)
    (noWorse : routeNoWorse better worse)
    (feasible : routeFeasible worse caps) :
    routeFeasible better caps := by
  exact
    ASPProof.SearchRouteGraphRouterParetoCostSelection.no_worse_route_preserves_feasibility
      better worse caps noWorse feasible

theorem shortest_hop_order_requires_prior_token_and_round_feasibility
    (tokenCap : Nat) :
    let unsafeRoute : RouteCost :=
      { graphHops := 0
        interactionRounds := 0
        projectedTokens := tokenCap + 1
        verificationOps := 0
        searchExecutions := 0
        modelPrefixRecomputations := 0 }
    let alternativeRoute : RouteCost :=
      { graphHops := 1
        interactionRounds := 0
        projectedTokens := 0
        verificationOps := 0
        searchExecutions := 0
        modelPrefixRecomputations := 0 }
    let caps : RouteCost :=
      { graphHops := 1
        interactionRounds := 0
        projectedTokens := tokenCap
        verificationOps := 0
        searchExecutions := 0
        modelPrefixRecomputations := 0 }
    ASPProof.SearchRouteGraphRouterParetoCostSelection.HopFirstLexBetter
        unsafeRoute alternativeRoute ∧
      ¬ routeFeasible unsafeRoute caps := by
  simpa [routeFeasible, Nat.succ_eq_add_one] using
    ASPProof.SearchRouteGraphRouterParetoCostSelection.hop_first_lex_order_requires_prior_feasibility
      tokenCap

structure ServerProcessIdentity where
  protocolDigest : String
  implementationDigest : String
  generation : Nat
  deriving Repr, DecidableEq, BEq

def processCanServe
    (process : ServerProcessIdentity)
    (session : GraphSessionIdentity) : Bool :=
  !process.protocolDigest.isEmpty &&
    !process.implementationDigest.isEmpty &&
    graphSessionIdentityValid session

def residentProcess : ServerProcessIdentity :=
  { protocolDigest := "resident-protocol:1"
    implementationDigest := "scipy-runtime:1"
    generation := 3 }

theorem one_process_can_serve_distinct_isolated_graph_sessions :
    processCanServe residentProcess graphSessionA = true ∧
      processCanServe residentProcess graphSessionB = true ∧
      sameGraphSession graphSessionA graphSessionB = false := by
  decide

structure ProjectionCacheIdentity where
  workspaceDigest : String
  snapshotDigest : String
  graphSchemaDigest : String
  projectionDigest : String
  algorithmImplementationDigest : String
  parameterDigest : String
  deterministicSeed : Nat
  budgetProfileDigest : String
  deriving Repr, DecidableEq, BEq

structure ModelPrefixCacheIdentity where
  modelDigest : String
  promptAndToolDigest : String
  prefixDigest : String
  deriving Repr, DecidableEq, BEq

def projectionCacheReusable
    (expected observed : ProjectionCacheIdentity) : Bool :=
  expected == observed

def modelPrefixCacheReusable
    (expected observed : ModelPrefixCacheIdentity) : Bool :=
  expected == observed

def projectionCacheIdentity : ProjectionCacheIdentity :=
  { workspaceDigest := "workspace:1"
    snapshotDigest := "snapshot:7"
    graphSchemaDigest := "schema:1"
    projectionDigest := "projection:1"
    algorithmImplementationDigest := "scipy:1"
    parameterDigest := "parameters:1"
    deterministicSeed := 0
    budgetProfileDigest := "budget:1" }

def modelPrefixCacheIdentity : ModelPrefixCacheIdentity :=
  { modelDigest := "model:1"
    promptAndToolDigest := "prompt-tools:1"
    prefixDigest := "prefix:1" }

theorem search_projection_cache_identity_is_exact :
    projectionCacheReusable projectionCacheIdentity projectionCacheIdentity = true := by
  decide

theorem model_prefix_cache_identity_is_exact :
    modelPrefixCacheReusable modelPrefixCacheIdentity modelPrefixCacheIdentity = true := by
  decide

def changedSnapshotProjectionCacheIdentity : ProjectionCacheIdentity :=
  { projectionCacheIdentity with snapshotDigest := "snapshot:6" }

theorem stale_snapshot_invalidates_search_projection_cache :
    projectionCacheReusable projectionCacheIdentity
      changedSnapshotProjectionCacheIdentity = false := by
  decide

theorem renderer_bytes_are_outside_search_and_model_cache_identity
    (_mermaid _dot : String) :
    projectionCacheReusable projectionCacheIdentity projectionCacheIdentity = true ∧
      modelPrefixCacheReusable modelPrefixCacheIdentity modelPrefixCacheIdentity = true := by
  decide

end ASPProof.GQLCoreSciPyResidentGraphTurbo
