namespace ASPProof.PolyglotSearchQualification

inductive CacheMode where
  | cold
  | warm
  deriving DecidableEq

structure Thresholds where
  maxTokenRatioBps : Nat
  maxGraphHopRatioBps : Nat
  maxTurnRatioBps : Nat
  maxLatencyRatioBps : Nat
  maxPrecisionRegressionBps : Nat
  maxRecallRegressionBps : Nat
  maxClosureRegressionBps : Nat
  maxAnswerQualityRegressionBps : Nat
  deriving DecidableEq

structure Observation where
  totalTokens : Nat
  graphHops : Nat
  interactionTurns : Nat
  latencyMicros : Nat
  cacheHit : Bool
  precisionBps : Nat
  recallBps : Nat
  closureBps : Nat
  answerQualityBps : Nat
  stateDigestBefore : Nat
  stateDigestAfter : Nat
  deriving DecidableEq

structure Scenario where
  cacheMode : CacheMode
  legacy : Observation
  candidate : Observation
  deriving DecidableEq

def referenceThresholds : Thresholds where
  maxTokenRatioBps := 7000
  maxGraphHopRatioBps := 7500
  maxTurnRatioBps := 7500
  maxLatencyRatioBps := 10000
  maxPrecisionRegressionBps := 200
  maxRecallRegressionBps := 200
  maxClosureRegressionBps := 100
  maxAnswerQualityRegressionBps := 100

def ratioAdmitted (candidate legacy ratioBps : Nat) : Prop :=
  candidate * 10000 ≤ legacy * ratioBps

def qualityAdmitted (candidate legacy allowanceBps : Nat) : Prop :=
  legacy ≤ candidate + allowanceBps

def cacheAdmitted (mode : CacheMode) (hit : Bool) : Prop :=
  match mode with
  | .cold => hit = false
  | .warm => hit = true

def observationReadOnly (observation : Observation) : Prop :=
  observation.stateDigestBefore = observation.stateDigestAfter

def scenarioAdmittedB (thresholds : Thresholds) (scenario : Scenario) : Bool :=
  decide (scenario.candidate.totalTokens * 10000 ≤
      scenario.legacy.totalTokens * thresholds.maxTokenRatioBps) &&
  decide (scenario.candidate.graphHops * 10000 ≤
      scenario.legacy.graphHops * thresholds.maxGraphHopRatioBps) &&
  decide (scenario.candidate.interactionTurns * 10000 ≤
      scenario.legacy.interactionTurns * thresholds.maxTurnRatioBps) &&
  decide (scenario.candidate.latencyMicros * 10000 ≤
      scenario.legacy.latencyMicros * thresholds.maxLatencyRatioBps) &&
  decide (scenario.legacy.precisionBps ≤
      scenario.candidate.precisionBps + thresholds.maxPrecisionRegressionBps) &&
  decide (scenario.legacy.recallBps ≤
      scenario.candidate.recallBps + thresholds.maxRecallRegressionBps) &&
  decide (scenario.legacy.closureBps ≤
      scenario.candidate.closureBps + thresholds.maxClosureRegressionBps) &&
  decide (scenario.legacy.answerQualityBps ≤
      scenario.candidate.answerQualityBps + thresholds.maxAnswerQualityRegressionBps) &&
  decide (scenario.legacy.stateDigestBefore = scenario.legacy.stateDigestAfter) &&
  decide (scenario.candidate.stateDigestBefore = scenario.candidate.stateDigestAfter) &&
  match scenario.cacheMode with
  | .cold => scenario.candidate.cacheHit == false
  | .warm => scenario.candidate.cacheHit == true

def scenarioAdmitted (thresholds : Thresholds) (scenario : Scenario) : Prop :=
  scenarioAdmittedB thresholds scenario = true

instance (thresholds : Thresholds) (scenario : Scenario) :
    Decidable (scenarioAdmitted thresholds scenario) := by
  unfold scenarioAdmitted
  infer_instance

def hasCacheMode (mode : CacheMode) (scenarios : List Scenario) : Bool :=
  scenarios.any fun scenario => scenario.cacheMode == mode

def totalLegacyTokens (scenarios : List Scenario) : Nat :=
  scenarios.foldl (fun total scenario => total + scenario.legacy.totalTokens) 0

def totalCandidateTokens (scenarios : List Scenario) : Nat :=
  scenarios.foldl (fun total scenario => total + scenario.candidate.totalTokens) 0

def totalLegacyHops (scenarios : List Scenario) : Nat :=
  scenarios.foldl (fun total scenario => total + scenario.legacy.graphHops) 0

def totalCandidateHops (scenarios : List Scenario) : Nat :=
  scenarios.foldl (fun total scenario => total + scenario.candidate.graphHops) 0

def totalLegacyTurns (scenarios : List Scenario) : Nat :=
  scenarios.foldl (fun total scenario => total + scenario.legacy.interactionTurns) 0

def totalCandidateTurns (scenarios : List Scenario) : Nat :=
  scenarios.foldl (fun total scenario => total + scenario.candidate.interactionTurns) 0

def qualificationAdmitted (thresholds : Thresholds) (scenarios : List Scenario) : Prop :=
  scenarios ≠ [] ∧
  hasCacheMode .cold scenarios = true ∧
  hasCacheMode .warm scenarios = true ∧
  scenarios.all (scenarioAdmittedB thresholds) = true ∧
  totalCandidateTokens scenarios < totalLegacyTokens scenarios ∧
  totalCandidateHops scenarios < totalLegacyHops scenarios ∧
  totalCandidateTurns scenarios < totalLegacyTurns scenarios

instance (thresholds : Thresholds) (scenarios : List Scenario) :
    Decidable (qualificationAdmitted thresholds scenarios) := by
  unfold qualificationAdmitted
  infer_instance

def observation
    (tokens hops turns latency : Nat) (cacheHit : Bool)
    (precision recall closure quality state : Nat) : Observation where
  totalTokens := tokens
  graphHops := hops
  interactionTurns := turns
  latencyMicros := latency
  cacheHit := cacheHit
  precisionBps := precision
  recallBps := recall
  closureBps := closure
  answerQualityBps := quality
  stateDigestBefore := state
  stateDigestAfter := state

def coldPath : Scenario where
  cacheMode := .cold
  legacy := observation 1000 5 4 120000 false 10000 10000 10000 10000 1
  candidate := observation 650 3 3 100000 false 10000 10000 10000 10000 1

def warmAlias : Scenario where
  cacheMode := .warm
  legacy := observation 900 6 4 90000 true 9900 9900 10000 9900 2
  candidate := observation 550 3 2 60000 true 9900 9900 10000 9900 2

def warmLogic : Scenario where
  cacheMode := .warm
  legacy := observation 1200 8 5 150000 true 9800 10000 9900 9900 3
  candidate := observation 700 5 3 110000 true 9800 10000 9900 9900 3

def warmParallelWitness : Scenario where
  cacheMode := .warm
  legacy := observation 800 5 3 80000 true 10000 10000 10000 10000 4
  candidate := observation 500 3 2 70000 true 10000 10000 10000 10000 4

def referenceScenarios : List Scenario :=
  [coldPath, warmAlias, warmLogic, warmParallelWitness]

def averageOnlyCounterexample : Scenario where
  cacheMode := .cold
  legacy := observation 100 10 4 100 true 10000 10000 10000 10000 5
  candidate := observation 80 7 3 100 false 10000 10000 10000 10000 5

def qualityRegressionCounterexample : Scenario where
  cacheMode := .warm
  legacy := observation 1000 10 4 100 true 10000 10000 10000 10000 6
  candidate := observation 500 5 2 50 true 10000 9000 10000 10000 6

def stateMutationCounterexample : Scenario where
  cacheMode := .warm
  legacy := observation 1000 10 4 100 true 10000 10000 10000 10000 7
  candidate :=
    { observation 500 5 2 50 true 10000 10000 10000 10000 7 with
      stateDigestAfter := 8 }

theorem reference_qualification_is_admitted :
    qualificationAdmitted referenceThresholds referenceScenarios := by
  decide

theorem qualification_exposes_every_scenario_gate
    {thresholds : Thresholds} {scenarios : List Scenario}
    (h : qualificationAdmitted thresholds scenarios) :
    scenarios.all (scenarioAdmittedB thresholds) = true := by
  exact h.2.2.2.1

theorem aggregate_token_improvement_does_not_imply_scenario_admission :
    totalCandidateTokens [averageOnlyCounterexample] <
        totalLegacyTokens [averageOnlyCounterexample] ∧
      ¬scenarioAdmitted referenceThresholds averageOnlyCounterexample := by
  decide

theorem quality_regression_is_rejected :
    ¬scenarioAdmitted referenceThresholds qualityRegressionCounterexample := by
  decide

theorem state_mutation_is_rejected :
    ¬scenarioAdmitted referenceThresholds stateMutationCounterexample := by
  decide

theorem cold_and_warm_coverage_are_both_required
    {thresholds : Thresholds} {scenarios : List Scenario}
    (h : qualificationAdmitted thresholds scenarios) :
    hasCacheMode .cold scenarios = true ∧
      hasCacheMode .warm scenarios = true := by
  exact ⟨h.2.1, h.2.2.1⟩

end ASPProof.PolyglotSearchQualification
