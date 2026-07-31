import ASPProof.SearchRouteForecastBoundParetoBatchScheduling

namespace ASPProof.SearchRouteAppendOnlyForecastCalibration

open ASPProof.SearchRouteCertifiedRegistryEpochTransition
open ASPProof.SearchRouteForecastBoundParetoBatchScheduling

structure CalibrationCost where
  disclosedTokenUnits : Nat
  latencyUnits : Nat
  llmRounds : Nat
  providerWorkUnits : Nat
  proofWorkUnits : Nat
  modelTokenWorkUnits : Nat
deriving DecidableEq, Repr

structure RegretVector where
  disclosedTokenError : Nat
  latencyError : Nat
  llmRoundError : Nat
  providerWorkError : Nat
  proofWorkError : Nat
  modelTokenWorkError : Nat
deriving DecidableEq, Repr

def absoluteDifference (left right : Nat) : Nat :=
  if left ≤ right then right - left else left - right

def regretVector
    (predicted realized : CalibrationCost) : RegretVector :=
  {
    disclosedTokenError :=
      absoluteDifference predicted.disclosedTokenUnits
        realized.disclosedTokenUnits
    latencyError :=
      absoluteDifference predicted.latencyUnits realized.latencyUnits
    llmRoundError :=
      absoluteDifference predicted.llmRounds realized.llmRounds
    providerWorkError :=
      absoluteDifference predicted.providerWorkUnits
        realized.providerWorkUnits
    proofWorkError :=
      absoluteDifference predicted.proofWorkUnits realized.proofWorkUnits
    modelTokenWorkError :=
      absoluteDifference predicted.modelTokenWorkUnits
        realized.modelTokenWorkUnits
  }

def totalRegret (regret : RegretVector) : Nat :=
  regret.disclosedTokenError +
    regret.latencyError +
    regret.llmRoundError +
    regret.providerWorkError +
    regret.proofWorkError +
    regret.modelTokenWorkError

structure ForecastSnapshot where
  version : Nat
  forecast : CacheForecast
deriving DecidableEq, Repr

structure CalibrationObservation where
  forecastVersion : Nat
  forecastDigest : Digest
  planDigest : Digest
  realizationDigest : Digest
  predicted : CalibrationCost
  realized : CalibrationCost
deriving DecidableEq, Repr

def observationRegret
    (observation : CalibrationObservation) : Nat :=
  totalRegret (regretVector observation.predicted observation.realized)

inductive CalibrationMode
  | normal
  | degraded
  | explore
deriving DecidableEq, Repr

def requiredMode (breachLimit consecutiveBreaches : Nat) :
    CalibrationMode :=
  if breachLimit ≤ consecutiveBreaches then
    .explore
  else if 0 < consecutiveBreaches then
    .degraded
  else
    .normal

structure CalibrationState where
  currentSnapshot : ForecastSnapshot
  observations : List CalibrationObservation
  sampleCount : Nat
  cumulativeRegret : Nat
  consecutiveBreaches : Nat
  mode : CalibrationMode
deriving DecidableEq, Repr

structure ValidCalibrationUpdate
    (threshold breachLimit : Nat)
    (before : CalibrationState)
    (observation : CalibrationObservation)
    (after : CalibrationState) : Prop where
  observationVersion :
    observation.forecastVersion = before.currentSnapshot.version
  observationDigest :
    observation.forecastDigest =
      before.currentSnapshot.forecast.forecastDigest
  appendOnly :
    after.observations = before.observations ++ [observation]
  versionAdvance :
    after.currentSnapshot.version =
      before.currentSnapshot.version + 1
  validNextForecast : ValidForecast after.currentSnapshot.forecast
  sampleAdvance : after.sampleCount = before.sampleCount + 1
  regretAdvance :
    after.cumulativeRegret =
      before.cumulativeRegret + observationRegret observation
  breachAdvance :
    after.consecutiveBreaches =
      if threshold < observationRegret observation then
        before.consecutiveBreaches + 1
      else
        0
  modeMatches :
    after.mode =
      requiredMode breachLimit after.consecutiveBreaches

structure HistoricalDecision where
  forecastVersion : Nat
  forecastDigest : Digest
  planDigest : Digest
  realizationDigest : Digest
deriving DecidableEq, Repr

def DecisionBoundToSnapshot
    (decision : HistoricalDecision)
    (snapshot : ForecastSnapshot) : Prop :=
  decision.forecastVersion = snapshot.version ∧
    decision.forecastDigest = snapshot.forecast.forecastDigest

def EligibleForExploitation (state : CalibrationState) : Prop :=
  ValidForecast state.currentSnapshot.forecast ∧
    state.mode ≠ .explore

def AuthorizedFutureBatch
    (chainVerified disclosureValid batchFeasible : Prop)
    (state : CalibrationState) : Prop :=
  chainVerified ∧
    disclosureValid ∧
    batchFeasible ∧
    EligibleForExploitation state

theorem exact_realization_has_zero_regret
    (cost : CalibrationCost) :
    observationRegret {
      forecastVersion := 0
      forecastDigest := 0
      planDigest := 0
      realizationDigest := 0
      predicted := cost
      realized := cost
    } = 0 := by
  simp [observationRegret, regretVector, absoluteDifference, totalRegret]

theorem calibration_update_appends_exactly_one_observation
    {threshold breachLimit : Nat}
    {before after : CalibrationState}
    {observation : CalibrationObservation}
    (valid :
      ValidCalibrationUpdate threshold breachLimit
        before observation after) :
    after.observations = before.observations ++ [observation] :=
  valid.appendOnly

theorem calibration_update_preserves_old_ledger_as_prefix
    {threshold breachLimit : Nat}
    {before after : CalibrationState}
    {observation : CalibrationObservation}
    (valid :
      ValidCalibrationUpdate threshold breachLimit
        before observation after) :
    before.observations.Sublist after.observations := by
  rw [valid.appendOnly]
  exact List.sublist_append_left before.observations [observation]

theorem calibration_update_increments_sample_count
    {threshold breachLimit : Nat}
    {before after : CalibrationState}
    {observation : CalibrationObservation}
    (valid :
      ValidCalibrationUpdate threshold breachLimit
        before observation after) :
    after.sampleCount = before.sampleCount + 1 :=
  valid.sampleAdvance

theorem calibration_update_accumulates_exact_regret
    {threshold breachLimit : Nat}
    {before after : CalibrationState}
    {observation : CalibrationObservation}
    (valid :
      ValidCalibrationUpdate threshold breachLimit
        before observation after) :
    after.cumulativeRegret =
      before.cumulativeRegret + observationRegret observation :=
  valid.regretAdvance

theorem nonbreach_resets_consecutive_breaches
    {threshold breachLimit : Nat}
    {before after : CalibrationState}
    {observation : CalibrationObservation}
    (valid :
      ValidCalibrationUpdate threshold breachLimit
        before observation after)
    (nonbreach : ¬threshold < observationRegret observation) :
    after.consecutiveBreaches = 0 := by
  rw [valid.breachAdvance, if_neg nonbreach]

theorem breach_increments_consecutive_breaches
    {threshold breachLimit : Nat}
    {before after : CalibrationState}
    {observation : CalibrationObservation}
    (valid :
      ValidCalibrationUpdate threshold breachLimit
        before observation after)
    (breach : threshold < observationRegret observation) :
    after.consecutiveBreaches = before.consecutiveBreaches + 1 := by
  rw [valid.breachAdvance, if_pos breach]

theorem persistent_breach_requires_exploration
    {threshold breachLimit : Nat}
    {before after : CalibrationState}
    {observation : CalibrationObservation}
    (valid :
      ValidCalibrationUpdate threshold breachLimit
        before observation after)
    (persistent : breachLimit ≤ after.consecutiveBreaches) :
    after.mode = .explore := by
  rw [valid.modeMatches]
  simp [requiredMode, persistent]

theorem bounded_bias_requires_degraded_mode
    {threshold breachLimit : Nat}
    {before after : CalibrationState}
    {observation : CalibrationObservation}
    (valid :
      ValidCalibrationUpdate threshold breachLimit
        before observation after)
    (positive : 0 < after.consecutiveBreaches)
    (belowLimit : after.consecutiveBreaches < breachLimit) :
    after.mode = .degraded := by
  rw [valid.modeMatches]
  simp [requiredMode, Nat.not_le_of_gt belowLimit, positive]

theorem zero_bias_with_positive_limit_requires_normal_mode
    {threshold breachLimit : Nat}
    {before after : CalibrationState}
    {observation : CalibrationObservation}
    (valid :
      ValidCalibrationUpdate threshold breachLimit
        before observation after)
    (positiveLimit : 0 < breachLimit)
    (zero : after.consecutiveBreaches = 0) :
    after.mode = .normal := by
  rw [valid.modeMatches, zero]
  simp [requiredMode, Nat.not_le_of_gt positiveLimit]

theorem calibration_update_preserves_old_decision_binding
    {threshold breachLimit : Nat}
    {before after : CalibrationState}
    {observation : CalibrationObservation}
    {decision : HistoricalDecision}
    (_valid :
      ValidCalibrationUpdate threshold breachLimit
        before observation after)
    (bound :
      DecisionBoundToSnapshot decision before.currentSnapshot) :
    DecisionBoundToSnapshot decision before.currentSnapshot :=
  bound

theorem new_snapshot_cannot_retroactively_rebind_old_decision
    {threshold breachLimit : Nat}
    {before after : CalibrationState}
    {observation : CalibrationObservation}
    {decision : HistoricalDecision}
    (valid :
      ValidCalibrationUpdate threshold breachLimit
        before observation after)
    (bound :
      DecisionBoundToSnapshot decision before.currentSnapshot) :
    ¬DecisionBoundToSnapshot decision after.currentSnapshot := by
  intro rebound
  have sameVersion :
      before.currentSnapshot.version = after.currentSnapshot.version :=
    bound.1.symm.trans rebound.1
  rw [valid.versionAdvance] at sameVersion
  omega

theorem exploration_mode_blocks_exploitation
    {state : CalibrationState}
    (explore : state.mode = .explore) :
    ¬EligibleForExploitation state := by
  intro eligible
  exact eligible.2 explore

theorem authorized_future_batch_requires_verified_chain
    {chainVerified disclosureValid batchFeasible : Prop}
    {state : CalibrationState}
    (authorized :
      AuthorizedFutureBatch chainVerified disclosureValid
        batchFeasible state) :
    chainVerified :=
  authorized.1

theorem authorized_future_batch_requires_valid_disclosure
    {chainVerified disclosureValid batchFeasible : Prop}
    {state : CalibrationState}
    (authorized :
      AuthorizedFutureBatch chainVerified disclosureValid
        batchFeasible state) :
    disclosureValid :=
  authorized.2.1

theorem authorized_future_batch_requires_feasible_batch
    {chainVerified disclosureValid batchFeasible : Prop}
    {state : CalibrationState}
    (authorized :
      AuthorizedFutureBatch chainVerified disclosureValid
        batchFeasible state) :
    batchFeasible :=
  authorized.2.2.1

theorem authorized_future_batch_requires_exploitation_eligibility
    {chainVerified disclosureValid batchFeasible : Prop}
    {state : CalibrationState}
    (authorized :
      AuthorizedFutureBatch chainVerified disclosureValid
        batchFeasible state) :
    EligibleForExploitation state :=
  authorized.2.2.2

end ASPProof.SearchRouteAppendOnlyForecastCalibration
