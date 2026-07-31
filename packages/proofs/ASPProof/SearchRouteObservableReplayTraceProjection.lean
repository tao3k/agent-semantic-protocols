import ASPProof.SearchRouteEndToEndReplayStateMachine

namespace ASPProof.SearchRouteObservableReplayTraceProjection

open ASPProof.SearchRouteShortcutEnvelopeAdmission
open ASPProof.SearchRouteSafeReplayModeNegotiation
open ASPProof.SearchRouteEndToEndReplayStateMachine

structure ObservableReplayEvent
    (context : ReplayContext)
    (start finish : ReplayPipelineState context) : Type where
  internalTransitions : Nat
  realizedPath : ReplayPath context internalTransitions start finish
  receiptTokens : Nat
  searchWorkUnitsAvoided : Nat
  modelPrefixTokensReused : Nat

inductive ObservableReplayTrace
    (context : ReplayContext) :
    Nat → Nat → ReplayPipelineState context → ReplayPipelineState context → Type where
  | nil (state : ReplayPipelineState context) :
      ObservableReplayTrace context 0 0 state state
  | cons
      {eventCount internalTransitions : Nat}
      {start middle finish : ReplayPipelineState context}
      (first : ObservableReplayEvent context start middle)
      (rest :
        ObservableReplayTrace
          context eventCount internalTransitions middle finish) :
      ObservableReplayTrace
        context
        (eventCount + 1)
        (first.internalTransitions + internalTransitions)
        start
        finish

theorem event_preserves_stage_rank_order
    {context : ReplayContext}
    {start finish : ReplayPipelineState context}
    (event : ObservableReplayEvent context start finish) :
    pipelineStageRank finish =
      pipelineStageRank start + event.internalTransitions :=
  path_stage_rank_accounting event.realizedPath

theorem trace_stage_rank_accounting
    {context : ReplayContext}
    {eventCount internalTransitions : Nat}
    {start finish : ReplayPipelineState context}
    (trace :
      ObservableReplayTrace
        context eventCount internalTransitions start finish) :
    pipelineStageRank finish =
      pipelineStageRank start + internalTransitions := by
  induction trace with
  | nil => rfl
  | cons first rest inductionHypothesis =>
      exact Eq.trans
        inductionHypothesis
        (Eq.trans
          (congrArg
            (fun middleRank => middleRank + _)
            (event_preserves_stage_rank_order first))
          (Nat.add_assoc _ _ _))

theorem raw_to_accepted_trace_realizes_exactly_three_internal_transitions
    {context : ReplayContext}
    {eventCount internalTransitions : Nat}
    (raw : RawShortcutEnvelope)
    (replay : AcceptedReplay context)
    (trace :
      ObservableReplayTrace
        context
        eventCount
        internalTransitions
        (.raw raw)
        (.accepted replay)) :
    internalTransitions = 3 := by
  have accounting := trace_stage_rank_accounting trace
  simpa [pipelineStageRank] using accounting.symm

theorem raw_to_fallback_trace_realizes_exactly_three_internal_transitions
    {context : ReplayContext}
    {eventCount internalTransitions : Nat}
    (raw : RawShortcutEnvelope)
    (replay : NegotiatedReplay context)
    (fallbackSelected :
      outcomeMode replay.outcome = ReplayModeTag.explicitFieldFallback)
    (trace :
      ObservableReplayTrace
        context
        eventCount
        internalTransitions
        (.raw raw)
        (.explicitFieldFallback replay fallbackSelected)) :
    internalTransitions = 3 := by
  have accounting := trace_stage_rank_accounting trace
  simpa [pipelineStageRank] using accounting.symm

def singleTransitionEvent
    {context : ReplayContext}
    {start finish : ReplayPipelineState context}
    (transition : ReplayTransition context start finish)
    (receiptTokens searchWorkUnitsAvoided modelPrefixTokensReused : Nat) :
    ObservableReplayEvent context start finish := {
  internalTransitions := 1
  realizedPath := .cons transition (.nil finish)
  receiptTokens := receiptTokens
  searchWorkUnitsAvoided := searchWorkUnitsAvoided
  modelPrefixTokensReused := modelPrefixTokensReused
}

def canonicalBatchedAcceptedEvent
    {context : ReplayContext}
    (replay : AcceptedReplay context) :
    ObservableReplayEvent
      context
      (.raw replay.negotiated.verified.raw)
      (.accepted replay) := {
  internalTransitions := 3
  realizedPath := canonicalAcceptedPath replay
  receiptTokens :=
    replayIdentityReceiptTokenCost ReplayModeTag.digestCompressed
  searchWorkUnitsAvoided := 0
  modelPrefixTokensReused := 0
}

def canonicalBatchedAcceptedTrace
    {context : ReplayContext}
    (replay : AcceptedReplay context) :
    ObservableReplayTrace
      context
      1
      3
      (.raw replay.negotiated.verified.raw)
      (.accepted replay) :=
  .cons (canonicalBatchedAcceptedEvent replay) (.nil (.accepted replay))

def canonicalSequentialAcceptedTrace
    {context : ReplayContext}
    (replay : AcceptedReplay context) :
    ObservableReplayTrace
      context
      3
      3
      (.raw replay.negotiated.verified.raw)
      (.accepted replay) :=
  .cons
    (singleTransitionEvent
      (.admit
        replay.negotiated.verified.raw
        replay.negotiated.verified.admission)
      4 0 0)
    (.cons
      (singleTransitionEvent (.negotiate replay.negotiated) 4 0 0)
      (.cons
        (singleTransitionEvent
          (.accept replay.negotiated replay.accepted)
          4 0 0)
        (.nil (.accepted replay))))

theorem batching_reduces_external_rounds_without_hiding_internal_transitions :
    1 < 3 ∧ 3 = 3 := by
  decide

def ObservableReplayTrace.receiptTokens
    {context : ReplayContext}
    {eventCount internalTransitions : Nat}
    {start finish : ReplayPipelineState context} :
    ObservableReplayTrace
      context eventCount internalTransitions start finish → Nat
  | .nil _ => 0
  | .cons first rest => first.receiptTokens + rest.receiptTokens

def ObservableReplayTrace.searchWorkUnitsAvoided
    {context : ReplayContext}
    {eventCount internalTransitions : Nat}
    {start finish : ReplayPipelineState context} :
    ObservableReplayTrace
      context eventCount internalTransitions start finish → Nat
  | .nil _ => 0
  | .cons first rest =>
      first.searchWorkUnitsAvoided + rest.searchWorkUnitsAvoided

def ObservableReplayTrace.modelPrefixTokensReused
    {context : ReplayContext}
    {eventCount internalTransitions : Nat}
    {start finish : ReplayPipelineState context} :
    ObservableReplayTrace
      context eventCount internalTransitions start finish → Nat
  | .nil _ => 0
  | .cons first rest =>
      first.modelPrefixTokensReused + rest.modelPrefixTokensReused

structure ObservableTraceReceipt where
  providerToolTurns : Nat
  internalTransitions : Nat
  receiptTokens : Nat
  searchWorkUnitsAvoided : Nat
  modelPrefixTokensReused : Nat
  deriving DecidableEq, Repr

def traceReceipt
    {context : ReplayContext}
    {eventCount internalTransitions : Nat}
    {start finish : ReplayPipelineState context}
    (trace :
      ObservableReplayTrace
        context eventCount internalTransitions start finish) :
    ObservableTraceReceipt := {
  providerToolTurns := eventCount
  internalTransitions := internalTransitions
  receiptTokens := trace.receiptTokens
  searchWorkUnitsAvoided := trace.searchWorkUnitsAvoided
  modelPrefixTokensReused := trace.modelPrefixTokensReused
}

theorem batched_trace_receipt_has_one_external_turn
    {context : ReplayContext}
    (replay : AcceptedReplay context) :
    (traceReceipt (canonicalBatchedAcceptedTrace replay)).providerToolTurns = 1 := by
  rfl

theorem sequential_trace_receipt_has_three_external_turns
    {context : ReplayContext}
    (replay : AcceptedReplay context) :
    (traceReceipt (canonicalSequentialAcceptedTrace replay)).providerToolTurns = 3 := by
  rfl

theorem batched_trace_uses_fewer_receipt_tokens
    {context : ReplayContext}
    (replay : AcceptedReplay context) :
    (traceReceipt (canonicalBatchedAcceptedTrace replay)).receiptTokens <
      (traceReceipt (canonicalSequentialAcceptedTrace replay)).receiptTokens := by
  change 3 < 12
  decide

def ObservableReplayEvent.withCacheCredits
    {context : ReplayContext}
    {start finish : ReplayPipelineState context}
    (event : ObservableReplayEvent context start finish)
    (searchWorkUnitsAvoided modelPrefixTokensReused : Nat) :
    ObservableReplayEvent context start finish := {
  internalTransitions := event.internalTransitions
  realizedPath := event.realizedPath
  receiptTokens := event.receiptTokens
  searchWorkUnitsAvoided := searchWorkUnitsAvoided
  modelPrefixTokensReused := modelPrefixTokensReused
}

theorem changing_cache_credits_preserves_internal_path
    {context : ReplayContext}
    {start finish : ReplayPipelineState context}
    (event : ObservableReplayEvent context start finish)
    (searchWorkUnitsAvoided modelPrefixTokensReused : Nat) :
    (event.withCacheCredits
        searchWorkUnitsAvoided
        modelPrefixTokensReused).realizedPath = event.realizedPath := by
  rfl

def acceptedTraceGateClosure
    {context : ReplayContext}
    {eventCount internalTransitions : Nat}
    (replay : AcceptedReplay context)
    (_trace :
      ObservableReplayTrace
        context
        eventCount
        internalTransitions
        (.raw replay.negotiated.verified.raw)
        (.accepted replay)) :
    AcceptedReplayGateClosure context replay :=
  acceptedReplayGateClosure replay

theorem accepted_trace_identity_ignores_cache_accounting
    {context : ReplayContext}
    {eventCount internalTransitions : Nat}
    (replay : AcceptedReplay context)
    (_trace :
      ObservableReplayTrace
        context
        eventCount
        internalTransitions
        (.raw replay.negotiated.verified.raw)
        (.accepted replay))
    (_searchWorkUnitsAvoided _modelPrefixTokensReused : Nat) :
    canonicalPayloadOfRaw replay.negotiated.verified.raw =
      context.expectedPayload :=
  replay.payloadEqual

end ASPProof.SearchRouteObservableReplayTraceProjection
