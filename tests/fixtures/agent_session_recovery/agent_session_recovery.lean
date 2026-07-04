namespace ASP.AgentSessionRecovery

inductive RolloutStatus where
  | activeRunning
  | completed
  | silent
  | orphanRisk
  deriving DecidableEq, Repr

inductive SendResult where
  | accepted
  | failed
  deriving DecidableEq, Repr

inductive ResumeResult where
  | accepted
  | failed
  deriving DecidableEq, Repr

inductive Evidence where
  | send : SendResult -> Evidence
  | resume : ResumeResult -> Evidence
  | parentWaitDeadline
  | rollout : RolloutStatus -> Evidence
  | hostNotFound
  | validationFailed
  | modelAbnormalClose
  | boundedNoProgressExhausted
  | providerTimeout
  deriving DecidableEq, Repr

inductive Action where
  | enterRolloutWait
  | resumeExistingChild
  | retrySend
  | querySessionStatus
  | waitPatiently
  | readCompletedTurn
  | requestBoundedStatusOrInterrupt
  | resolveOrphanRisk
  | recordProviderTimeoutReceipt
  | replaceChild
  deriving DecidableEq, Repr

inductive SessionState where
  | mainNeedsChild
  | childReady
  | resumePending
  | retryingSend
  | statusRequired
  | rolloutWaiting
  | resultReady
  | boundedRecovery
  | orphanRecovery
  | providerTimeoutHandled
  | replacingChild
  deriving DecidableEq, Repr

structure ProviderTimeoutRecovery where
  internalTimeout : Bool
  processGroupTerminated : Bool
  orphanProviderChild : Bool
  replacesAgentChild : Bool
  deriving DecidableEq, Repr

def providerTimeoutRecovery : ProviderTimeoutRecovery := {
  internalTimeout := true,
  processGroupTerminated := true,
  orphanProviderChild := false,
  replacesAgentChild := false
}

def nextAction : Evidence -> Action
  | .send .accepted => .enterRolloutWait
  | .send .failed => .resumeExistingChild
  | .resume .accepted => .retrySend
  | .resume .failed => .querySessionStatus
  | .parentWaitDeadline => .querySessionStatus
  | .rollout .activeRunning => .waitPatiently
  | .rollout .completed => .readCompletedTurn
  | .rollout .silent => .requestBoundedStatusOrInterrupt
  | .rollout .orphanRisk => .resolveOrphanRisk
  | .hostNotFound => .replaceChild
  | .validationFailed => .replaceChild
  | .modelAbnormalClose => .replaceChild
  | .boundedNoProgressExhausted => .replaceChild
  | .providerTimeout => .recordProviderTimeoutReceipt

def replacementEvidence : Evidence -> Prop
  | .hostNotFound => True
  | .validationFailed => True
  | .modelAbnormalClose => True
  | .boundedNoProgressExhausted => True
  | _ => False

def stateFromAction : Action -> SessionState
  | .enterRolloutWait => .rolloutWaiting
  | .resumeExistingChild => .resumePending
  | .retrySend => .retryingSend
  | .querySessionStatus => .statusRequired
  | .waitPatiently => .rolloutWaiting
  | .readCompletedTurn => .resultReady
  | .requestBoundedStatusOrInterrupt => .boundedRecovery
  | .resolveOrphanRisk => .orphanRecovery
  | .recordProviderTimeoutReceipt => .providerTimeoutHandled
  | .replaceChild => .replacingChild

def step (_state : SessionState) (evidence : Evidence) : SessionState :=
  stateFromAction (nextAction evidence)

def retryEvidence (remaining : Nat) (progressObserved : Bool) : Evidence :=
  if progressObserved then
    .rollout .activeRunning
  else
    match remaining with
    | 0 => .boundedNoProgressExhausted
    | _ + 1 => .rollout .silent

theorem wait_deadline_is_not_replacement :
    nextAction .parentWaitDeadline ≠ .replaceChild := by
  simp [nextAction]

theorem active_heartbeat_is_not_replacement :
    nextAction (.rollout .activeRunning) ≠ .replaceChild := by
  simp [nextAction]

theorem completed_turn_is_not_model_abnormal_close :
    nextAction (.rollout .completed) = .readCompletedTurn := by
  simp [nextAction]

theorem model_abnormal_close_replaces :
    nextAction .modelAbnormalClose = .replaceChild := by
  simp [nextAction]

theorem send_failure_resumes_existing_child :
    nextAction (.send .failed) = .resumeExistingChild := by
  simp [nextAction]

theorem resume_failure_queries_status :
    nextAction (.resume .failed) = .querySessionStatus := by
  simp [nextAction]

theorem provider_timeout_does_not_replace_child :
    nextAction .providerTimeout ≠ .replaceChild := by
  simp [nextAction]

theorem provider_timeout_uses_internal_process_group_cleanup :
    providerTimeoutRecovery.internalTimeout = true
      ∧ providerTimeoutRecovery.processGroupTerminated = true
      ∧ providerTimeoutRecovery.orphanProviderChild = false
      ∧ providerTimeoutRecovery.replacesAgentChild = false := by
  simp [providerTimeoutRecovery]

theorem replace_only_after_replacement_evidence (e : Evidence) :
    nextAction e = .replaceChild -> replacementEvidence e := by
  cases e with
  | send result =>
      cases result <;> simp [nextAction, replacementEvidence]
  | resume result =>
      cases result <;> simp [nextAction, replacementEvidence]
  | parentWaitDeadline =>
      simp [nextAction, replacementEvidence]
  | rollout status =>
      cases status <;> simp [nextAction, replacementEvidence]
  | hostNotFound =>
      simp [nextAction, replacementEvidence]
  | validationFailed =>
      simp [nextAction, replacementEvidence]
  | modelAbnormalClose =>
      simp [nextAction, replacementEvidence]
  | boundedNoProgressExhausted =>
      simp [nextAction, replacementEvidence]
  | providerTimeout =>
      simp [nextAction, replacementEvidence]

theorem replacement_evidence_replaces (e : Evidence) :
    replacementEvidence e -> nextAction e = .replaceChild := by
  cases e with
  | send result =>
      cases result <;> simp [nextAction, replacementEvidence]
  | resume result =>
      cases result <;> simp [nextAction, replacementEvidence]
  | parentWaitDeadline =>
      simp [nextAction, replacementEvidence]
  | rollout status =>
      cases status <;> simp [nextAction, replacementEvidence]
  | hostNotFound =>
      simp [nextAction, replacementEvidence]
  | validationFailed =>
      simp [nextAction, replacementEvidence]
  | modelAbnormalClose =>
      simp [nextAction, replacementEvidence]
  | boundedNoProgressExhausted =>
      simp [nextAction, replacementEvidence]
  | providerTimeout =>
      simp [nextAction, replacementEvidence]

theorem step_replaces_only_after_replacement_evidence
    (state : SessionState) (evidence : Evidence) :
    step state evidence = .replacingChild -> replacementEvidence evidence := by
  cases evidence with
  | send result =>
      cases result <;> simp [step, stateFromAction, nextAction, replacementEvidence]
  | resume result =>
      cases result <;> simp [step, stateFromAction, nextAction, replacementEvidence]
  | parentWaitDeadline =>
      simp [step, stateFromAction, nextAction, replacementEvidence]
  | rollout status =>
      cases status <;> simp [step, stateFromAction, nextAction, replacementEvidence]
  | hostNotFound =>
      simp [step, stateFromAction, nextAction, replacementEvidence]
  | validationFailed =>
      simp [step, stateFromAction, nextAction, replacementEvidence]
  | modelAbnormalClose =>
      simp [step, stateFromAction, nextAction, replacementEvidence]
  | boundedNoProgressExhausted =>
      simp [step, stateFromAction, nextAction, replacementEvidence]
  | providerTimeout =>
      simp [step, stateFromAction, nextAction, replacementEvidence]

theorem replace_in_trace_implies_replacement_evidence (events : List Evidence) :
    .replaceChild ∈ events.map nextAction ->
      ∃ evidence, evidence ∈ events ∧ replacementEvidence evidence := by
  intro replace_in_actions
  rcases List.mem_map.mp replace_in_actions with ⟨evidence, evidence_in_events, action_eq⟩
  exact ⟨evidence, evidence_in_events, replace_only_after_replacement_evidence evidence action_eq⟩

theorem no_replace_if_no_replacement_evidence (events : List Evidence) :
    (∀ evidence, evidence ∈ events -> ¬ replacementEvidence evidence) ->
      .replaceChild ∉ events.map nextAction := by
  intro no_replacement_evidence replace_in_actions
  rcases replace_in_trace_implies_replacement_evidence events replace_in_actions with
    ⟨evidence, evidence_in_events, replacement_evidence⟩
  exact no_replacement_evidence evidence evidence_in_events replacement_evidence

theorem wait_and_heartbeat_trace_does_not_replace :
    .replaceChild ∉
      ([Evidence.send .accepted,
        Evidence.parentWaitDeadline,
        Evidence.rollout .activeRunning,
        Evidence.providerTimeout] : List Evidence).map nextAction := by
  simp [nextAction]

theorem send_then_resume_failure_queries_status_before_replacement :
    ([Evidence.send .failed, Evidence.resume .failed] : List Evidence).map nextAction =
      [Action.resumeExistingChild, Action.querySessionStatus] := by
  rfl

theorem no_progress_with_remaining_budget_does_not_replace (n : Nat) :
    nextAction (retryEvidence (n + 1) false) ≠ .replaceChild := by
  simp [retryEvidence, nextAction]

theorem no_progress_with_remaining_budget_requests_bounded_status (n : Nat) :
    nextAction (retryEvidence (n + 1) false) = .requestBoundedStatusOrInterrupt := by
  simp [retryEvidence, nextAction]

theorem no_progress_without_budget_replaces :
    nextAction (retryEvidence 0 false) = .replaceChild := by
  simp [retryEvidence, nextAction]

theorem observed_progress_waits_patiently (n : Nat) :
    nextAction (retryEvidence n true) = .waitPatiently := by
  simp [retryEvidence, nextAction]

inductive CandidateAction where
  | sendToAspExplore
  | querySessionStatus
  | resumeChild
  | createChild
  | waitPatiently
  | readResult
  | requestBoundedStatusOrInterrupt
  | resolveOrphanRisk
  | recordProviderTimeoutReceipt
  | rawFallback
  deriving DecidableEq, Repr

structure DenyReceipt where
  hasRequiredAction : Bool
  hasNextAction : Bool
  hasCompletionReceipt : Bool
  forbidsRawFallback : Bool
  deriving DecidableEq, Repr

def DenyReceipt.sound (receipt : DenyReceipt) : Prop :=
  receipt.hasRequiredAction = true
    ∧ receipt.hasNextAction = true
    ∧ receipt.hasCompletionReceipt = true
    ∧ receipt.forbidsRawFallback = true

def actionPermittedByReceipt
    (receipt : DenyReceipt) (action : CandidateAction) : Prop :=
  if receipt.hasRequiredAction
      && receipt.hasNextAction
      && receipt.hasCompletionReceipt
      && receipt.forbidsRawFallback then
    action ≠ .rawFallback
  else
    True

def underspecifiedDenyReceipt : DenyReceipt := {
  hasRequiredAction := true,
  hasNextAction := false,
  hasCompletionReceipt := false,
  forbidsRawFallback := false
}

theorem underspecified_receipt_permits_raw_fallback :
    actionPermittedByReceipt underspecifiedDenyReceipt .rawFallback := by
  simp [actionPermittedByReceipt, underspecifiedDenyReceipt]

theorem sound_receipt_forbids_raw_fallback (receipt : DenyReceipt) :
    receipt.sound -> ¬ actionPermittedByReceipt receipt .rawFallback := by
  intro sound
  rcases sound with
    ⟨hasRequiredAction, hasNextAction, hasCompletionReceipt, forbidsRawFallback⟩
  simp [
    actionPermittedByReceipt,
    hasRequiredAction,
    hasNextAction,
    hasCompletionReceipt,
    forbidsRawFallback,
  ]

inductive RecoveryObservation where
  | hookDenied : DenyReceipt -> RecoveryObservation
  | noActiveChild
  | sendFailed
  | resumeFailed
  | parentWaitDeadline
  | rollout : RolloutStatus -> RecoveryObservation
  | hostNotFound
  | validationFailed
  | modelAbnormalClose
  | boundedNoProgressExhausted
  | providerTimeout
  deriving DecidableEq, Repr

def recoveryPolicy : RecoveryObservation -> CandidateAction
  | .hookDenied receipt =>
      if receipt.hasRequiredAction
          && receipt.hasNextAction
          && receipt.hasCompletionReceipt
          && receipt.forbidsRawFallback then
        .sendToAspExplore
      else
        .querySessionStatus
  | .noActiveChild => .createChild
  | .sendFailed => .resumeChild
  | .resumeFailed => .querySessionStatus
  | .parentWaitDeadline => .querySessionStatus
  | .rollout .activeRunning => .waitPatiently
  | .rollout .completed => .readResult
  | .rollout .silent => .requestBoundedStatusOrInterrupt
  | .rollout .orphanRisk => .resolveOrphanRisk
  | .hostNotFound => .createChild
  | .validationFailed => .createChild
  | .modelAbnormalClose => .createChild
  | .boundedNoProgressExhausted => .createChild
  | .providerTimeout => .recordProviderTimeoutReceipt

def protocolAction : CandidateAction -> Prop
  | .rawFallback => False
  | _ => True

def standaloneLoopObservation
    (remainingRetries : Nat) (progressObserved : Bool) : RecoveryObservation :=
  if progressObserved then
    .rollout .activeRunning
  else
    match remainingRetries with
    | 0 => .boundedNoProgressExhausted
    | _ + 1 => .rollout .silent

theorem recovery_policy_never_raw_fallback (observation : RecoveryObservation) :
    recoveryPolicy observation ≠ .rawFallback := by
  cases observation with
  | hookDenied receipt =>
      rcases receipt with ⟨hasRequiredAction, hasNextAction, hasCompletionReceipt, forbidsRawFallback⟩
      cases hasRequiredAction <;>
        cases hasNextAction <;>
        cases hasCompletionReceipt <;>
        cases forbidsRawFallback <;>
        simp [recoveryPolicy]
  | noActiveChild =>
      simp [recoveryPolicy]
  | sendFailed =>
      simp [recoveryPolicy]
  | resumeFailed =>
      simp [recoveryPolicy]
  | parentWaitDeadline =>
      simp [recoveryPolicy]
  | rollout status =>
      cases status <;> simp [recoveryPolicy]
  | hostNotFound =>
      simp [recoveryPolicy]
  | validationFailed =>
      simp [recoveryPolicy]
  | modelAbnormalClose =>
      simp [recoveryPolicy]
  | boundedNoProgressExhausted =>
      simp [recoveryPolicy]
  | providerTimeout =>
      simp [recoveryPolicy]

theorem recovery_policy_is_standalone_protocol_action
    (observation : RecoveryObservation) :
    protocolAction (recoveryPolicy observation) := by
  cases observation with
  | hookDenied receipt =>
      rcases receipt with ⟨hasRequiredAction, hasNextAction, hasCompletionReceipt, forbidsRawFallback⟩
      cases hasRequiredAction <;>
        cases hasNextAction <;>
        cases hasCompletionReceipt <;>
        cases forbidsRawFallback <;>
        simp [protocolAction, recoveryPolicy]
  | noActiveChild =>
      simp [protocolAction, recoveryPolicy]
  | sendFailed =>
      simp [protocolAction, recoveryPolicy]
  | resumeFailed =>
      simp [protocolAction, recoveryPolicy]
  | parentWaitDeadline =>
      simp [protocolAction, recoveryPolicy]
  | rollout status =>
      cases status <;> simp [protocolAction, recoveryPolicy]
  | hostNotFound =>
      simp [protocolAction, recoveryPolicy]
  | validationFailed =>
      simp [protocolAction, recoveryPolicy]
  | modelAbnormalClose =>
      simp [protocolAction, recoveryPolicy]
  | boundedNoProgressExhausted =>
      simp [protocolAction, recoveryPolicy]
  | providerTimeout =>
      simp [protocolAction, recoveryPolicy]

theorem hook_deny_with_sound_receipt_routes_to_asp_explore
    (receipt : DenyReceipt) :
    receipt.sound ->
      recoveryPolicy (.hookDenied receipt) = .sendToAspExplore := by
  intro sound
  rcases sound with
    ⟨hasRequiredAction, hasNextAction, hasCompletionReceipt, forbidsRawFallback⟩
  simp [
    recoveryPolicy,
    hasRequiredAction,
    hasNextAction,
    hasCompletionReceipt,
    forbidsRawFallback,
  ]

theorem hook_deny_without_sound_receipt_queries_status_not_raw_fallback :
    recoveryPolicy (.hookDenied underspecifiedDenyReceipt) = .querySessionStatus := by
  simp [recoveryPolicy, underspecifiedDenyReceipt]

theorem standalone_loop_with_remaining_budget_stays_bounded (n : Nat) :
    recoveryPolicy (standaloneLoopObservation (n + 1) false)
      = .requestBoundedStatusOrInterrupt := by
  simp [standaloneLoopObservation, recoveryPolicy]

theorem standalone_loop_without_budget_recreates :
    recoveryPolicy (standaloneLoopObservation 0 false) = .createChild := by
  simp [standaloneLoopObservation, recoveryPolicy]

theorem standalone_loop_with_progress_waits (n : Nat) :
    recoveryPolicy (standaloneLoopObservation n true) = .waitPatiently := by
  simp [standaloneLoopObservation, recoveryPolicy]

theorem provider_timeout_is_receipt_not_escape :
    recoveryPolicy .providerTimeout = .recordProviderTimeoutReceipt
      ∧ recoveryPolicy .providerTimeout ≠ .rawFallback
      ∧ recoveryPolicy .providerTimeout ≠ .createChild := by
  simp [recoveryPolicy]

structure RecoveryFields where
  requiredAction : Option String
  nextAction : Option String
  completionReceipt : Option String
  forbiddenUntilResolved : Option String
deriving Repr

def fillMissing (current fallback : Option String) : Option String :=
  match current with
  | some value => some value
  | none => fallback

def sourceAccessReplayEnrichFields (fields : RecoveryFields) : RecoveryFields :=
  { fields with
    requiredAction :=
      fillMissing fields.requiredAction (some "send-to-asp-explore")
    nextAction :=
      fillMissing fields.nextAction
        (some "run-asp-command-in-registered-asp-explore-child")
    completionReceipt :=
      fillMissing fields.completionReceipt (some "asp-explore-child-command")
    forbiddenUntilResolved :=
      fillMissing fields.forbiddenUntilResolved (some "raw-source-fallback") }

def sessionBootstrapRecoveryFields : RecoveryFields :=
  { requiredAction := some "start-asp-explore-child"
    nextAction := some "run-asp-agent-session-register-guide"
    completionReceipt := some "asp-explore-child-registration"
    forbiddenUntilResolved := some "raw-source-fallback" }

theorem source_access_replay_enrichment_preserves_required_action
    (fields : RecoveryFields) (action : String) :
    fields.requiredAction = some action →
      (sourceAccessReplayEnrichFields fields).requiredAction = some action := by
  intro existing
  simp [sourceAccessReplayEnrichFields, fillMissing, existing]

theorem source_access_replay_enrichment_adds_required_action_when_missing
    (fields : RecoveryFields) :
    fields.requiredAction = none →
      (sourceAccessReplayEnrichFields fields).requiredAction
        = some "send-to-asp-explore" := by
  intro missing
  simp [sourceAccessReplayEnrichFields, fillMissing, missing]

theorem source_access_replay_enrichment_does_not_downgrade_bootstrap :
    (sourceAccessReplayEnrichFields sessionBootstrapRecoveryFields).requiredAction
      = some "start-asp-explore-child"
    ∧ (sourceAccessReplayEnrichFields sessionBootstrapRecoveryFields).nextAction
      = some "run-asp-agent-session-register-guide"
    ∧ (sourceAccessReplayEnrichFields sessionBootstrapRecoveryFields).completionReceipt
      = some "asp-explore-child-registration" := by
  simp [sourceAccessReplayEnrichFields, sessionBootstrapRecoveryFields, fillMissing]

end ASP.AgentSessionRecovery
