import ASPProof.AgentSessionHostRegistryExecutableReceiptMaterialization
import ASPProof.AgentSessionDispatchTransaction

namespace ASPProof.AgentSessionHostRegistryCrashSafeMaterializationRecovery

open ASPProof.AgentSessionHostRegistryExecutableReceiptMaterialization
open ASPProof.AgentSessionDispatchTransaction

structure StableInvocationKey where
  dispatchKey : DispatchKey
  invocationDigest : Nat
  generation : Nat
  deriving DecidableEq, Repr

structure DurableMaterializationIntent where
  key : StableInvocationKey
  payloadDigest : Nat
  persisted : Bool
  deriving DecidableEq, Repr

structure MaterializationInvocation where
  key : StableInvocationKey
  payloadDigest : Nat
  started : Bool
  deriving DecidableEq, Repr

def IntentAuthorizesInvocation
    (intent : DurableMaterializationIntent)
    (invocation : MaterializationInvocation) : Prop :=
  intent.persisted = true ∧
  invocation.started = true ∧
  invocation.key = intent.key ∧
  invocation.payloadDigest = intent.payloadDigest

inductive CaptureChannel where
  | stdout
  | stderr
  deriving DecidableEq, Repr

structure CaptureFragment where
  sequence : Nat
  channel : CaptureChannel
  bytesDigest : Nat
  deriving DecidableEq, Repr

abbrev CaptureLog := List CaptureFragment

def AppendOnlyCapture (before after : CaptureLog) : Prop :=
  ∃ suffix, after = before ++ suffix

structure TerminalCapture where
  stdoutDigest : Nat
  stderrDigest : Nat
  exitCode : Nat
  durationMs : Nat
  effectDigest : Nat
  deriving DecidableEq, Repr

inductive SealSlot where
  | empty
  | sealed (key : StableInvocationKey) (terminal : TerminalCapture)
  | quarantined
      (key : StableInvocationKey)
      (accepted conflicting : TerminalCapture)
  deriving DecidableEq, Repr

structure SealHead where
  revision : Nat
  slot : SealSlot
  deriving DecidableEq, Repr

structure SealRequest where
  expectedRevision : Nat
  key : StableInvocationKey
  terminal : TerminalCapture
  deriving DecidableEq, Repr

inductive SealCas : SealRequest → SealHead → SealHead → Prop where
  | first
      (revision : Nat)
      (key : StableInvocationKey)
      (terminal : TerminalCapture) :
      SealCas
        ⟨revision, key, terminal⟩
        ⟨revision, .empty⟩
        ⟨revision + 1, .sealed key terminal⟩
  | replay
      (revision : Nat)
      (key : StableInvocationKey)
      (terminal : TerminalCapture) :
      SealCas
        ⟨revision, key, terminal⟩
        ⟨revision, .sealed key terminal⟩
        ⟨revision, .sealed key terminal⟩
  | conflict
      (revision : Nat)
      (key : StableInvocationKey)
      (accepted conflicting : TerminalCapture)
      (different : conflicting ≠ accepted) :
      SealCas
        ⟨revision, key, conflicting⟩
        ⟨revision, .sealed key accepted⟩
        ⟨revision + 1, .quarantined key accepted conflicting⟩

structure RecoverySnapshot where
  intent : DurableMaterializationIntent
  capture : CaptureLog
  sealHead : SealHead
  deriving DecidableEq, Repr

def crash (snapshot : RecoverySnapshot) : RecoverySnapshot := snapshot

def RetryPreservesExperiment
    (before after : RecoverySnapshot) : Prop :=
  after.intent = before.intent ∧
  AppendOnlyCapture before.capture after.capture

theorem persisted_intent_authorizes_matching_invocation
    (key : StableInvocationKey)
    (payload : Nat) :
    IntentAuthorizesInvocation
      ⟨key, payload, true⟩
      ⟨key, payload, true⟩ := by
  exact ⟨rfl, rfl, rfl, rfl⟩

theorem unpersisted_intent_cannot_authorize
    (intent : DurableMaterializationIntent)
    (invocation : MaterializationInvocation)
    (notPersisted : intent.persisted = false) :
    ¬ IntentAuthorizesInvocation intent invocation := by
  intro authorized
  exact Bool.false_ne_true (notPersisted.symm.trans authorized.1)

theorem wrong_invocation_key_cannot_authorize
    (intent : DurableMaterializationIntent)
    (invocation : MaterializationInvocation)
    (wrongKey : invocation.key ≠ intent.key) :
    ¬ IntentAuthorizesInvocation intent invocation := by
  intro authorized
  exact wrongKey authorized.2.2.1

theorem wrong_payload_cannot_authorize
    (intent : DurableMaterializationIntent)
    (invocation : MaterializationInvocation)
    (wrongPayload : invocation.payloadDigest ≠ intent.payloadDigest) :
    ¬ IntentAuthorizesInvocation intent invocation := by
  intro authorized
  exact wrongPayload authorized.2.2.2

theorem append_fragment_is_append_only
    (before : CaptureLog)
    (fragment : CaptureFragment) :
    AppendOnlyCapture before (before ++ [fragment]) := by
  exact ⟨[fragment], rfl⟩

theorem empty_capture_is_append_only :
    AppendOnlyCapture [] [] := by
  exact ⟨[], rfl⟩

theorem append_only_capture_preserves_existing_prefix
    {before after : CaptureLog}
    (appendOnly : AppendOnlyCapture before after) :
    ∃ suffix, after = before ++ suffix := by
  exact appendOnly

theorem first_seal_publishes_terminal
    (revision : Nat)
    (key : StableInvocationKey)
    (terminal : TerminalCapture) :
    SealCas ⟨revision, key, terminal⟩
      ⟨revision, .empty⟩
      ⟨revision + 1, .sealed key terminal⟩ := by
  exact SealCas.first revision key terminal

theorem identical_seal_retry_is_idempotent
    (revision : Nat)
    (key : StableInvocationKey)
    (terminal : TerminalCapture) :
    SealCas ⟨revision, key, terminal⟩
      ⟨revision, .sealed key terminal⟩
      ⟨revision, .sealed key terminal⟩ := by
  exact SealCas.replay revision key terminal

theorem conflicting_terminal_quarantines
    (revision : Nat)
    (key : StableInvocationKey)
    (accepted conflicting : TerminalCapture)
    (different : conflicting ≠ accepted) :
    SealCas ⟨revision, key, conflicting⟩
      ⟨revision, .sealed key accepted⟩
      ⟨revision + 1, .quarantined key accepted conflicting⟩ := by
  exact SealCas.conflict revision key accepted conflicting different

theorem stale_revision_cannot_first_seal
    (expected current : Nat)
    (key : StableInvocationKey)
    (terminal : TerminalCapture)
    (stale : expected ≠ current) :
    ¬ ∃ after, SealCas ⟨expected, key, terminal⟩
      ⟨current, .empty⟩ after := by
  intro witness
  obtain ⟨after, transition⟩ := witness
  cases transition
  exact stale rfl

theorem different_terminal_cannot_be_second_seal
    (key : StableInvocationKey)
    (accepted conflicting : TerminalCapture)
    (different : conflicting ≠ accepted) :
    SealSlot.sealed key conflicting ≠ SealSlot.sealed key accepted := by
  intro equal
  cases equal
  exact different rfl

theorem quarantined_slot_cannot_publish_seal
    (request : SealRequest)
    (revision : Nat)
    (key : StableInvocationKey)
    (accepted conflicting : TerminalCapture) :
    ¬ ∃ after, SealCas request
      ⟨revision, .quarantined key accepted conflicting⟩ after := by
  intro witness
  obtain ⟨after, transition⟩ := witness
  cases transition

theorem crash_preserves_recovery_snapshot
    (snapshot : RecoverySnapshot) :
    crash snapshot = snapshot := by
  rfl

theorem retry_preserves_durable_intent
    {before after : RecoverySnapshot}
    (stable : RetryPreservesExperiment before after) :
    after.intent = before.intent := by
  exact stable.1

theorem retry_capture_is_append_only
    {before after : RecoverySnapshot}
    (stable : RetryPreservesExperiment before after) :
    AppendOnlyCapture before.capture after.capture := by
  exact stable.2

theorem generation_change_changes_invocation_key
    (dispatchKey : DispatchKey)
    (invocationDigest oldGeneration newGeneration : Nat)
    (changed : oldGeneration ≠ newGeneration) :
    StableInvocationKey.mk dispatchKey invocationDigest oldGeneration ≠
      StableInvocationKey.mk dispatchKey invocationDigest newGeneration := by
  intro equal
  exact changed (congrArg StableInvocationKey.generation equal)

theorem same_key_different_terminal_forces_quarantine_target
    (revision : Nat)
    (key : StableInvocationKey)
    (accepted conflicting : TerminalCapture)
    (different : conflicting ≠ accepted) :
    ∃ after,
      SealCas ⟨revision, key, conflicting⟩
        ⟨revision, .sealed key accepted⟩ after ∧
      after.slot = .quarantined key accepted conflicting := by
  exact ⟨
    ⟨revision + 1, .quarantined key accepted conflicting⟩,
    SealCas.conflict revision key accepted conflicting different,
    rfl
  ⟩

theorem invoke_before_intent_is_rejected
    (key : StableInvocationKey)
    (payload : Nat) :
    ¬ IntentAuthorizesInvocation
      ⟨key, payload, false⟩
      ⟨key, payload, true⟩ := by
  exact unpersisted_intent_cannot_authorize _ _ rfl

end ASPProof.AgentSessionHostRegistryCrashSafeMaterializationRecovery
