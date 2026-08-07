import ASPProof.CodexMultiAgentV2FocusedDelegation

namespace ASPProof.CodexMultiAgentV2DurableDelegation

open ASPProof.CodexMultiAgentV2FocusedDelegation

structure DurableDelegationEvent where
  eventId : String
  expectedGeneration : Nat
  proposal : DelegationProposal
  receipt : DelegationAdmissionReceipt
deriving DecidableEq

structure DurableDelegationState where
  generation : Nat
  projection : DelegationState
  journal : List DurableDelegationEvent
deriving DecidableEq

inductive DurableDelegationOutcome where
  | committed (receipt : DelegationAdmissionReceipt)
  | replayed (receipt : DelegationAdmissionReceipt)
  | staleGeneration
deriving DecidableEq

structure DurableDelegationTransaction where
  state : DurableDelegationState
  outcome : DurableDelegationOutcome
deriving DecidableEq

def committedEvent
    (state : DurableDelegationState)
    (eventId : String) : Option DurableDelegationEvent :=
  state.journal.find? (fun event => event.eventId == eventId)

def admittedGeneration
    (generation : Nat)
    (receipt : DelegationAdmissionReceipt) : Nat :=
  match receipt.decision with
  | .accepted => generation + 1
  | .denied _ => generation

def transactDelegation
    (state : DurableDelegationState)
    (eventId : String)
    (expectedGeneration : Nat)
    (proposal : DelegationProposal) : DurableDelegationTransaction :=
  match committedEvent state eventId with
  | some event =>
      { state := state, outcome := .replayed event.receipt }
  | none =>
      if expectedGeneration = state.generation then
        let receipt := admitDelegation state.projection proposal
        let nextState :=
          { generation := admittedGeneration state.generation receipt
            projection := receipt.nextState
            journal := state.journal ++
              [{ eventId
                 expectedGeneration
                 proposal
                 receipt }] }
        { state := nextState, outcome := .committed receipt }
      else
        { state := state, outcome := .staleGeneration }

theorem duplicateEventReplaysWithoutMutation
    (state : DurableDelegationState)
    (eventId : String)
    (expectedGeneration : Nat)
    (proposal : DelegationProposal)
    (event : DurableDelegationEvent)
    (committed : committedEvent state eventId = some event) :
    transactDelegation state eventId expectedGeneration proposal =
      { state := state, outcome := .replayed event.receipt } := by
  simp [transactDelegation, committed]

theorem staleGenerationPreservesDurableState
    (state : DurableDelegationState)
    (eventId : String)
    (expectedGeneration : Nat)
    (proposal : DelegationProposal)
    (fresh : committedEvent state eventId = none)
    (stale : expectedGeneration ≠ state.generation) :
    transactDelegation state eventId expectedGeneration proposal =
      { state := state, outcome := .staleGeneration } := by
  simp [transactDelegation, fresh, stale]

theorem focusedLeafCommitPreservesProjectionAndGeneration
    (state : DurableDelegationState)
    (eventId : String)
    (proposal : DelegationProposal)
    (current : ControlAgent)
    (fresh : committedEvent state eventId = none)
    (found : currentAgent state.projection proposal.parentSessionId = some current)
    (leaf : current.capability = .focusedLeaf) :
    let transaction :=
      transactDelegation state eventId state.generation proposal
    transaction.state.generation = state.generation ∧
      transaction.state.projection = state.projection ∧
      transaction.state.journal.length = state.journal.length + 1 := by
  simp [transactDelegation, fresh, admittedGeneration, admitDelegation, found, leaf]

end ASPProof.CodexMultiAgentV2DurableDelegation
