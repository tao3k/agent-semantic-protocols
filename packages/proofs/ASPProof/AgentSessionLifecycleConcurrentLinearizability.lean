import ASPProof.AgentSessionLifecycleCrashRecovery

namespace ASPProof.AgentSessionLifecycleConcurrentLinearizability

open AgentSessionTransactionalGenerationLifecycle

structure IntentClaimCommit where
  slot : ResidentSlot
  generation : Nat
  intentKey : Nat
  payloadDigest : Nat
  controllerId : Nat
  commitRevision : Nat
  deriving DecidableEq

def ClaimLogLinearizable (commits : List IntentClaimCommit) : Prop :=
  ∀ left, left ∈ commits → ∀ right, right ∈ commits →
    left.slot = right.slot →
    left.generation = right.generation →
    left.intentKey = right.intentKey →
    left.commitRevision = right.commitRevision →
    left = right

structure IndexedHostReceipt where
  slot : ResidentSlot
  generation : Nat
  intentKey : Nat
  payloadDigest : Nat
  effectIdentity : Nat
  indexRevision : Nat
  deriving DecidableEq

def ReceiptIndexLinearizable (receipts : List IndexedHostReceipt) : Prop :=
  ∀ left, left ∈ receipts → ∀ right, right ∈ receipts →
    left.slot = right.slot →
    left.generation = right.generation →
    left.intentKey = right.intentKey →
    left = right

structure BindingCommit where
  slot : ResidentSlot
  generation : Nat
  childId : Nat
  canonicalTarget : Nat
  bindingRevision : Nat
  deriving DecidableEq

def BindingLedgerLinearizable (bindings : List BindingCommit) : Prop :=
  ∀ left, left ∈ bindings → ∀ right, right ∈ bindings →
    left.slot = right.slot →
    left.generation = right.generation →
    left = right

structure TerminalDispatchCommit where
  slot : ResidentSlot
  dispatchKey : Nat
  commandDigest : Nat
  effectDigest : Nat
  terminalRevision : Nat
  deriving DecidableEq

def TerminalLedgerLinearizable (terminals : List TerminalDispatchCommit) : Prop :=
  ∀ left, left ∈ terminals → ∀ right, right ∈ terminals →
    left.slot = right.slot →
    left.dispatchKey = right.dispatchKey →
    left = right

structure LifecycleLinearizationOrder where
  claimRevision : Nat
  receiptRevision : Nat
  bindingRevision : Nat
  claimBeforeReceipt : claimRevision < receiptRevision
  receiptBeforeBinding : receiptRevision < bindingRevision

structure ClaimSnapshot where
  occupied : Bool
  intentKey : Nat
  payloadDigest : Nat
  deriving DecidableEq

inductive ClaimAction where
  | attemptReservationCas
  | reuseCommittedIntent
  | rejectPayloadConflict
  deriving DecidableEq

def classifyClaim
    (snapshot : ClaimSnapshot)
    (requestedKey requestedPayload : Nat) : ClaimAction :=
  if !snapshot.occupied then .attemptReservationCas
  else if snapshot.intentKey = requestedKey &&
      snapshot.payloadDigest = requestedPayload then
    .reuseCommittedIntent
  else
    .rejectPayloadConflict

def NonAtomicClaimAdmitsTwo
    (left right : IntentClaimCommit) : Prop :=
  left.slot = right.slot ∧
  left.generation = right.generation ∧
  left.intentKey = right.intentKey ∧
  left.commitRevision = right.commitRevision ∧
  left.controllerId ≠ right.controllerId

theorem linearizable_claim_has_one_controller
    {commits : List IntentClaimCommit}
    (linearizable : ClaimLogLinearizable commits)
    {left right : IntentClaimCommit}
    (leftMember : left ∈ commits) (rightMember : right ∈ commits)
    (sameSlot : left.slot = right.slot)
    (sameGeneration : left.generation = right.generation)
    (sameIntent : left.intentKey = right.intentKey)
    (sameRevision : left.commitRevision = right.commitRevision) :
    left.controllerId = right.controllerId := by
  have same := linearizable left leftMember right rightMember
    sameSlot sameGeneration sameIntent sameRevision
  exact congrArg IntentClaimCommit.controllerId same

theorem linearizable_claim_has_one_payload
    {commits : List IntentClaimCommit}
    (linearizable : ClaimLogLinearizable commits)
    {left right : IntentClaimCommit}
    (leftMember : left ∈ commits) (rightMember : right ∈ commits)
    (sameSlot : left.slot = right.slot)
    (sameGeneration : left.generation = right.generation)
    (sameIntent : left.intentKey = right.intentKey)
    (sameRevision : left.commitRevision = right.commitRevision) :
    left.payloadDigest = right.payloadDigest := by
  have same := linearizable left leftMember right rightMember
    sameSlot sameGeneration sameIntent sameRevision
  exact congrArg IntentClaimCommit.payloadDigest same

theorem non_atomic_claim_can_express_two_winners :
    ∃ left right : IntentClaimCommit, NonAtomicClaimAdmitsTwo left right := by
  let slot : ResidentSlot := ⟨1, 2, 3⟩
  exact ⟨⟨slot, 4, 5, 6, 7, 8⟩, ⟨slot, 4, 5, 6, 9, 8⟩,
    rfl, rfl, rfl, rfl, by decide⟩

theorem non_atomic_two_winners_violate_linearizable_claim
    {commits : List IntentClaimCommit}
    (linearizable : ClaimLogLinearizable commits)
    {left right : IntentClaimCommit}
    (leftMember : left ∈ commits) (rightMember : right ∈ commits)
    (two : NonAtomicClaimAdmitsTwo left right) : False := by
  have sameController := linearizable_claim_has_one_controller linearizable
    leftMember rightMember two.1 two.2.1 two.2.2.1 two.2.2.2.1
  exact two.2.2.2.2 sameController

theorem receipt_index_has_one_effect_identity
    {receipts : List IndexedHostReceipt}
    (linearizable : ReceiptIndexLinearizable receipts)
    {left right : IndexedHostReceipt}
    (leftMember : left ∈ receipts) (rightMember : right ∈ receipts)
    (sameSlot : left.slot = right.slot)
    (sameGeneration : left.generation = right.generation)
    (sameIntent : left.intentKey = right.intentKey) :
    left.effectIdentity = right.effectIdentity := by
  have same := linearizable left leftMember right rightMember
    sameSlot sameGeneration sameIntent
  exact congrArg IndexedHostReceipt.effectIdentity same

theorem receipt_index_rejects_payload_conflict
    {receipts : List IndexedHostReceipt}
    (linearizable : ReceiptIndexLinearizable receipts)
    {left right : IndexedHostReceipt}
    (leftMember : left ∈ receipts) (rightMember : right ∈ receipts)
    (sameSlot : left.slot = right.slot)
    (sameGeneration : left.generation = right.generation)
    (sameIntent : left.intentKey = right.intentKey)
    (conflict : left.payloadDigest ≠ right.payloadDigest) : False := by
  have same := linearizable left leftMember right rightMember
    sameSlot sameGeneration sameIntent
  exact conflict (congrArg IndexedHostReceipt.payloadDigest same)

theorem binding_ledger_has_one_child_per_generation
    {bindings : List BindingCommit}
    (linearizable : BindingLedgerLinearizable bindings)
    {left right : BindingCommit}
    (leftMember : left ∈ bindings) (rightMember : right ∈ bindings)
    (sameSlot : left.slot = right.slot)
    (sameGeneration : left.generation = right.generation) :
    left.childId = right.childId := by
  have same := linearizable left leftMember right rightMember
    sameSlot sameGeneration
  exact congrArg BindingCommit.childId same

theorem binding_ledger_has_one_target_per_generation
    {bindings : List BindingCommit}
    (linearizable : BindingLedgerLinearizable bindings)
    {left right : BindingCommit}
    (leftMember : left ∈ bindings) (rightMember : right ∈ bindings)
    (sameSlot : left.slot = right.slot)
    (sameGeneration : left.generation = right.generation) :
    left.canonicalTarget = right.canonicalTarget := by
  have same := linearizable left leftMember right rightMember
    sameSlot sameGeneration
  exact congrArg BindingCommit.canonicalTarget same

theorem terminal_ledger_has_one_effect
    {terminals : List TerminalDispatchCommit}
    (linearizable : TerminalLedgerLinearizable terminals)
    {left right : TerminalDispatchCommit}
    (leftMember : left ∈ terminals) (rightMember : right ∈ terminals)
    (sameSlot : left.slot = right.slot)
    (sameDispatch : left.dispatchKey = right.dispatchKey) :
    left.effectDigest = right.effectDigest := by
  have same := linearizable left leftMember right rightMember
    sameSlot sameDispatch
  exact congrArg TerminalDispatchCommit.effectDigest same

theorem terminal_ledger_rejects_command_conflict
    {terminals : List TerminalDispatchCommit}
    (linearizable : TerminalLedgerLinearizable terminals)
    {left right : TerminalDispatchCommit}
    (leftMember : left ∈ terminals) (rightMember : right ∈ terminals)
    (sameSlot : left.slot = right.slot)
    (sameDispatch : left.dispatchKey = right.dispatchKey)
    (conflict : left.commandDigest ≠ right.commandDigest) : False := by
  have same := linearizable left leftMember right rightMember
    sameSlot sameDispatch
  exact conflict (congrArg TerminalDispatchCommit.commandDigest same)

theorem empty_claim_snapshot_attempts_reservation_cas :
    classifyClaim ⟨false, 0, 0⟩ 10 20 = .attemptReservationCas :=
  rfl

theorem matching_claim_snapshot_reuses_committed_intent :
    classifyClaim ⟨true, 10, 20⟩ 10 20 = .reuseCommittedIntent :=
  rfl

theorem matching_key_with_payload_drift_is_rejected :
    classifyClaim ⟨true, 10, 21⟩ 10 20 = .rejectPayloadConflict :=
  rfl

theorem different_key_is_rejected_while_slot_occupied :
    classifyClaim ⟨true, 11, 20⟩ 10 20 = .rejectPayloadConflict :=
  rfl

theorem restarted_controller_reuses_durable_intent :
    classifyClaim ⟨true, 10, 20⟩ 10 20 = .reuseCommittedIntent :=
  rfl

theorem claim_before_receipt
    (order : LifecycleLinearizationOrder) :
    order.claimRevision < order.receiptRevision :=
  order.claimBeforeReceipt

theorem receipt_before_binding
    (order : LifecycleLinearizationOrder) :
    order.receiptRevision < order.bindingRevision :=
  order.receiptBeforeBinding

theorem claim_before_binding
    (order : LifecycleLinearizationOrder) :
    order.claimRevision < order.bindingRevision :=
  Nat.lt_trans order.claimBeforeReceipt order.receiptBeforeBinding

theorem binding_cannot_precede_claim
    (order : LifecycleLinearizationOrder) :
    ¬ order.bindingRevision < order.claimRevision :=
  Nat.not_lt_of_ge (Nat.le_of_lt (claim_before_binding order))

end ASPProof.AgentSessionLifecycleConcurrentLinearizability
