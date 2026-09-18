-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.AgentSessionHostRegistrySemanticAbstractionDiscriminatorConformance

structure TerminalObservation where
  stdoutDigest : Nat
  stderrDigest : Nat
  exitCode : Nat
  effectDigest : Nat
  deriving DecidableEq

def ObservationallyEquivalent (left right : TerminalObservation) : Prop :=
  left.stdoutDigest = right.stdoutDigest ∧
  left.stderrDigest = right.stderrDigest ∧
  left.exitCode = right.exitCode ∧
  left.effectDigest = right.effectDigest

abbrev TerminalAbstraction (α : Type) := TerminalObservation → α

def AbstractionAdequate {α : Type} (abstract : TerminalAbstraction α) : Prop :=
  ∀ left right, abstract left = abstract right → ObservationallyEquivalent left right

def constantAbstraction : TerminalAbstraction Unit := fun _ => ()

abbrev SemanticsEncoding (α : Type) := TerminalObservation → α

def EncodingInjective {α : Type} (encode : SemanticsEncoding α) : Prop :=
  Function.Injective encode

def constantEncoding : SemanticsEncoding Unit := fun _ => ()

inductive DiscriminatorDecision where
  | equivalent
  | preferAccepted
  | preferConflicting
  | ambiguous
  deriving DecidableEq

abbrev ReferenceDiscriminator :=
  TerminalObservation → TerminalObservation → DiscriminatorDecision

def alwaysPreferAccepted : ReferenceDiscriminator := fun _ _ => .preferAccepted

structure DiscriminatorInvocationReceipt where
  artifactDigest : Nat
  inputDigest : Nat
  exitZero : Bool
  decoded : Bool
  responseDigest : Nat
  decision : DiscriminatorDecision

structure DiscriminatorReplayReceipt where
  artifactDigest : Nat
  inputDigest : Nat
  responseDigest : Nat
  decision : DiscriminatorDecision
  executorId : Nat
  replayed : Bool

structure ExecutableDiscriminatorConforms
    (reference : ReferenceDiscriminator)
    (accepted conflicting : TerminalObservation)
    (expectedArtifact expectedInput primaryExecutor : Nat)
    (invocation : DiscriminatorInvocationReceipt)
    (replay : DiscriminatorReplayReceipt) : Prop where
  invocationArtifact : invocation.artifactDigest = expectedArtifact
  invocationInput : invocation.inputDigest = expectedInput
  invocationExitZero : invocation.exitZero = true
  invocationDecoded : invocation.decoded = true
  invocationDecision : invocation.decision = reference accepted conflicting
  replayArtifact : replay.artifactDigest = expectedArtifact
  replayInput : replay.inputDigest = expectedInput
  replayResponse : replay.responseDigest = invocation.responseDigest
  replayDecision : replay.decision = invocation.decision
  independentExecutor : replay.executorId ≠ primaryExecutor
  replayed : replay.replayed = true

theorem adequate_abstraction_reflects_observational_equivalence
    {α : Type} {abstract : TerminalAbstraction α}
    (adequate : AbstractionAdequate abstract)
    {left right : TerminalObservation}
    (same : abstract left = abstract right) :
    ObservationallyEquivalent left right :=
  adequate left right same

theorem observational_equivalence_binds_all_relevant_fields
    {left right : TerminalObservation}
    (equivalent : ObservationallyEquivalent left right) :
    left.stdoutDigest = right.stdoutDigest ∧
    left.stderrDigest = right.stderrDigest ∧
    left.exitCode = right.exitCode ∧
    left.effectDigest = right.effectDigest :=
  equivalent

theorem constant_abstraction_is_not_adequate :
    ¬ AbstractionAdequate constantAbstraction := by
  intro adequate
  let left : TerminalObservation := ⟨1, 0, 0, 0⟩
  let right : TerminalObservation := ⟨2, 0, 0, 0⟩
  have equivalent : ObservationallyEquivalent left right :=
    adequate left right rfl
  exact (by decide : (1 : Nat) ≠ 2) equivalent.1

theorem injective_encoding_reflects_semantic_equality
    {α : Type} {encode : SemanticsEncoding α}
    (injective : EncodingInjective encode)
    {left right : TerminalObservation}
    (same : encode left = encode right) : left = right :=
  injective same

theorem constant_encoding_is_not_injective :
    ¬ EncodingInjective constantEncoding := by
  intro injective
  let left : TerminalObservation := ⟨1, 0, 0, 0⟩
  let right : TerminalObservation := ⟨2, 0, 0, 0⟩
  have same : left = right := injective rfl
  have digestSame : left.stdoutDigest = right.stdoutDigest :=
    congrArg TerminalObservation.stdoutDigest same
  exact (by decide : (1 : Nat) ≠ 2) digestSame

theorem every_reference_discriminator_is_total
    (reference : ReferenceDiscriminator)
    (accepted conflicting : TerminalObservation) :
    ∃ decision, reference accepted conflicting = decision :=
  ⟨reference accepted conflicting, rfl⟩

theorem conforming_discriminator_binds_artifact
    {reference accepted conflicting expectedArtifact expectedInput primaryExecutor invocation replay}
    (conforms : ExecutableDiscriminatorConforms reference accepted conflicting
      expectedArtifact expectedInput primaryExecutor invocation replay) :
    invocation.artifactDigest = expectedArtifact :=
  conforms.invocationArtifact

theorem conforming_discriminator_binds_input
    {reference accepted conflicting expectedArtifact expectedInput primaryExecutor invocation replay}
    (conforms : ExecutableDiscriminatorConforms reference accepted conflicting
      expectedArtifact expectedInput primaryExecutor invocation replay) :
    invocation.inputDigest = expectedInput :=
  conforms.invocationInput

theorem conforming_discriminator_requires_exit_zero
    {reference accepted conflicting expectedArtifact expectedInput primaryExecutor invocation replay}
    (conforms : ExecutableDiscriminatorConforms reference accepted conflicting
      expectedArtifact expectedInput primaryExecutor invocation replay) :
    invocation.exitZero = true :=
  conforms.invocationExitZero

theorem conforming_discriminator_matches_reference_decision
    {reference accepted conflicting expectedArtifact expectedInput primaryExecutor invocation replay}
    (conforms : ExecutableDiscriminatorConforms reference accepted conflicting
      expectedArtifact expectedInput primaryExecutor invocation replay) :
    invocation.decision = reference accepted conflicting :=
  conforms.invocationDecision

theorem conforming_replay_binds_artifact_input_output
    {reference accepted conflicting expectedArtifact expectedInput primaryExecutor invocation replay}
    (conforms : ExecutableDiscriminatorConforms reference accepted conflicting
      expectedArtifact expectedInput primaryExecutor invocation replay) :
    replay.artifactDigest = expectedArtifact ∧
    replay.inputDigest = expectedInput ∧
    replay.responseDigest = invocation.responseDigest :=
  ⟨conforms.replayArtifact, conforms.replayInput, conforms.replayResponse⟩

theorem conforming_replay_matches_decision
    {reference accepted conflicting expectedArtifact expectedInput primaryExecutor invocation replay}
    (conforms : ExecutableDiscriminatorConforms reference accepted conflicting
      expectedArtifact expectedInput primaryExecutor invocation replay) :
    replay.decision = invocation.decision :=
  conforms.replayDecision

theorem conforming_replay_uses_independent_executor
    {reference accepted conflicting expectedArtifact expectedInput primaryExecutor invocation replay}
    (conforms : ExecutableDiscriminatorConforms reference accepted conflicting
      expectedArtifact expectedInput primaryExecutor invocation replay) :
    replay.executorId ≠ primaryExecutor :=
  conforms.independentExecutor

theorem conforming_replay_is_marked_replayed
    {reference accepted conflicting expectedArtifact expectedInput primaryExecutor invocation replay}
    (conforms : ExecutableDiscriminatorConforms reference accepted conflicting
      expectedArtifact expectedInput primaryExecutor invocation replay) :
    replay.replayed = true :=
  conforms.replayed

theorem artifact_drift_rejects_discriminator
    {reference accepted conflicting expectedArtifact expectedInput primaryExecutor invocation replay}
    (drift : invocation.artifactDigest ≠ expectedArtifact) :
    ¬ ExecutableDiscriminatorConforms reference accepted conflicting
      expectedArtifact expectedInput primaryExecutor invocation replay := by
  intro conforms
  exact drift conforms.invocationArtifact

theorem input_drift_rejects_discriminator
    {reference accepted conflicting expectedArtifact expectedInput primaryExecutor invocation replay}
    (drift : invocation.inputDigest ≠ expectedInput) :
    ¬ ExecutableDiscriminatorConforms reference accepted conflicting
      expectedArtifact expectedInput primaryExecutor invocation replay := by
  intro conforms
  exact drift conforms.invocationInput

theorem reference_mismatch_rejects_discriminator
    {reference accepted conflicting expectedArtifact expectedInput primaryExecutor invocation replay}
    (mismatch : invocation.decision ≠ reference accepted conflicting) :
    ¬ ExecutableDiscriminatorConforms reference accepted conflicting
      expectedArtifact expectedInput primaryExecutor invocation replay := by
  intro conforms
  exact mismatch conforms.invocationDecision

theorem same_executor_rejects_discriminator_replay
    {reference accepted conflicting expectedArtifact expectedInput primaryExecutor invocation replay}
    (same : replay.executorId = primaryExecutor) :
    ¬ ExecutableDiscriminatorConforms reference accepted conflicting
      expectedArtifact expectedInput primaryExecutor invocation replay := by
  intro conforms
  exact conforms.independentExecutor same

theorem response_drift_rejects_discriminator_replay
    {reference accepted conflicting expectedArtifact expectedInput primaryExecutor invocation replay}
    (drift : replay.responseDigest ≠ invocation.responseDigest) :
    ¬ ExecutableDiscriminatorConforms reference accepted conflicting
      expectedArtifact expectedInput primaryExecutor invocation replay := by
  intro conforms
  exact drift conforms.replayResponse

theorem exit_zero_only_evidence_is_constructible :
    ∃ receipt : DiscriminatorInvocationReceipt, receipt.exitZero = true :=
  ⟨⟨0, 0, true, false, 0, .ambiguous⟩, rfl⟩

theorem always_prefer_accepted_disagrees_with_equivalent_reference :
    DiscriminatorDecision.preferAccepted ≠ .equivalent := by
  intro equality
  exact DiscriminatorDecision.noConfusion equality

end ASPProof.AgentSessionHostRegistrySemanticAbstractionDiscriminatorConformance
