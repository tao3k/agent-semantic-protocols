import ASPProof.AgentSessionTransactionalGenerationLifecycle

namespace ASPProof.AgentSessionLifecycleCrashRecovery

open AgentSessionTransactionalGenerationLifecycle

inductive LifecycleIntentKind where
  | retire
  | releasePath
  | spawnReplacement
  | bindGeneration
  deriving DecidableEq

structure LifecycleIntent where
  slot : ResidentSlot
  generation : Nat
  intentKey : Nat
  kind : LifecycleIntentKind
  payloadDigest : Nat
  deriving DecidableEq

structure HostEffectReceipt where
  slot : ResidentSlot
  generation : Nat
  intentKey : Nat
  kind : LifecycleIntentKind
  payloadDigest : Nat
  effectIdentity : Nat
  accepted : Bool
  deriving DecidableEq

def ReceiptMatchesIntent
    (intent : LifecycleIntent) (receipt : HostEffectReceipt) : Prop :=
  receipt.slot = intent.slot ∧
  receipt.generation = intent.generation ∧
  receipt.intentKey = intent.intentKey ∧
  receipt.kind = intent.kind ∧
  receipt.payloadDigest = intent.payloadDigest ∧
  receipt.accepted = true

def HostIdempotentFor
    (intent : LifecycleIntent)
    (left right : HostEffectReceipt) : Prop :=
  ReceiptMatchesIntent intent left →
  ReceiptMatchesIntent intent right →
  left.effectIdentity = right.effectIdentity

structure CrashState where
  intentDurable : Bool
  hostAccepted : Bool
  receiptIndexed : Bool
  stateCommitted : Bool
  pollBudget : Nat
  deriving DecidableEq

inductive RecoveryAction where
  | blockedMissingIntent
  | pollStableIntent
  | persistIndexedReceipt
  | commitRetiredCas
  | commitReleasedCas
  | commitBindingCas
  | alreadyCommitted
  | blockedPollBudget
  deriving DecidableEq

def commitAction : LifecycleIntentKind → RecoveryAction
  | .retire => .commitRetiredCas
  | .releasePath => .commitReleasedCas
  | .spawnReplacement => .commitBindingCas
  | .bindGeneration => .commitBindingCas

def classifyRecovery
    (kind : LifecycleIntentKind) (state : CrashState) : RecoveryAction :=
  if !state.intentDurable then
    .blockedMissingIntent
  else if state.stateCommitted then
    .alreadyCommitted
  else if state.receiptIndexed then
    commitAction kind
  else if state.hostAccepted then
    .persistIndexedReceipt
  else if state.pollBudget = 0 then
    .blockedPollBudget
  else
    .pollStableIntent

def recoveryRank (state : CrashState) : Nat :=
  if state.stateCommitted then 0
  else if state.receiptIndexed then 1
  else if state.hostAccepted then 2
  else if state.intentDurable then 3
  else 0

structure CommittedRecoveryStep where
  current : CrashState
  next : CrashState
  rankDecreases : recoveryRank next < recoveryRank current

structure PollStep where
  currentBudget : Nat
  nextBudget : Nat
  budgetDecreases : nextBudget < currentBudget

structure DispatchRecord where
  slot : ResidentSlot
  dispatchKey : Nat
  commandDigest : Nat
  deriving DecidableEq

structure DispatchCompletionReceipt where
  slot : ResidentSlot
  dispatchKey : Nat
  commandDigest : Nat
  deliveryGeneration : Nat
  effectDigest : Nat
  terminal : Bool
  deriving DecidableEq

def ValidDispatchCompletion
    (dispatch : DispatchRecord)
    (receipt : DispatchCompletionReceipt) : Prop :=
  receipt.slot = dispatch.slot ∧
  receipt.dispatchKey = dispatch.dispatchKey ∧
  receipt.commandDigest = dispatch.commandDigest ∧
  receipt.terminal = true

inductive DispatchRecoveryAction where
  | returnStoredTerminal
  | replayStableDispatchKey
  | awaitVerifiedBinding
  deriving DecidableEq

def classifyDispatchRecovery
    (terminalReceiptExists verifiedBindingExists : Bool) : DispatchRecoveryAction :=
  if terminalReceiptExists then .returnStoredTerminal
  else if verifiedBindingExists then .replayStableDispatchKey
  else .awaitVerifiedBinding

theorem matching_receipt_binds_intent_key
    {intent : LifecycleIntent} {receipt : HostEffectReceipt}
    (valid : ReceiptMatchesIntent intent receipt) :
    receipt.intentKey = intent.intentKey :=
  valid.2.2.1

theorem matching_receipt_binds_generation
    {intent : LifecycleIntent} {receipt : HostEffectReceipt}
    (valid : ReceiptMatchesIntent intent receipt) :
    receipt.generation = intent.generation :=
  valid.2.1

theorem matching_receipt_binds_payload
    {intent : LifecycleIntent} {receipt : HostEffectReceipt}
    (valid : ReceiptMatchesIntent intent receipt) :
    receipt.payloadDigest = intent.payloadDigest :=
  valid.2.2.2.2.1

theorem idempotent_host_replay_preserves_effect_identity
    {intent : LifecycleIntent} {left right : HostEffectReceipt}
    (idempotent : HostIdempotentFor intent left right)
    (leftMatches : ReceiptMatchesIntent intent left)
    (rightMatches : ReceiptMatchesIntent intent right) :
    left.effectIdentity = right.effectIdentity :=
  idempotent leftMatches rightMatches

theorem different_intent_key_rejects_receipt
    {intent : LifecycleIntent} {receipt : HostEffectReceipt}
    (different : receipt.intentKey ≠ intent.intentKey) :
    ¬ ReceiptMatchesIntent intent receipt := by
  intro valid
  exact different valid.2.2.1

theorem different_payload_rejects_receipt
    {intent : LifecycleIntent} {receipt : HostEffectReceipt}
    (different : receipt.payloadDigest ≠ intent.payloadDigest) :
    ¬ ReceiptMatchesIntent intent receipt := by
  intro valid
  exact different valid.2.2.2.2.1

theorem retirement_accepted_before_receipt_persists_receipt :
    classifyRecovery .retire ⟨true, true, false, false, 2⟩ =
      .persistIndexedReceipt :=
  rfl

theorem release_receipt_before_delivered_cas_commits_release :
    classifyRecovery .releasePath ⟨true, true, true, false, 2⟩ =
      .commitReleasedCas :=
  rfl

theorem typed_start_before_binding_commit_commits_binding :
    classifyRecovery .spawnReplacement ⟨true, true, true, false, 2⟩ =
      .commitBindingCas :=
  rfl

theorem bind_receipt_before_binding_commit_commits_binding :
    classifyRecovery .bindGeneration ⟨true, true, true, false, 2⟩ =
      .commitBindingCas :=
  rfl

theorem missing_durable_intent_blocks_recovery :
    classifyRecovery .retire ⟨false, true, true, false, 2⟩ =
      .blockedMissingIntent :=
  rfl

theorem committed_state_is_idempotent :
    classifyRecovery .spawnReplacement ⟨true, true, true, true, 2⟩ =
      .alreadyCommitted :=
  rfl

theorem unknown_acceptance_polls_same_intent :
    classifyRecovery .retire ⟨true, false, false, false, 2⟩ =
      .pollStableIntent :=
  rfl

theorem exhausted_poll_budget_is_explicitly_blocked :
    classifyRecovery .retire ⟨true, false, false, false, 0⟩ =
      .blockedPollBudget :=
  rfl

theorem committed_recovery_has_no_self_loop
    (step : CommittedRecoveryStep) : step.current ≠ step.next := by
  intro same
  exact (Nat.ne_of_lt step.rankDecreases) (congrArg recoveryRank same).symm

theorem committed_recovery_has_no_two_cycle
    (forward backward : CommittedRecoveryStep)
    (linked : forward.next = backward.current)
    (returned : backward.next = forward.current) : False := by
  have lessForward := forward.rankDecreases
  have lessBackward : recoveryRank forward.current < recoveryRank forward.next := by
    simpa [linked, returned] using backward.rankDecreases
  exact (Nat.not_lt_of_ge (Nat.le_of_lt lessBackward)) lessForward

theorem poll_step_has_no_self_loop (step : PollStep) :
    step.currentBudget ≠ step.nextBudget := by
  exact (Nat.ne_of_gt step.budgetDecreases)

theorem valid_completion_binds_stable_dispatch_key
    {dispatch : DispatchRecord} {receipt : DispatchCompletionReceipt}
    (valid : ValidDispatchCompletion dispatch receipt) :
    receipt.dispatchKey = dispatch.dispatchKey :=
  valid.2.1

theorem valid_completion_is_generation_independent
    {dispatch : DispatchRecord} {receipt : DispatchCompletionReceipt}
    (valid : ValidDispatchCompletion dispatch receipt) :
    receipt.commandDigest = dispatch.commandDigest :=
  valid.2.2.1

theorem terminal_completion_during_replacement_wins :
    classifyDispatchRecovery true false = .returnStoredTerminal :=
  rfl

theorem terminal_completion_prevents_replay_even_after_binding :
    classifyDispatchRecovery true true = .returnStoredTerminal :=
  rfl

theorem orphaned_dispatch_without_binding_waits :
    classifyDispatchRecovery false false = .awaitVerifiedBinding :=
  rfl

theorem orphaned_dispatch_after_binding_replays_stable_key :
    classifyDispatchRecovery false true = .replayStableDispatchKey :=
  rfl

end ASPProof.AgentSessionLifecycleCrashRecovery
