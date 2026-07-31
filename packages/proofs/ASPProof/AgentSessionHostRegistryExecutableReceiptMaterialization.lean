import ASPProof.AgentSessionHostRegistryCrossRuntimeReplay

namespace ASPProof.AgentSessionHostRegistryExecutableReceiptMaterialization

open ASPProof.AgentSessionHostRegistryCrossRuntimeReplay

inductive MaterializationStage where
  | planned
  | resolved
  | invoked
  | captured
  | effectsObserved
  | sealed
  deriving DecidableEq, Repr

inductive LegalStageTransition : MaterializationStage → MaterializationStage → Prop where
  | resolve : LegalStageTransition .planned .resolved
  | invoke : LegalStageTransition .resolved .invoked
  | capture : LegalStageTransition .invoked .captured
  | observe : LegalStageTransition .captured .effectsObserved
  | seal : LegalStageTransition .effectsObserved .sealed

structure ArtifactResolutionEvidence where
  executionCommandDigest : Nat
  artifactDigest : Nat
  materializedPathDigest : Nat
  resolved : Bool
  deriving DecidableEq, Repr

structure VectorMaterializationEvidence where
  generatorDigest : Nat
  suiteDigest : Nat
  vectorBytesDigest : Nat
  immutable : Bool
  deriving DecidableEq, Repr

structure InvocationEvidence where
  invocationDigest : Nat
  executionCommandDigest : Nat
  artifactDigest : Nat
  vectorBytesDigest : Nat
  executorDigest : Nat
  started : Bool
  deriving DecidableEq, Repr

structure ChannelCaptureEvidence where
  invocationDigest : Nat
  stdoutDigest : Nat
  stderrDigest : Nat
  exitCode : Nat
  durationMs : Nat
  complete : Bool
  deriving DecidableEq, Repr

structure EffectObservationEvidence where
  invocationDigest : Nat
  effects : WarmPathEffects
  complete : Bool
  deriving DecidableEq, Repr

structure MaterializationPayload where
  stage : MaterializationStage
  artifact : ArtifactResolutionEvidence
  vectors : VectorMaterializationEvidence
  invocation : InvocationEvidence
  capture : ChannelCaptureEvidence
  observation : EffectObservationEvidence
  deriving DecidableEq, Repr

structure SealedMaterializationReceipt where
  payload : MaterializationPayload
  sealedPayloadDigest : Nat
  deriving DecidableEq, Repr

abbrev PayloadDigest := MaterializationPayload → Nat

def ExecutableReceiptMaterialized
    (expectedCommandDigest expectedArtifactDigest expectedSuiteDigest : Nat)
    (producerExecutorDigest : Nat)
    (digestPayload : PayloadDigest)
    (receipt : SealedMaterializationReceipt) : Prop :=
  receipt.payload.stage = .sealed ∧
  receipt.payload.artifact.resolved = true ∧
  receipt.payload.artifact.executionCommandDigest = expectedCommandDigest ∧
  receipt.payload.artifact.artifactDigest = expectedArtifactDigest ∧
  receipt.payload.vectors.immutable = true ∧
  receipt.payload.vectors.suiteDigest = expectedSuiteDigest ∧
  receipt.payload.invocation.started = true ∧
  receipt.payload.invocation.executionCommandDigest =
    receipt.payload.artifact.executionCommandDigest ∧
  receipt.payload.invocation.artifactDigest =
    receipt.payload.artifact.artifactDigest ∧
  receipt.payload.invocation.vectorBytesDigest =
    receipt.payload.vectors.vectorBytesDigest ∧
  receipt.payload.invocation.executorDigest ≠ producerExecutorDigest ∧
  receipt.payload.capture.complete = true ∧
  receipt.payload.capture.invocationDigest =
    receipt.payload.invocation.invocationDigest ∧
  receipt.payload.observation.complete = true ∧
  receipt.payload.observation.invocationDigest =
    receipt.payload.invocation.invocationDigest ∧
  EffectFree receipt.payload.observation.effects ∧
  receipt.sealedPayloadDigest = digestPayload receipt.payload

def RetryStable
    (original retry : MaterializationPayload) : Prop :=
  retry.artifact = original.artifact ∧
  retry.vectors = original.vectors ∧
  retry.invocation.invocationDigest = original.invocation.invocationDigest ∧
  retry.invocation.executionCommandDigest =
    original.invocation.executionCommandDigest ∧
  retry.invocation.artifactDigest = original.invocation.artifactDigest ∧
  retry.invocation.vectorBytesDigest = original.invocation.vectorBytesDigest ∧
  retry.invocation.executorDigest = original.invocation.executorDigest

theorem legal_transition_from_planned_is_resolution
    {next : MaterializationStage}
    (legal : LegalStageTransition .planned next) :
    next = .resolved := by
  cases legal
  rfl

theorem planned_cannot_skip_to_invoked :
    ¬ LegalStageTransition .planned .invoked := by
  intro legal
  cases legal

theorem resolved_cannot_skip_to_captured :
    ¬ LegalStageTransition .resolved .captured := by
  intro legal
  cases legal

theorem invoked_cannot_skip_effect_observation :
    ¬ LegalStageTransition .invoked .effectsObserved := by
  intro legal
  cases legal

theorem captured_cannot_skip_to_sealed :
    ¬ LegalStageTransition .captured .sealed := by
  intro legal
  cases legal

theorem materialized_receipt_is_sealed
    {command artifact suite producer : Nat}
    {digestPayload : PayloadDigest}
    {receipt : SealedMaterializationReceipt}
    (materialized : ExecutableReceiptMaterialized command artifact suite producer
      digestPayload receipt) :
    receipt.payload.stage = .sealed := by
  exact materialized.1

theorem materialized_receipt_binds_command
    {command artifact suite producer : Nat}
    {digestPayload : PayloadDigest}
    {receipt : SealedMaterializationReceipt}
    (materialized : ExecutableReceiptMaterialized command artifact suite producer
      digestPayload receipt) :
    receipt.payload.artifact.executionCommandDigest = command := by
  exact materialized.2.2.1

theorem materialized_receipt_binds_artifact
    {command artifact suite producer : Nat}
    {digestPayload : PayloadDigest}
    {receipt : SealedMaterializationReceipt}
    (materialized : ExecutableReceiptMaterialized command artifact suite producer
      digestPayload receipt) :
    receipt.payload.artifact.artifactDigest = artifact := by
  exact materialized.2.2.2.1

theorem materialized_receipt_binds_vector_suite
    {command artifact suite producer : Nat}
    {digestPayload : PayloadDigest}
    {receipt : SealedMaterializationReceipt}
    (materialized : ExecutableReceiptMaterialized command artifact suite producer
      digestPayload receipt) :
    receipt.payload.vectors.suiteDigest = suite := by
  exact materialized.2.2.2.2.2.1

theorem materialized_vectors_are_immutable
    {command artifact suite producer : Nat}
    {digestPayload : PayloadDigest}
    {receipt : SealedMaterializationReceipt}
    (materialized : ExecutableReceiptMaterialized command artifact suite producer
      digestPayload receipt) :
    receipt.payload.vectors.immutable = true := by
  exact materialized.2.2.2.2.1

theorem materialized_invocation_is_independent
    {command artifact suite producer : Nat}
    {digestPayload : PayloadDigest}
    {receipt : SealedMaterializationReceipt}
    (materialized : ExecutableReceiptMaterialized command artifact suite producer
      digestPayload receipt) :
    receipt.payload.invocation.executorDigest ≠ producer := by
  exact materialized.2.2.2.2.2.2.2.2.2.2.1

theorem materialized_capture_is_complete
    {command artifact suite producer : Nat}
    {digestPayload : PayloadDigest}
    {receipt : SealedMaterializationReceipt}
    (materialized : ExecutableReceiptMaterialized command artifact suite producer
      digestPayload receipt) :
    receipt.payload.capture.complete = true := by
  exact materialized.2.2.2.2.2.2.2.2.2.2.2.1

theorem materialized_observation_is_complete
    {command artifact suite producer : Nat}
    {digestPayload : PayloadDigest}
    {receipt : SealedMaterializationReceipt}
    (materialized : ExecutableReceiptMaterialized command artifact suite producer
      digestPayload receipt) :
    receipt.payload.observation.complete = true := by
  exact materialized.2.2.2.2.2.2.2.2.2.2.2.2.2.1

theorem materialized_receipt_is_effect_free
    {command artifact suite producer : Nat}
    {digestPayload : PayloadDigest}
    {receipt : SealedMaterializationReceipt}
    (materialized : ExecutableReceiptMaterialized command artifact suite producer
      digestPayload receipt) :
    EffectFree receipt.payload.observation.effects := by
  exact materialized.2.2.2.2.2.2.2.2.2.2.2.2.2.2.2.1

theorem materialized_receipt_seal_matches_payload
    {command artifact suite producer : Nat}
    {digestPayload : PayloadDigest}
    {receipt : SealedMaterializationReceipt}
    (materialized : ExecutableReceiptMaterialized command artifact suite producer
      digestPayload receipt) :
    receipt.sealedPayloadDigest = digestPayload receipt.payload := by
  exact materialized.2.2.2.2.2.2.2.2.2.2.2.2.2.2.2.2

theorem unresolved_artifact_cannot_materialize
    {command artifact suite producer : Nat}
    {digestPayload : PayloadDigest}
    {receipt : SealedMaterializationReceipt}
    (unresolved : receipt.payload.artifact.resolved = false) :
    ¬ ExecutableReceiptMaterialized command artifact suite producer
      digestPayload receipt := by
  intro materialized
  exact Bool.false_ne_true (unresolved.symm.trans materialized.2.1)

theorem mutable_vectors_cannot_materialize
    {command artifact suite producer : Nat}
    {digestPayload : PayloadDigest}
    {receipt : SealedMaterializationReceipt}
    (mutableVectors : receipt.payload.vectors.immutable = false) :
    ¬ ExecutableReceiptMaterialized command artifact suite producer
      digestPayload receipt := by
  intro materialized
  exact Bool.false_ne_true
    (mutableVectors.symm.trans materialized.2.2.2.2.1)

theorem incomplete_capture_cannot_materialize
    {command artifact suite producer : Nat}
    {digestPayload : PayloadDigest}
    {receipt : SealedMaterializationReceipt}
    (incomplete : receipt.payload.capture.complete = false) :
    ¬ ExecutableReceiptMaterialized command artifact suite producer
      digestPayload receipt := by
  intro materialized
  exact Bool.false_ne_true
    (incomplete.symm.trans
      materialized.2.2.2.2.2.2.2.2.2.2.2.1)

theorem incomplete_effect_observation_cannot_materialize
    {command artifact suite producer : Nat}
    {digestPayload : PayloadDigest}
    {receipt : SealedMaterializationReceipt}
    (incomplete : receipt.payload.observation.complete = false) :
    ¬ ExecutableReceiptMaterialized command artifact suite producer
      digestPayload receipt := by
  intro materialized
  exact Bool.false_ne_true
    (incomplete.symm.trans
      materialized.2.2.2.2.2.2.2.2.2.2.2.2.2.1)

theorem wrong_stage_cannot_materialize
    {command artifact suite producer : Nat}
    {digestPayload : PayloadDigest}
    {receipt : SealedMaterializationReceipt}
    (wrongStage : receipt.payload.stage ≠ .sealed) :
    ¬ ExecutableReceiptMaterialized command artifact suite producer
      digestPayload receipt := by
  intro materialized
  exact wrongStage materialized.1

theorem wrong_seal_cannot_materialize
    {command artifact suite producer : Nat}
    {digestPayload : PayloadDigest}
    {receipt : SealedMaterializationReceipt}
    (wrongSeal : receipt.sealedPayloadDigest ≠ digestPayload receipt.payload) :
    ¬ ExecutableReceiptMaterialized command artifact suite producer
      digestPayload receipt := by
  intro materialized
  exact wrongSeal
    materialized.2.2.2.2.2.2.2.2.2.2.2.2.2.2.2.2

theorem retry_stability_binds_invocation
    {original retry : MaterializationPayload}
    (stable : RetryStable original retry) :
    retry.invocation.invocationDigest = original.invocation.invocationDigest := by
  exact stable.2.2.1

theorem retry_stability_binds_artifact
    {original retry : MaterializationPayload}
    (stable : RetryStable original retry) :
    retry.artifact = original.artifact := by
  exact stable.1

theorem retry_stability_binds_vectors
    {original retry : MaterializationPayload}
    (stable : RetryStable original retry) :
    retry.vectors = original.vectors := by
  exact stable.2.1

theorem exit_zero_alone_is_not_complete_capture :
    let capture : ChannelCaptureEvidence := ⟨1, 2, 3, 0, 4, false⟩
    capture.exitCode = 0 ∧ capture.complete = false := by
  exact ⟨rfl, rfl⟩

end ASPProof.AgentSessionHostRegistryExecutableReceiptMaterialization
