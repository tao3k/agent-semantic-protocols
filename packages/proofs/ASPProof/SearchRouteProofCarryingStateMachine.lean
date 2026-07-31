namespace ASPProof.SearchRouteProofCarryingStateMachine

structure SearchContext where
  digest : Nat
  deriving DecidableEq, Repr

inductive CertificateKind where
  | obligation
  | feasibility
  | ledger
  | costBound

structure VerifiedCertificateRef (kind : CertificateKind) where
  contextDigest : Nat
  claim : Prop
  verified : claim

structure AdmissionBundle where
  obligation : VerifiedCertificateRef .obligation
  feasibility : VerifiedCertificateRef .feasibility
  ledger : VerifiedCertificateRef .ledger
  costBound : VerifiedCertificateRef .costBound

def BundleCurrent
    (context : SearchContext)
    (bundle : AdmissionBundle) : Prop :=
  bundle.obligation.contextDigest = context.digest ∧
  bundle.feasibility.contextDigest = context.digest ∧
  bundle.ledger.contextDigest = context.digest ∧
  bundle.costBound.contextDigest = context.digest

inductive CompletionMode where
  | exact
  | bounded
  | incomplete
  deriving DecidableEq, Repr

inductive DecodedAction where
  | execute
  | boundedFailure
  | resetScope
  deriving DecidableEq, Repr

structure ExploreState where
  context : SearchContext
  ambiguity : Nat
  roundsRemaining : Nat
  transitionsRemaining : Nat

structure DecisionState where
  context : SearchContext
  action : DecodedAction
  completionMode : CompletionMode

structure AdmittedState where
  context : SearchContext
  action : DecodedAction
  completionMode : CompletionMode
  bundle : AdmissionBundle

def ExecutionReady (state : AdmittedState) : Prop :=
  state.action = .execute ∧
  BundleCurrent state.context state.bundle

structure ExecutingState where
  context : SearchContext
  completionMode : CompletionMode
  requestDigest : Nat

inductive ExecutionOutcome where
  | succeeded
  | failed
  deriving DecidableEq, Repr

structure CompletedState where
  context : SearchContext
  completionMode : CompletionMode
  requestDigest : Nat
  outcome : ExecutionOutcome

inductive SearchState where
  | exploring (state : ExploreState)
  | decisionReady (state : DecisionState)
  | admitted (state : AdmittedState)
  | executing (state : ExecutingState)
  | completed (state : CompletedState)

inductive LegalTransition : SearchState → SearchState → Prop where
  | refine
      (current next : ExploreState)
      (sameContext : current.context = next.context)
      (lessAmbiguity : next.ambiguity < current.ambiguity)
      (chargedRound :
        next.roundsRemaining < current.roundsRemaining)
      (chargedTransition :
        next.transitionsRemaining < current.transitionsRemaining) :
      LegalTransition (.exploring current) (.exploring next)
  | decide
      (current : ExploreState)
      (next : DecisionState)
      (resolved : current.ambiguity = 0)
      (sameContext : current.context = next.context) :
      LegalTransition (.exploring current) (.decisionReady next)
  | admit
      (current : DecisionState)
      (next : AdmittedState)
      (executeAction : current.action = .execute)
      (sameContext : current.context = next.context)
      (sameMode :
        current.completionMode = next.completionMode)
      (ready : ExecutionReady next) :
      LegalTransition (.decisionReady current) (.admitted next)
  | execute
      (current : AdmittedState)
      (next : ExecutingState)
      (ready : ExecutionReady current)
      (sameContext : current.context = next.context)
      (sameMode :
        current.completionMode = next.completionMode) :
      LegalTransition (.admitted current) (.executing next)
  | complete
      (current : ExecutingState)
      (next : CompletedState)
      (sameContext : current.context = next.context)
      (sameMode :
        current.completionMode = next.completionMode)
      (sameRequest :
        current.requestDigest = next.requestDigest) :
      LegalTransition (.executing current) (.completed next)
  | boundedFailure
      (current : DecisionState)
      (next : CompletedState)
      (failureAction : current.action = .boundedFailure)
      (sameContext : current.context = next.context)
      (sameMode :
        current.completionMode = next.completionMode)
      (noRequest : next.requestDigest = 0)
      (failed : next.outcome = .failed) :
      LegalTransition (.decisionReady current) (.completed next)

theorem no_direct_exploring_to_executing
    (explore : ExploreState)
    (executing : ExecutingState) :
    ¬ LegalTransition (.exploring explore) (.executing executing) := by
  intro transition
  cases transition

theorem no_direct_decision_to_executing
    (decision : DecisionState)
    (executing : ExecutingState) :
    ¬ LegalTransition
      (.decisionReady decision)
      (.executing executing) := by
  intro transition
  cases transition

theorem execution_transition_requires_ready
    (admitted : AdmittedState)
    (executing : ExecutingState)
    (transition :
      LegalTransition (.admitted admitted) (.executing executing)) :
    ExecutionReady admitted := by
  cases transition with
  | execute current next ready sameContext sameMode =>
      exact ready

theorem execution_transition_preserves_context
    (admitted : AdmittedState)
    (executing : ExecutingState)
    (transition :
      LegalTransition (.admitted admitted) (.executing executing)) :
    admitted.context = executing.context := by
  cases transition with
  | execute current next ready sameContext sameMode =>
      exact sameContext

theorem execution_transition_preserves_completion_mode
    (admitted : AdmittedState)
    (executing : ExecutingState)
    (transition :
      LegalTransition (.admitted admitted) (.executing executing)) :
    admitted.completionMode = executing.completionMode := by
  cases transition with
  | execute current next ready sameContext sameMode =>
      exact sameMode

def verifiedTrueAt
    (kind : CertificateKind)
    (contextDigest : Nat) :
    VerifiedCertificateRef kind :=
  {
    contextDigest := contextDigest
    claim := True
    verified := True.intro
  }

def exampleContext : SearchContext :=
  { digest := 71 }

def freshBundle : AdmissionBundle :=
  {
    obligation := verifiedTrueAt .obligation 71
    feasibility := verifiedTrueAt .feasibility 71
    ledger := verifiedTrueAt .ledger 71
    costBound := verifiedTrueAt .costBound 71
  }

def staleLedgerBundle : AdmissionBundle :=
  {
    obligation := verifiedTrueAt .obligation 71
    feasibility := verifiedTrueAt .feasibility 71
    ledger := verifiedTrueAt .ledger 70
    costBound := verifiedTrueAt .costBound 71
  }

def freshAdmitted : AdmittedState :=
  {
    context := exampleContext
    action := .execute
    completionMode := .bounded
    bundle := freshBundle
  }

def staleAdmitted : AdmittedState :=
  {
    context := exampleContext
    action := .execute
    completionMode := .bounded
    bundle := staleLedgerBundle
  }

theorem fresh_admission_is_execution_ready :
    ExecutionReady freshAdmitted := by
  decide

theorem stale_ledger_admission_is_not_execution_ready :
    ¬ ExecutionReady staleAdmitted := by
  decide

end ASPProof.SearchRouteProofCarryingStateMachine
