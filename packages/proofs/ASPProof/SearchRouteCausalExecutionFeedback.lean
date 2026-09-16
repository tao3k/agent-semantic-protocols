-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteProofCarryingStateMachine

namespace ASPProof.SearchRouteCausalExecutionFeedback

open ASPProof.SearchRouteProofCarryingStateMachine

structure AdmissionSnapshot where
  admissionDigest : Nat
  contextDigest : Nat
  requestDigest : Nat
  plannedTokens : Nat
  tokenBudget : Nat
  completionMode : CompletionMode
  deriving DecidableEq, Repr

def AdmissionFeasible (admission : AdmissionSnapshot) : Prop :=
  admission.plannedTokens ≤ admission.tokenBudget

structure OutcomeReceipt where
  admissionDigest : Nat
  requestDigest : Nat
  observedTokens : Nat
  outcome : ExecutionOutcome
  deriving DecidableEq, Repr

def ReceiptBound
    (admission : AdmissionSnapshot)
    (receipt : OutcomeReceipt) : Prop :=
  receipt.admissionDigest = admission.admissionDigest ∧
  receipt.requestDigest = admission.requestDigest

def ExactAdmission (admission : AdmissionSnapshot) : Prop :=
  admission.completionMode = .exact

structure ProblemGeneration where
  generation : Nat
  contextDigest : Nat
  deriving DecidableEq, Repr

structure FeedbackGeneration where
  prior : ProblemGeneration
  admission : AdmissionSnapshot
  next : ProblemGeneration
  receipt : OutcomeReceipt
  receiptBound : ReceiptBound admission receipt
  advances : next.generation = prior.generation + 1

def applyFeedback
    (prior : ProblemGeneration)
    (admission : AdmissionSnapshot)
    (receipt : OutcomeReceipt)
    (receiptBound : ReceiptBound admission receipt) :
    FeedbackGeneration :=
  {
    prior := prior
    admission := admission
    next := {
      generation := prior.generation + 1
      contextDigest :=
        prior.contextDigest + receipt.admissionDigest + receipt.requestDigest
    }
    receipt := receipt
    receiptBound := receiptBound
    advances := rfl
  }

theorem feedback_strictly_advances_generation
    (prior : ProblemGeneration)
    (admission : AdmissionSnapshot)
    (receipt : OutcomeReceipt)
    (receiptBound : ReceiptBound admission receipt) :
    prior.generation <
      (applyFeedback prior admission receipt receiptBound).next.generation := by
  exact Nat.lt_succ_self prior.generation

theorem feedback_preserves_prior_problem
    (prior : ProblemGeneration)
    (admission : AdmissionSnapshot)
    (receipt : OutcomeReceipt)
    (receiptBound : ReceiptBound admission receipt) :
    (applyFeedback prior admission receipt receiptBound).prior = prior := by
  rfl

theorem feedback_preserves_admission_snapshot
    (prior : ProblemGeneration)
    (admission : AdmissionSnapshot)
    (receipt : OutcomeReceipt)
    (receiptBound : ReceiptBound admission receipt) :
    (applyFeedback prior admission receipt receiptBound).admission =
      admission := by
  rfl

def feasibleAdmission : AdmissionSnapshot :=
  {
    admissionDigest := 81
    contextDigest := 71
    requestDigest := 91
    plannedTokens := 15
    tokenBudget := 20
    completionMode := .incomplete
  }

def infeasibleHypothesis : AdmissionSnapshot :=
  {
    admissionDigest := 82
    contextDigest := 71
    requestDigest := 92
    plannedTokens := 100
    tokenBudget := 20
    completionMode := .incomplete
  }

def feasibleOutcome : OutcomeReceipt :=
  {
    admissionDigest := 81
    requestDigest := 91
    observedTokens := 10
    outcome := .succeeded
  }

def infeasibleHypotheticalOutcome : OutcomeReceipt :=
  {
    admissionDigest := 82
    requestDigest := 92
    observedTokens := 10
    outcome := .succeeded
  }

def mismatchedOutcome : OutcomeReceipt :=
  { feasibleOutcome with requestDigest := 999 }

theorem feasible_receipt_is_bound :
    ReceiptBound feasibleAdmission feasibleOutcome := by
  unfold ReceiptBound feasibleAdmission feasibleOutcome
  decide

theorem mismatched_receipt_is_rejected :
    ¬ ReceiptBound feasibleAdmission mismatchedOutcome := by
  unfold ReceiptBound feasibleAdmission mismatchedOutcome feasibleOutcome
  decide

theorem equal_observed_cost_does_not_determine_admission_feasibility :
    feasibleOutcome.observedTokens =
        infeasibleHypotheticalOutcome.observedTokens ∧
      AdmissionFeasible feasibleAdmission ∧
      ¬ AdmissionFeasible infeasibleHypothesis := by
  unfold AdmissionFeasible feasibleAdmission infeasibleHypothesis
    feasibleOutcome infeasibleHypotheticalOutcome
  decide

theorem successful_incomplete_execution_is_not_exact :
    feasibleOutcome.outcome = .succeeded ∧
      ¬ ExactAdmission feasibleAdmission := by
  unfold ExactAdmission feasibleAdmission feasibleOutcome
  decide

def examplePriorGeneration : ProblemGeneration :=
  {
    generation := 7
    contextDigest := 71
  }

theorem example_feedback_advances_to_generation_eight :
    (applyFeedback examplePriorGeneration feasibleAdmission feasibleOutcome
      feasible_receipt_is_bound).next.generation = 8 := by
  unfold applyFeedback examplePriorGeneration
  decide

end ASPProof.SearchRouteCausalExecutionFeedback
