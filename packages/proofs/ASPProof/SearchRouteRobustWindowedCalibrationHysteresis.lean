import ASPProof.SearchRouteAppendOnlyForecastCalibration

namespace ASPProof.SearchRouteRobustWindowedCalibrationHysteresis

open ASPProof.SearchRouteAppendOnlyForecastCalibration

structure ObservationSourceKey where
  authorityId : Nat
  authorityEpoch : Nat
deriving DecidableEq, Repr

structure WindowObservation where
  source : ObservationSourceKey
  regret : Nat
  verified : Bool
deriving DecidableEq, Repr

def clippedRegret (cap : Nat) (observation : WindowObservation) : Nat :=
  min cap observation.regret

def windowScore (cap : Nat) : List WindowObservation → Nat
  | [] => 0
  | observation :: rest =>
      clippedRegret cap observation + windowScore cap rest

structure RobustWindow where
  capacity : Nat
  regretCap : Nat
  entries : List WindowObservation
deriving DecidableEq, Repr

def ValidRobustWindow (window : RobustWindow) : Prop :=
  window.entries.length ≤ window.capacity ∧
  (window.entries.map WindowObservation.source).Nodup ∧
  ∀ observation ∈ window.entries, observation.verified = true

theorem clipped_regret_is_bounded
    (cap : Nat)
    (observation : WindowObservation) :
    clippedRegret cap observation ≤ cap := by
  exact Nat.min_le_left cap observation.regret

theorem window_score_is_bounded_by_entry_count
    (cap : Nat)
    (entries : List WindowObservation) :
    windowScore cap entries ≤ entries.length * cap := by
  induction entries with
  | nil =>
      simp [windowScore]
  | cons observation rest inductionHypothesis =>
      have observationBound := clipped_regret_is_bounded cap observation
      calc
        windowScore cap (observation :: rest) =
            clippedRegret cap observation + windowScore cap rest := rfl
        _ ≤ cap + rest.length * cap :=
          Nat.add_le_add observationBound inductionHypothesis
        _ = (observation :: rest).length * cap := by
          simp [Nat.succ_mul, Nat.add_comm]

theorem valid_window_score_is_bounded_by_capacity
    (window : RobustWindow)
    (valid : ValidRobustWindow window) :
    windowScore window.regretCap window.entries ≤
      window.capacity * window.regretCap := by
  have scoreBound :=
    window_score_is_bounded_by_entry_count
      window.regretCap window.entries
  have capacityBound :
      window.entries.length * window.regretCap ≤
        window.capacity * window.regretCap :=
    Nat.mul_le_mul_right window.regretCap valid.1
  exact Nat.le_trans scoreBound capacityBound

theorem valid_window_has_unique_source_keys
    (window : RobustWindow)
    (valid : ValidRobustWindow window) :
    (window.entries.map WindowObservation.source).Nodup :=
  valid.2.1

theorem valid_window_contains_only_verified_observations
    (window : RobustWindow)
    (valid : ValidRobustWindow window)
    (observation : WindowObservation)
    (member : observation ∈ window.entries) :
    observation.verified = true :=
  valid.2.2 observation member

structure HysteresisPolicy where
  lowThreshold : Nat
  highThreshold : Nat
  recoveryLimit : Nat
  separated : lowThreshold < highThreshold
  positiveRecovery : 0 < recoveryLimit

def nextHysteresisMode
    (policy : HysteresisPolicy)
    (beforeMode : CalibrationMode)
    (score recoveryStreak : Nat) : CalibrationMode :=
  match beforeMode with
  | .explore =>
      if score ≤ policy.lowThreshold ∧
          policy.recoveryLimit ≤ recoveryStreak then
        .normal
      else
        .explore
  | .degraded =>
      if policy.highThreshold ≤ score then
        .explore
      else if score ≤ policy.lowThreshold then
        .normal
      else
        .degraded
  | .normal =>
      if policy.highThreshold ≤ score then
        .explore
      else if policy.lowThreshold < score then
        .degraded
      else
        .normal

theorem high_score_forces_exploration
    (policy : HysteresisPolicy)
    (beforeMode : CalibrationMode)
    (score recoveryStreak : Nat)
    (highReached : policy.highThreshold ≤ score) :
    nextHysteresisMode policy beforeMode score recoveryStreak =
      .explore := by
  have separated := policy.separated
  have notLow : ¬score ≤ policy.lowThreshold := by
    omega
  cases beforeMode <;>
    simp [nextHysteresisMode, highReached, notLow]

theorem exploration_persists_without_recovery
    (policy : HysteresisPolicy)
    (score recoveryStreak : Nat)
    (notRecovered :
      ¬(score ≤ policy.lowThreshold ∧
        policy.recoveryLimit ≤ recoveryStreak)) :
    nextHysteresisMode policy .explore score recoveryStreak =
      .explore := by
  simp [nextHysteresisMode, notRecovered]

theorem exploration_exit_requires_low_score_and_recovery
    (policy : HysteresisPolicy)
    (score recoveryStreak : Nat)
    (exits :
      nextHysteresisMode policy .explore score recoveryStreak =
        .normal) :
    score ≤ policy.lowThreshold ∧
      policy.recoveryLimit ≤ recoveryStreak := by
  change
    (if score ≤ policy.lowThreshold ∧
        policy.recoveryLimit ≤ recoveryStreak then
      CalibrationMode.normal
    else
      CalibrationMode.explore) = CalibrationMode.normal at exits
  by_cases recovered :
      score ≤ policy.lowThreshold ∧
        policy.recoveryLimit ≤ recoveryStreak
  ·
    exact recovered
  ·
    simp [recovered] at exits

theorem degraded_mode_is_stable_inside_hysteresis_band
    (policy : HysteresisPolicy)
    (score recoveryStreak : Nat)
    (aboveLow : policy.lowThreshold < score)
    (belowHigh : score < policy.highThreshold) :
    nextHysteresisMode policy .degraded score recoveryStreak =
      .degraded := by
  have notHigh : ¬policy.highThreshold ≤ score := by
    omega
  have notLow : ¬score ≤ policy.lowThreshold := by
    omega
  simp [nextHysteresisMode, notHigh, notLow]

structure SearchCacheKey where
  evidenceRootDigest : Nat
  providerDigest : Nat
  queryDigest : Nat
deriving DecidableEq, Repr

structure ModelPrefixCacheKey where
  protocolDigest : Nat
  stableInstructionDigest : Nat
deriving DecidableEq, Repr

structure ForecastPlanCacheKey where
  forecastVersion : Nat
  forecastDigest : Nat
deriving DecidableEq, Repr

structure CachePartition where
  search : SearchCacheKey
  modelPrefix : ModelPrefixCacheKey
  forecastPlan : ForecastPlanCacheKey
deriving DecidableEq, Repr

structure ValidCalibrationRefresh
    (before after : CachePartition) : Prop where
  searchCacheStable : after.search = before.search
  modelPrefixStable : after.modelPrefix = before.modelPrefix
  forecastVersionAdvanced :
    after.forecastPlan.forecastVersion =
      before.forecastPlan.forecastVersion + 1

theorem calibration_refresh_preserves_search_cache
    {before after : CachePartition}
    (valid : ValidCalibrationRefresh before after) :
    after.search = before.search :=
  valid.searchCacheStable

theorem calibration_refresh_preserves_model_prefix_cache
    {before after : CachePartition}
    (valid : ValidCalibrationRefresh before after) :
    after.modelPrefix = before.modelPrefix :=
  valid.modelPrefixStable

theorem calibration_refresh_advances_forecast_plan_version
    {before after : CachePartition}
    (valid : ValidCalibrationRefresh before after) :
    after.forecastPlan.forecastVersion =
      before.forecastPlan.forecastVersion + 1 :=
  valid.forecastVersionAdvanced

structure LoopOverhead where
  graphHops : Nat
  providerCommands : Nat
  llmRounds : Nat
  disclosedTokens : Nat
deriving DecidableEq, Repr

def ZeroLoopOverhead (overhead : LoopOverhead) : Prop :=
  overhead.graphHops = 0 ∧
  overhead.providerCommands = 0 ∧
  overhead.llmRounds = 0 ∧
  overhead.disclosedTokens = 0

theorem passive_calibration_adds_no_graph_hops
    (overhead : LoopOverhead)
    (zero : ZeroLoopOverhead overhead) :
    overhead.graphHops = 0 :=
  zero.1

theorem passive_calibration_adds_no_provider_commands
    (overhead : LoopOverhead)
    (zero : ZeroLoopOverhead overhead) :
    overhead.providerCommands = 0 :=
  zero.2.1

theorem passive_calibration_adds_no_llm_rounds
    (overhead : LoopOverhead)
    (zero : ZeroLoopOverhead overhead) :
    overhead.llmRounds = 0 :=
  zero.2.2.1

theorem passive_calibration_adds_no_disclosed_tokens
    (overhead : LoopOverhead)
    (zero : ZeroLoopOverhead overhead) :
    overhead.disclosedTokens = 0 :=
  zero.2.2.2

structure AuthorizationFacts where
  verifiedChain : Bool
  validDisclosure : Bool
  feasibleBatch : Bool
deriving DecidableEq, Repr

def ValidAuthorizationFacts (facts : AuthorizationFacts) : Prop :=
  facts.verifiedChain = true ∧
  facts.validDisclosure = true ∧
  facts.feasibleBatch = true

structure AuthorizedRobustBatch
    (window : RobustWindow)
    (mode : CalibrationMode)
    (overhead : LoopOverhead)
    (facts : AuthorizationFacts) : Prop where
  windowValid : ValidRobustWindow window
  factsValid : ValidAuthorizationFacts facts
  exploitationEligible : mode ≠ .explore
  zeroCalibrationOverhead : ZeroLoopOverhead overhead

theorem authorized_robust_batch_requires_valid_window
    {window : RobustWindow}
    {mode : CalibrationMode}
    {overhead : LoopOverhead}
    {facts : AuthorizationFacts}
    (authorized : AuthorizedRobustBatch window mode overhead facts) :
    ValidRobustWindow window :=
  authorized.windowValid

theorem authorized_robust_batch_requires_valid_facts
    {window : RobustWindow}
    {mode : CalibrationMode}
    {overhead : LoopOverhead}
    {facts : AuthorizationFacts}
    (authorized : AuthorizedRobustBatch window mode overhead facts) :
    ValidAuthorizationFacts facts :=
  authorized.factsValid

theorem authorized_robust_batch_rejects_exploration_mode
    {window : RobustWindow}
    {mode : CalibrationMode}
    {overhead : LoopOverhead}
    {facts : AuthorizationFacts}
    (authorized : AuthorizedRobustBatch window mode overhead facts) :
    mode ≠ .explore :=
  authorized.exploitationEligible

theorem authorized_robust_batch_has_zero_calibration_overhead
    {window : RobustWindow}
    {mode : CalibrationMode}
    {overhead : LoopOverhead}
    {facts : AuthorizationFacts}
    (authorized : AuthorizedRobustBatch window mode overhead facts) :
    ZeroLoopOverhead overhead :=
  authorized.zeroCalibrationOverhead

end ASPProof.SearchRouteRobustWindowedCalibrationHysteresis
