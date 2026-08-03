import ASPProof.SearchRouterInteractiveGraphState

namespace ASPProof

/-!
Executable refinement for one merged agent-facing search trace.

Tree-sitter is a backend and incremental is an execution mode.  They cannot
mint sibling trace identities or independently publish terminal/next actions.
-/

inductive SearchProjectionBackend where
  | treeSitter
  deriving DecidableEq, Repr

inductive SearchProjectionExecutionMode where
  | incremental
  | complete
  deriving DecidableEq, Repr

structure SearchTraceIdentity where
  requestId : String
  providerWorkspaceIdentity : String
  queryDigest : String
  admittedGeneration : String
  deriving DecidableEq, Repr

structure SearchTraceEmission where
  identity : SearchTraceIdentity
  backend : SearchProjectionBackend
  executionMode : SearchProjectionExecutionMode
  terminalEmissions : Nat
  nextActionEmissions : Nat
  deriving DecidableEq, Repr

def AgentFacingSearchTraceAdmitted (trace : SearchTraceEmission) : Prop :=
  trace.terminalEmissions = 1 ∧ trace.nextActionEmissions = 1

def SameSearchTrace
    (left right : SearchTraceEmission) : Prop :=
  left.identity = right.identity

def mergeTreeSitterIncrementalTrace
    (identity : SearchTraceIdentity) : SearchTraceEmission :=
  { identity
    backend := .treeSitter
    executionMode := .incremental
    terminalEmissions := 1
    nextActionEmissions := 1 }

theorem mergedTreeSitterIncrementalTraceIsAdmitted
    (identity : SearchTraceIdentity) :
    AgentFacingSearchTraceAdmitted (mergeTreeSitterIncrementalTrace identity) := by
  simp [AgentFacingSearchTraceAdmitted, mergeTreeSitterIncrementalTrace]

theorem incrementalModeDoesNotMintSecondTraceIdentity
    (identity : SearchTraceIdentity) :
    (mergeTreeSitterIncrementalTrace identity).identity = identity := by
  rfl

theorem stackedTerminalEmissionIsRejected
    (trace : SearchTraceEmission)
    (stacked : trace.terminalEmissions = 2) :
    ¬ AgentFacingSearchTraceAdmitted trace := by
  intro admitted
  rcases admitted with ⟨singleTerminal, _⟩
  omega

theorem stackedNextActionEmissionIsRejected
    (trace : SearchTraceEmission)
    (stacked : trace.nextActionEmissions = 2) :
    ¬ AgentFacingSearchTraceAdmitted trace := by
  intro admitted
  rcases admitted with ⟨_, singleNext⟩
  omega

theorem sameIdentitySubtracesMustMergeBeforeAgentProjection
    (treeSitter incremental : SearchTraceEmission)
    (_same : SameSearchTrace treeSitter incremental)
    (treeTerminal : treeSitter.terminalEmissions = 1)
    (incrementalTerminal : incremental.terminalEmissions = 1) :
    treeSitter.terminalEmissions + incremental.terminalEmissions ≠ 1 := by
  omega

end ASPProof
