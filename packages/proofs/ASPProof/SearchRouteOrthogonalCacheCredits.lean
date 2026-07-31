import Lean.Elab.Tactic.Omega

namespace ASPProof.SearchRouteOrthogonalCacheCredits

structure RouteCost where
  graphHops : Nat
  toolRounds : Nat
  searchTokens : Nat
  uncachedModelTokens : Nat
  deriving DecidableEq, Repr

structure SearchCacheCredit where
  graphHops : Nat
  toolRounds : Nat
  searchTokens : Nat
  deriving DecidableEq, Repr

structure ModelPrefixCredit where
  uncachedModelTokens : Nat
  deriving DecidableEq, Repr

def CreditsWithinPlan
    (planned : RouteCost)
    (search : SearchCacheCredit)
    (model : ModelPrefixCredit) : Prop :=
  search.graphHops ≤ planned.graphHops
    ∧ search.toolRounds ≤ planned.toolRounds
    ∧ search.searchTokens ≤ planned.searchTokens
    ∧ model.uncachedModelTokens ≤ planned.uncachedModelTokens

def applyCacheCredits
    (planned : RouteCost)
    (search : SearchCacheCredit)
    (model : ModelPrefixCredit) : RouteCost :=
  { graphHops := planned.graphHops - search.graphHops
  , toolRounds := planned.toolRounds - search.toolRounds
  , searchTokens := planned.searchTokens - search.searchTokens
  , uncachedModelTokens :=
      planned.uncachedModelTokens - model.uncachedModelTokens
  }

def CostNoWorse (left right : RouteCost) : Prop :=
  left.graphHops ≤ right.graphHops
    ∧ left.toolRounds ≤ right.toolRounds
    ∧ left.searchTokens ≤ right.searchTokens
    ∧ left.uncachedModelTokens ≤ right.uncachedModelTokens

theorem cache_credit_conservation
    (planned : RouteCost)
    (search : SearchCacheCredit)
    (model : ModelPrefixCredit)
    (valid : CreditsWithinPlan planned search model) :
    let realized := applyCacheCredits planned search model
    realized.graphHops + search.graphHops = planned.graphHops
      ∧ realized.toolRounds + search.toolRounds = planned.toolRounds
      ∧ realized.searchTokens + search.searchTokens = planned.searchTokens
      ∧ realized.uncachedModelTokens + model.uncachedModelTokens
          = planned.uncachedModelTokens := by
  simp [CreditsWithinPlan, applyCacheCredits] at valid ⊢
  omega

theorem cache_realized_cost_is_no_worse
    (planned : RouteCost)
    (search : SearchCacheCredit)
    (model : ModelPrefixCredit) :
    CostNoWorse (applyCacheCredits planned search model) planned := by
  simp [CostNoWorse, applyCacheCredits]

def zeroSearchCredit : SearchCacheCredit :=
  ⟨0, 0, 0⟩

def zeroModelCredit : ModelPrefixCredit :=
  ⟨0⟩

theorem search_credit_is_model_orthogonal
    (planned : RouteCost)
    (search : SearchCacheCredit)
    (model : ModelPrefixCredit) :
    (applyCacheCredits planned search model).uncachedModelTokens
      = (applyCacheCredits planned zeroSearchCredit model).uncachedModelTokens := by
  rfl

theorem model_credit_is_search_orthogonal
    (planned : RouteCost)
    (search : SearchCacheCredit)
    (model : ModelPrefixCredit) :
    let withModel := applyCacheCredits planned search model
    let withoutModel := applyCacheCredits planned search zeroModelCredit
    withModel.graphHops = withoutModel.graphHops
      ∧ withModel.toolRounds = withoutModel.toolRounds
      ∧ withModel.searchTokens = withoutModel.searchTokens := by
  simp [applyCacheCredits, zeroModelCredit]

structure CompletionKey where
  generation : Nat
  coverageDigest : Nat
  modeDigest : Nat
  deriving DecidableEq, Repr

structure RoutePlan where
  initialPotential : Nat
  finalPotential : Nat
  completionKey : CompletionKey
  cost : RouteCost
  deriving DecidableEq, Repr

def applyCacheToRoute
    (route : RoutePlan)
    (search : SearchCacheCredit)
    (model : ModelPrefixCredit) : RoutePlan :=
  { route with cost := applyCacheCredits route.cost search model }

theorem cache_application_preserves_semantics
    (route : RoutePlan)
    (search : SearchCacheCredit)
    (model : ModelPrefixCredit) :
    let realized := applyCacheToRoute route search model
    realized.initialPotential = route.initialPotential
      ∧ realized.finalPotential = route.finalPotential
      ∧ realized.completionKey = route.completionKey := by
  simp [applyCacheToRoute]

structure CacheContext where
  graphGeneration : Nat
  requestDigest : Nat
  executionId : Nat
  dependencyDigest : Nat
  prefixDigest : Nat
  modelDigest : Nat
  deriving DecidableEq, Repr

structure SearchCacheReceipt where
  graphGeneration : Nat
  requestDigest : Nat
  executionId : Nat
  dependencyDigest : Nat
  credit : SearchCacheCredit
  deriving DecidableEq, Repr

structure ModelPrefixReceipt where
  requestDigest : Nat
  executionId : Nat
  prefixDigest : Nat
  modelDigest : Nat
  credit : ModelPrefixCredit
  deriving DecidableEq, Repr

def SearchReceiptBound
    (context : CacheContext)
    (receipt : SearchCacheReceipt) : Prop :=
  receipt.graphGeneration = context.graphGeneration
    ∧ receipt.requestDigest = context.requestDigest
    ∧ receipt.executionId = context.executionId
    ∧ receipt.dependencyDigest = context.dependencyDigest

def ModelPrefixReceiptBound
    (context : CacheContext)
    (receipt : ModelPrefixReceipt) : Prop :=
  receipt.requestDigest = context.requestDigest
    ∧ receipt.executionId = context.executionId
    ∧ receipt.prefixDigest = context.prefixDigest
    ∧ receipt.modelDigest = context.modelDigest

theorem model_prefix_binding_ignores_graph_generation
    (context : CacheContext)
    (receipt : ModelPrefixReceipt)
    (newGeneration : Nat)
    (bound : ModelPrefixReceiptBound context receipt) :
    ModelPrefixReceiptBound
      { context with graphGeneration := newGeneration }
      receipt := by
  exact bound

inductive CreditKind
  | searchResult
  | modelPrefix
  deriving DecidableEq, Repr

inductive CreditStatus
  | pending
  | applied
  deriving DecidableEq, Repr

structure CreditRecord where
  receiptDigest : Nat
  executionId : Nat
  kind : CreditKind
  revision : Nat
  status : CreditStatus
  deriving DecidableEq, Repr

structure CreditProposal where
  receiptDigest : Nat
  executionId : Nat
  kind : CreditKind
  expectedRevision : Nat
  deriving DecidableEq, Repr

def CanCommitCredit
    (record : CreditRecord)
    (proposal : CreditProposal) : Prop :=
  proposal.receiptDigest = record.receiptDigest
    ∧ proposal.executionId = record.executionId
    ∧ proposal.kind = record.kind
    ∧ proposal.expectedRevision = record.revision
    ∧ record.status = .pending

def commitCredit
    (record : CreditRecord)
    (_proposal : CreditProposal) : CreditRecord :=
  { record with
    revision := record.revision + 1
    status := .applied }

theorem committed_credit_rejects_replay
    (record : CreditRecord)
    (proposal : CreditProposal) :
    ¬ CanCommitCredit (commitCredit record proposal) proposal := by
  simp [CanCommitCredit, commitCredit]

def examplePlanned : RouteCost :=
  ⟨4, 2, 100, 1000⟩

def exampleSearchCredit : SearchCacheCredit :=
  ⟨2, 1, 80⟩

def exampleModelCredit : ModelPrefixCredit :=
  ⟨800⟩

def currentContext : CacheContext :=
  ⟨8, 11, 20, 42, 99, 7⟩

def nextGraphGenerationContext : CacheContext :=
  { currentContext with graphGeneration := 9 }

def exampleSearchReceipt : SearchCacheReceipt :=
  ⟨8, 11, 20, 42, exampleSearchCredit⟩

def exampleModelReceipt : ModelPrefixReceipt :=
  ⟨11, 20, 99, 7, exampleModelCredit⟩

theorem example_credits_are_balanced :
    CreditsWithinPlan
      examplePlanned exampleSearchCredit exampleModelCredit := by
  simp
    [ CreditsWithinPlan
    , examplePlanned
    , exampleSearchCredit
    , exampleModelCredit
    ]

theorem stale_search_receipt_is_rejected :
    ¬ SearchReceiptBound
      nextGraphGenerationContext exampleSearchReceipt := by
  simp
    [ SearchReceiptBound
    , nextGraphGenerationContext
    , currentContext
    , exampleSearchReceipt
    ]

theorem model_prefix_receipt_survives_unrelated_graph_change :
    ModelPrefixReceiptBound
      nextGraphGenerationContext exampleModelReceipt := by
  apply model_prefix_binding_ignores_graph_generation
    currentContext exampleModelReceipt 9
  simp
    [ ModelPrefixReceiptBound
    , currentContext
    , exampleModelReceipt
    ]

end ASPProof.SearchRouteOrthogonalCacheCredits

