import ASPProof.SearchRouteShortcutEnvelopeAdmission
import ASPProof.SearchRouteCanonicalPayloadDigestReplay
import ASPProof.SearchRouteSafeReplayModeNegotiation

namespace ASPProof.SearchRouteEndToEndReplayStateMachine

open ASPProof.SearchRouteFiniteMultiobjectiveParetoMaskWitness
open ASPProof.SearchRouteShortcutEnvelopeAdmission
open ASPProof.SearchRouteCanonicalPayloadDigestReplay
open ASPProof.SearchRouteSafeReplayModeNegotiation

def canonicalPayloadOfRaw
    (raw : RawShortcutEnvelope) : CanonicalShortcutPayload := {
  schemaVersion := raw.schemaVersion
  hashAlgorithmId := raw.hashAlgorithmId
  commitment := raw.commitment
  directDominanceClaimed := raw.claimedDirectDominance
}

structure ReplayContext where
  policy : ShortcutAdmissionPolicy
  start : Candidate
  endpoint : Candidate
  digest : DigestFunction
  admittedPayload : CanonicalShortcutPayload → Prop
  expectedPayload : CanonicalShortcutPayload
  expectedPayloadAdmitted : admittedPayload expectedPayload

structure NegotiatedReplay (context : ReplayContext) : Type where
  verified : VerifiedCachedShortcut context.policy context.start context.endpoint
  cachedPayloadAdmitted :
    context.admittedPayload (canonicalPayloadOfRaw verified.raw)
  collisionEvidence :
    Option (CollisionFreeEvidence context.digest context.admittedPayload)
  digestEquality :
    Option
      (DigestEqualityEvidence
        context.digest
        (canonicalPayloadOfRaw verified.raw)
        context.expectedPayload)
  canonicalEquality :
    Option
      (CanonicalEqualityEvidence
        (canonicalPayloadOfRaw verified.raw)
        context.expectedPayload)
  outcome :
    ReplayIdentityOutcome
      (canonicalPayloadOfRaw verified.raw)
      context.expectedPayload
  outcomeIsNegotiated :
    outcome =
      negotiateReplayIdentity
        context.digest
        context.admittedPayload
        (canonicalPayloadOfRaw verified.raw)
        context.expectedPayload
        cachedPayloadAdmitted
        context.expectedPayloadAdmitted
        collisionEvidence
        digestEquality
        canonicalEquality

structure AcceptedReplay (context : ReplayContext) : Type where
  negotiated : NegotiatedReplay context
  accepted :
    outcomeMode negotiated.outcome ≠ ReplayModeTag.explicitFieldFallback

theorem AcceptedReplay.payloadEqual
    {context : ReplayContext}
    (replay : AcceptedReplay context) :
    canonicalPayloadOfRaw replay.negotiated.verified.raw =
      context.expectedPayload :=
  accepted_outcome_implies_payload_equality
    (canonicalPayloadOfRaw replay.negotiated.verified.raw)
    context.expectedPayload
    replay.negotiated.outcome
    replay.accepted

inductive ReplayPipelineState (context : ReplayContext) : Type where
  | raw (envelope : RawShortcutEnvelope)
  | admitted
      (verified :
        VerifiedCachedShortcut context.policy context.start context.endpoint)
  | negotiated (replay : NegotiatedReplay context)
  | accepted (replay : AcceptedReplay context)
  | explicitFieldFallback
      (replay : NegotiatedReplay context)
      (fallbackSelected :
        outcomeMode replay.outcome = ReplayModeTag.explicitFieldFallback)

inductive ReplayTransition
    (context : ReplayContext) :
    ReplayPipelineState context → ReplayPipelineState context → Prop where
  | admit
      (raw : RawShortcutEnvelope)
      (admission :
        AdmissionWitness context.policy raw context.start context.endpoint) :
      ReplayTransition
        context
        (.raw raw)
        (.admitted { raw := raw, admission := admission })
  | negotiate (replay : NegotiatedReplay context) :
      ReplayTransition
        context
        (.admitted replay.verified)
        (.negotiated replay)
  | accept
      (replay : NegotiatedReplay context)
      (accepted :
        outcomeMode replay.outcome ≠ ReplayModeTag.explicitFieldFallback) :
      ReplayTransition
        context
        (.negotiated replay)
        (.accepted { negotiated := replay, accepted := accepted })
  | fallback
      (replay : NegotiatedReplay context)
      (fallbackSelected :
        outcomeMode replay.outcome = ReplayModeTag.explicitFieldFallback) :
      ReplayTransition
        context
        (.negotiated replay)
        (.explicitFieldFallback replay fallbackSelected)

inductive ReplayPath
    (context : ReplayContext) :
    Nat → ReplayPipelineState context → ReplayPipelineState context → Prop where
  | nil (state : ReplayPipelineState context) :
      ReplayPath context 0 state state
  | cons
      {length : Nat}
      {start middle finish : ReplayPipelineState context}
      (first : ReplayTransition context start middle)
      (rest : ReplayPath context length middle finish) :
      ReplayPath context (length + 1) start finish

def pipelineStageRank
    {context : ReplayContext} : ReplayPipelineState context → Nat
  | .raw _ => 0
  | .admitted _ => 1
  | .negotiated _ => 2
  | .accepted _ => 3
  | .explicitFieldFallback _ _ => 3

theorem transition_increments_stage_rank
    {context : ReplayContext}
    {start finish : ReplayPipelineState context}
    (transition : ReplayTransition context start finish) :
    pipelineStageRank finish = pipelineStageRank start + 1 := by
  cases transition <;> rfl

theorem add_one_then_length
    (startRank length : Nat) :
    (startRank + 1) + length = startRank + (length + 1) := by
  rw [Nat.add_assoc]
  exact congrArg (fun suffix => startRank + suffix) (Nat.add_comm 1 length)

theorem path_stage_rank_accounting
    {context : ReplayContext}
    {length : Nat}
    {start finish : ReplayPipelineState context}
    (path : ReplayPath context length start finish) :
    pipelineStageRank finish = pipelineStageRank start + length := by
  induction path with
  | nil => rfl
  | cons first rest inductionHypothesis =>
      exact Eq.trans
        inductionHypothesis
        (Eq.trans
          (congrArg
            (fun middleRank => middleRank + _)
            (transition_increments_stage_rank first))
          (add_one_then_length _ _))

theorem raw_to_accepted_path_has_exact_length
    {context : ReplayContext}
    {length : Nat}
    (raw : RawShortcutEnvelope)
    (replay : AcceptedReplay context)
    (path :
      ReplayPath context length (.raw raw) (.accepted replay)) :
    length = 3 := by
  have accounting := path_stage_rank_accounting path
  simpa [pipelineStageRank] using accounting.symm

theorem raw_to_fallback_path_has_exact_length
    {context : ReplayContext}
    {length : Nat}
    (raw : RawShortcutEnvelope)
    (replay : NegotiatedReplay context)
    (fallbackSelected :
      outcomeMode replay.outcome = ReplayModeTag.explicitFieldFallback)
    (path :
      ReplayPath
        context
        length
        (.raw raw)
        (.explicitFieldFallback replay fallbackSelected)) :
    length = 3 := by
  have accounting := path_stage_rank_accounting path
  simpa [pipelineStageRank] using accounting.symm

theorem canonicalAcceptedPath
    {context : ReplayContext}
    (replay : AcceptedReplay context) :
    ReplayPath
      context
      3
      (.raw replay.negotiated.verified.raw)
      (.accepted replay) :=
  .cons
    (.admit
      replay.negotiated.verified.raw
      replay.negotiated.verified.admission)
    (.cons
      (.negotiate replay.negotiated)
      (.cons
        (.accept replay.negotiated replay.accepted)
        (.nil (.accepted replay))))

theorem no_direct_raw_to_accepted_transition
    {context : ReplayContext}
    (raw : RawShortcutEnvelope)
    (replay : AcceptedReplay context) :
    ¬ ReplayTransition context (.raw raw) (.accepted replay) := by
  intro transition
  cases transition

theorem no_direct_admitted_to_accepted_transition
    {context : ReplayContext}
    (verified :
      VerifiedCachedShortcut context.policy context.start context.endpoint)
    (replay : AcceptedReplay context) :
    ¬ ReplayTransition context (.admitted verified) (.accepted replay) := by
  intro transition
  cases transition

def acceptedReplayAdmissionWitness
    {context : ReplayContext}
    (replay : AcceptedReplay context) :
    AdmissionWitness
      context.policy
      replay.negotiated.verified.raw
      context.start
      context.endpoint :=
  replay.negotiated.verified.admission

theorem accepted_replay_contains_negotiated_identity
    {context : ReplayContext}
    (replay : AcceptedReplay context) :
    replay.negotiated.outcome =
      negotiateReplayIdentity
        context.digest
        context.admittedPayload
        (canonicalPayloadOfRaw replay.negotiated.verified.raw)
        context.expectedPayload
        replay.negotiated.cachedPayloadAdmitted
        context.expectedPayloadAdmitted
        replay.negotiated.collisionEvidence
        replay.negotiated.digestEquality
        replay.negotiated.canonicalEquality :=
  replay.negotiated.outcomeIsNegotiated

structure AcceptedReplayGateClosure
    (context : ReplayContext)
    (replay : AcceptedReplay context) : Type where
  admission :
    AdmissionWitness
      context.policy
      replay.negotiated.verified.raw
      context.start
      context.endpoint
  payloadEqual :
    canonicalPayloadOfRaw replay.negotiated.verified.raw =
      context.expectedPayload

def acceptedReplayGateClosure
    {context : ReplayContext}
    (replay : AcceptedReplay context) :
    AcceptedReplayGateClosure context replay := {
  admission := acceptedReplayAdmissionWitness replay
  payloadEqual := replay.payloadEqual
}

theorem fallback_is_not_accepted
    {context : ReplayContext}
    (replay : NegotiatedReplay context)
    (fallbackSelected :
      outcomeMode replay.outcome = ReplayModeTag.explicitFieldFallback) :
    ¬ outcomeMode replay.outcome ≠ ReplayModeTag.explicitFieldFallback := by
  intro accepted
  exact accepted fallbackSelected

structure ReplayCoreCost where
  graphTransitions : Nat
  providerToolTurns : Nat
  receiptTokens : Nat
  deriving DecidableEq, Repr

structure CacheCostCredits where
  searchWorkUnitsAvoided : Nat
  modelPrefixTokensReused : Nat
  deriving DecidableEq, Repr

structure ReplayPerformanceReceipt where
  core : ReplayCoreCost
  cacheCredits : CacheCostCredits
  deriving DecidableEq, Repr

def StrictlyImprovesCore
    (shortcut baseline : ReplayCoreCost) : Prop :=
  shortcut.graphTransitions < baseline.graphTransitions ∧
    shortcut.providerToolTurns < baseline.providerToolTurns ∧
    shortcut.receiptTokens < baseline.receiptTokens

theorem componentwise_cost_reduction_is_strict_improvement
    (shortcut baseline : ReplayCoreCost)
    (graphReduced :
      shortcut.graphTransitions < baseline.graphTransitions)
    (turnsReduced :
      shortcut.providerToolTurns < baseline.providerToolTurns)
    (tokensReduced :
      shortcut.receiptTokens < baseline.receiptTokens) :
    StrictlyImprovesCore shortcut baseline :=
  ⟨graphReduced, turnsReduced, tokensReduced⟩

def canonicalDigestShortcutCost : ReplayCoreCost := {
  graphTransitions := 3
  providerToolTurns := 2
  receiptTokens :=
    replayIdentityReceiptTokenCost ReplayModeTag.digestCompressed
}

def illustrativeExplicitReplayCost : ReplayCoreCost := {
  graphTransitions := 6
  providerToolTurns := 4
  receiptTokens :=
    replayIdentityReceiptTokenCost ReplayModeTag.fullField
}

theorem illustrative_digest_shortcut_strictly_improves_core :
    StrictlyImprovesCore
      canonicalDigestShortcutCost
      illustrativeExplicitReplayCost := by
  change 3 < 6 ∧ 2 < 4 ∧ 3 < 12
  decide

theorem accepted_identity_is_independent_of_cache_credits
    {context : ReplayContext}
    (replay : AcceptedReplay context)
    (_cacheCredits : CacheCostCredits) :
    canonicalPayloadOfRaw replay.negotiated.verified.raw =
      context.expectedPayload :=
  replay.payloadEqual

theorem cache_credits_do_not_change_core_cost
    (core : ReplayCoreCost)
    (leftCredits rightCredits : CacheCostCredits) :
    (ReplayPerformanceReceipt.mk core leftCredits).core =
      (ReplayPerformanceReceipt.mk core rightCredits).core := by
  rfl

end ASPProof.SearchRouteEndToEndReplayStateMachine
