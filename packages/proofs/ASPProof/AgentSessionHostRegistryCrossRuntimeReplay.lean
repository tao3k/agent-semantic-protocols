-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.AgentSessionHostRegistryVerifierAttestation

namespace ASPProof.AgentSessionHostRegistryCrossRuntimeReplay

open ASPProof.AgentSessionHostRegistryVerifierAttestation

structure ReplayOutcome where
  stdoutDigest : Nat
  stderrDigest : Nat
  exitCode : Nat
  accepted : Bool
  deriving DecidableEq, Repr

structure ReplayVector where
  vectorId : Nat
  inputDigest : Nat
  expected : ReplayOutcome
  deriving DecidableEq, Repr

abbrev ReplayExecutor := ReplayVector → ReplayOutcome

def SuiteConforms
    (vectors : List ReplayVector)
    (executor : ReplayExecutor) : Prop :=
  ∀ vector, vector ∈ vectors → executor vector = vector.expected

structure WarmPathEffects where
  providerLaunchCount : Nat
  persistentWriteCount : Nat
  addedAgentRoundCount : Nat
  deriving DecidableEq, Repr

def EffectFree (effects : WarmPathEffects) : Prop :=
  effects.providerLaunchCount = 0 ∧
  effects.persistentWriteCount = 0 ∧
  effects.addedAgentRoundCount = 0

structure CrossRuntimeReplayReceiptData where
  producerExecutorDigest : Nat
  replayExecutorDigest : Nat
  verifierArtifactDigest : Nat
  vectorSuiteDigest : Nat
  referenceOutcome : ReplayOutcome
  runtimeOutcome : ReplayOutcome
  stdoutDigest : Nat
  stderrDigest : Nat
  exitCode : Nat
  durationMs : Nat
  effects : WarmPathEffects
  deriving DecidableEq, Repr

def CrossRuntimeReplayAccepted
    (expectedArtifactDigest expectedVectorSuiteDigest : Nat)
    (receipt : CrossRuntimeReplayReceiptData) : Prop :=
  receipt.producerExecutorDigest ≠ receipt.replayExecutorDigest ∧
  receipt.verifierArtifactDigest = expectedArtifactDigest ∧
  receipt.vectorSuiteDigest = expectedVectorSuiteDigest ∧
  receipt.referenceOutcome = receipt.runtimeOutcome ∧
  receipt.stdoutDigest = receipt.runtimeOutcome.stdoutDigest ∧
  receipt.stderrDigest = receipt.runtimeOutcome.stderrDigest ∧
  receipt.exitCode = receipt.runtimeOutcome.exitCode ∧
  EffectFree receipt.effects

def WithinTimingBound
    (maximumDurationMs : Nat)
    (receipt : CrossRuntimeReplayReceiptData) : Prop :=
  receipt.durationMs ≤ maximumDurationMs

def zeroEffects : WarmPathEffects := ⟨0, 0, 0⟩

def successfulOutcome (stdoutDigest stderrDigest : Nat) : ReplayOutcome :=
  ⟨stdoutDigest, stderrDigest, 0, true⟩

theorem suite_conformance_replays_declared_vector
    {vectors : List ReplayVector}
    {executor : ReplayExecutor}
    (conforms : SuiteConforms vectors executor)
    (vector : ReplayVector)
    (member : vector ∈ vectors) :
    executor vector = vector.expected := by
  exact conforms vector member

theorem zero_effects_are_effect_free : EffectFree zeroEffects := by
  exact ⟨rfl, rfl, rfl⟩

theorem accepted_replay_uses_independent_executor
    {artifactDigest vectorDigest : Nat}
    {receipt : CrossRuntimeReplayReceiptData}
    (accepted : CrossRuntimeReplayAccepted artifactDigest vectorDigest receipt) :
    receipt.producerExecutorDigest ≠ receipt.replayExecutorDigest := by
  exact accepted.1

theorem accepted_replay_binds_artifact
    {artifactDigest vectorDigest : Nat}
    {receipt : CrossRuntimeReplayReceiptData}
    (accepted : CrossRuntimeReplayAccepted artifactDigest vectorDigest receipt) :
    receipt.verifierArtifactDigest = artifactDigest := by
  exact accepted.2.1

theorem accepted_replay_binds_vector_suite
    {artifactDigest vectorDigest : Nat}
    {receipt : CrossRuntimeReplayReceiptData}
    (accepted : CrossRuntimeReplayAccepted artifactDigest vectorDigest receipt) :
    receipt.vectorSuiteDigest = vectorDigest := by
  exact accepted.2.2.1

theorem accepted_replay_matches_reference_semantics
    {artifactDigest vectorDigest : Nat}
    {receipt : CrossRuntimeReplayReceiptData}
    (accepted : CrossRuntimeReplayAccepted artifactDigest vectorDigest receipt) :
    receipt.referenceOutcome = receipt.runtimeOutcome := by
  exact accepted.2.2.2.1

theorem accepted_replay_binds_stdout
    {artifactDigest vectorDigest : Nat}
    {receipt : CrossRuntimeReplayReceiptData}
    (accepted : CrossRuntimeReplayAccepted artifactDigest vectorDigest receipt) :
    receipt.stdoutDigest = receipt.runtimeOutcome.stdoutDigest := by
  exact accepted.2.2.2.2.1

theorem accepted_replay_binds_stderr
    {artifactDigest vectorDigest : Nat}
    {receipt : CrossRuntimeReplayReceiptData}
    (accepted : CrossRuntimeReplayAccepted artifactDigest vectorDigest receipt) :
    receipt.stderrDigest = receipt.runtimeOutcome.stderrDigest := by
  exact accepted.2.2.2.2.2.1

theorem accepted_replay_binds_exit_code
    {artifactDigest vectorDigest : Nat}
    {receipt : CrossRuntimeReplayReceiptData}
    (accepted : CrossRuntimeReplayAccepted artifactDigest vectorDigest receipt) :
    receipt.exitCode = receipt.runtimeOutcome.exitCode := by
  exact accepted.2.2.2.2.2.2.1

theorem accepted_replay_is_warm_effect_free
    {artifactDigest vectorDigest : Nat}
    {receipt : CrossRuntimeReplayReceiptData}
    (accepted : CrossRuntimeReplayAccepted artifactDigest vectorDigest receipt) :
    EffectFree receipt.effects := by
  exact accepted.2.2.2.2.2.2.2

theorem accepted_replay_launches_no_provider
    {artifactDigest vectorDigest : Nat}
    {receipt : CrossRuntimeReplayReceiptData}
    (accepted : CrossRuntimeReplayAccepted artifactDigest vectorDigest receipt) :
    receipt.effects.providerLaunchCount = 0 := by
  exact (accepted_replay_is_warm_effect_free accepted).1

theorem accepted_replay_performs_no_persistent_write
    {artifactDigest vectorDigest : Nat}
    {receipt : CrossRuntimeReplayReceiptData}
    (accepted : CrossRuntimeReplayAccepted artifactDigest vectorDigest receipt) :
    receipt.effects.persistentWriteCount = 0 := by
  exact (accepted_replay_is_warm_effect_free accepted).2.1

theorem accepted_replay_adds_no_agent_round
    {artifactDigest vectorDigest : Nat}
    {receipt : CrossRuntimeReplayReceiptData}
    (accepted : CrossRuntimeReplayAccepted artifactDigest vectorDigest receipt) :
    receipt.effects.addedAgentRoundCount = 0 := by
  exact (accepted_replay_is_warm_effect_free accepted).2.2

theorem same_executor_replay_is_rejected
    (executor artifact vector stdout stderr duration : Nat) :
    ¬ CrossRuntimeReplayAccepted artifact vector {
      producerExecutorDigest := executor
      replayExecutorDigest := executor
      verifierArtifactDigest := artifact
      vectorSuiteDigest := vector
      referenceOutcome := successfulOutcome stdout stderr
      runtimeOutcome := successfulOutcome stdout stderr
      stdoutDigest := stdout
      stderrDigest := stderr
      exitCode := 0
      durationMs := duration
      effects := zeroEffects
    } := by
  intro accepted
  exact accepted.1 rfl

theorem exit_zero_does_not_imply_semantic_conformance :
    let reference := successfulOutcome 11 0
    let runtime := successfulOutcome 12 0
    reference.exitCode = runtime.exitCode ∧ reference ≠ runtime := by
  decide

theorem stdout_match_does_not_hide_stderr_mismatch :
    let reference := successfulOutcome 11 3
    let runtime := successfulOutcome 11 4
    reference.stdoutDigest = runtime.stdoutDigest ∧ reference ≠ runtime := by
  decide

theorem wrong_artifact_replay_is_rejected
    (artifact vector stdout stderr duration : Nat) :
    ¬ CrossRuntimeReplayAccepted artifact vector {
      producerExecutorDigest := 1
      replayExecutorDigest := 2
      verifierArtifactDigest := artifact + 1
      vectorSuiteDigest := vector
      referenceOutcome := successfulOutcome stdout stderr
      runtimeOutcome := successfulOutcome stdout stderr
      stdoutDigest := stdout
      stderrDigest := stderr
      exitCode := 0
      durationMs := duration
      effects := zeroEffects
    } := by
  intro accepted
  exact (Nat.ne_of_gt (Nat.lt_succ_self artifact)) accepted.2.1

theorem wrong_vector_suite_replay_is_rejected
    (artifact vector stdout stderr duration : Nat) :
    ¬ CrossRuntimeReplayAccepted artifact vector {
      producerExecutorDigest := 1
      replayExecutorDigest := 2
      verifierArtifactDigest := artifact
      vectorSuiteDigest := vector + 1
      referenceOutcome := successfulOutcome stdout stderr
      runtimeOutcome := successfulOutcome stdout stderr
      stdoutDigest := stdout
      stderrDigest := stderr
      exitCode := 0
      durationMs := duration
      effects := zeroEffects
    } := by
  intro accepted
  exact (Nat.ne_of_gt (Nat.lt_succ_self vector)) accepted.2.2.1

theorem provider_launch_breaks_warm_path
    (effects : WarmPathEffects)
    (launched : effects.providerLaunchCount ≠ 0) :
    ¬ EffectFree effects := by
  intro effectFree
  exact launched effectFree.1

theorem persistent_write_breaks_warm_path
    (effects : WarmPathEffects)
    (wrote : effects.persistentWriteCount ≠ 0) :
    ¬ EffectFree effects := by
  intro effectFree
  exact wrote effectFree.2.1

theorem agent_round_breaks_warm_path
    (effects : WarmPathEffects)
    (addedRound : effects.addedAgentRoundCount ≠ 0) :
    ¬ EffectFree effects := by
  intro effectFree
  exact addedRound effectFree.2.2

theorem fast_execution_does_not_imply_conformance
    (artifact vector : Nat) :
    let receipt : CrossRuntimeReplayReceiptData := {
      producerExecutorDigest := 1
      replayExecutorDigest := 1
      verifierArtifactDigest := artifact
      vectorSuiteDigest := vector
      referenceOutcome := successfulOutcome 10 0
      runtimeOutcome := successfulOutcome 10 0
      stdoutDigest := 10
      stderrDigest := 0
      exitCode := 0
      durationMs := 0
      effects := zeroEffects
    }
    WithinTimingBound 0 receipt ∧
      ¬ CrossRuntimeReplayAccepted artifact vector receipt := by
  constructor
  · exact Nat.le_refl 0
  · intro accepted
    exact accepted.1 rfl

theorem timing_bound_is_parametric
    (maximumDuration duration : Nat)
    (bounded : duration ≤ maximumDuration) :
    WithinTimingBound maximumDuration {
      producerExecutorDigest := 1
      replayExecutorDigest := 2
      verifierArtifactDigest := 3
      vectorSuiteDigest := 4
      referenceOutcome := successfulOutcome 5 0
      runtimeOutcome := successfulOutcome 5 0
      stdoutDigest := 5
      stderrDigest := 0
      exitCode := 0
      durationMs := duration
      effects := zeroEffects
    } := by
  exact bounded

end ASPProof.AgentSessionHostRegistryCrossRuntimeReplay
