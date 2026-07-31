import ASPProof.SearchRouteAdmissionRetryCacheRejoinRecovery

namespace ASPProof.SearchRouteAdmissionRetryCacheRejoinReceipts

open ASPProof.SearchRouteAdmissionRetryCacheRejoinRecovery

structure RejoinReceiptSet where
  stateInstalledGeneration : Option Nat
  tokenIssuedGeneration : Option Nat
  membershipPublishedGeneration : Option Nat
  readsEnabledGeneration : Option Nat
deriving DecidableEq, Repr

def ReceiptsSupportPhase
    (receipts : RejoinReceiptSet)
    (phase : RejoinPhase) : Prop :=
  match phase with
  | .notStarted => True
  | .stateInstalled =>
      ∃ stateGeneration,
        receipts.stateInstalledGeneration = some stateGeneration
  | .tokenIssued =>
      ∃ stateGeneration tokenGeneration,
        receipts.stateInstalledGeneration = some stateGeneration
          ∧ receipts.tokenIssuedGeneration = some tokenGeneration
          ∧ stateGeneration < tokenGeneration
  | .membershipPublished =>
      ∃ stateGeneration tokenGeneration membershipGeneration,
        receipts.stateInstalledGeneration = some stateGeneration
          ∧ receipts.tokenIssuedGeneration = some tokenGeneration
          ∧ receipts.membershipPublishedGeneration =
            some membershipGeneration
          ∧ stateGeneration < tokenGeneration
          ∧ tokenGeneration < membershipGeneration
  | .readsEnabled =>
      ∃ stateGeneration tokenGeneration membershipGeneration readGeneration,
        receipts.stateInstalledGeneration = some stateGeneration
          ∧ receipts.tokenIssuedGeneration = some tokenGeneration
          ∧ receipts.membershipPublishedGeneration =
            some membershipGeneration
          ∧ receipts.readsEnabledGeneration = some readGeneration
          ∧ stateGeneration < tokenGeneration
          ∧ tokenGeneration < membershipGeneration
          ∧ membershipGeneration < readGeneration

structure DurableRejoinRecord where
  phase : RejoinPhase
  receipts : RejoinReceiptSet
deriving DecidableEq, Repr

def DurableRejoinRecordWellFormed
    (record : DurableRejoinRecord) : Prop :=
  ReceiptsSupportPhase record.receipts record.phase

theorem reads_enabled_record_has_complete_ordered_receipt_chain
    {record : DurableRejoinRecord}
    (readsPhase : record.phase = RejoinPhase.readsEnabled)
    (wellFormed : DurableRejoinRecordWellFormed record) :
    ∃ stateGeneration tokenGeneration membershipGeneration readGeneration,
      record.receipts.stateInstalledGeneration = some stateGeneration
        ∧ record.receipts.tokenIssuedGeneration = some tokenGeneration
        ∧ record.receipts.membershipPublishedGeneration =
          some membershipGeneration
        ∧ record.receipts.readsEnabledGeneration = some readGeneration
        ∧ stateGeneration < tokenGeneration
        ∧ tokenGeneration < membershipGeneration
        ∧ membershipGeneration < readGeneration := by
  unfold DurableRejoinRecordWellFormed at wellFormed
  rw [readsPhase] at wellFormed
  exact wellFormed

def completeReceiptSet : RejoinReceiptSet :=
  { stateInstalledGeneration := some 0
    tokenIssuedGeneration := some 1
    membershipPublishedGeneration := some 2
    readsEnabledGeneration := some 3 }

def completeDurableRecord : DurableRejoinRecord :=
  { phase := .readsEnabled
    receipts := completeReceiptSet }

theorem complete_strict_receipt_chain_supports_reads_enabled_phase :
    DurableRejoinRecordWellFormed completeDurableRecord := by
  simp
    [DurableRejoinRecordWellFormed,
      ReceiptsSupportPhase,
      completeDurableRecord,
      completeReceiptSet]

def tornTokenReceiptSet : RejoinReceiptSet :=
  { stateInstalledGeneration := some 0
    tokenIssuedGeneration := none
    membershipPublishedGeneration := none
    readsEnabledGeneration := none }

def tornTokenRecord : DurableRejoinRecord :=
  { phase := .tokenIssued
    receipts := tornTokenReceiptSet }

theorem torn_token_receipt_cannot_support_token_issued_phase :
    tornTokenRecord.receipts.stateInstalledGeneration = some 0
      ∧ tornTokenRecord.receipts.tokenIssuedGeneration = none
      ∧ ¬ DurableRejoinRecordWellFormed tornTokenRecord := by
  simp
    [DurableRejoinRecordWellFormed,
      ReceiptsSupportPhase,
      tornTokenRecord,
      tornTokenReceiptSet]

def missingMembershipReceiptSet : RejoinReceiptSet :=
  { stateInstalledGeneration := some 0
    tokenIssuedGeneration := some 1
    membershipPublishedGeneration := none
    readsEnabledGeneration := some 3 }

def missingMembershipRecord : DurableRejoinRecord :=
  { phase := .readsEnabled
    receipts := missingMembershipReceiptSet }

theorem later_read_receipt_cannot_replace_missing_predecessor :
    missingMembershipRecord.receipts.readsEnabledGeneration = some 3
      ∧ missingMembershipRecord.receipts.membershipPublishedGeneration = none
      ∧ ¬ DurableRejoinRecordWellFormed missingMembershipRecord := by
  simp
    [DurableRejoinRecordWellFormed,
      ReceiptsSupportPhase,
      missingMembershipRecord,
      missingMembershipReceiptSet]

def reorderedReceiptSet : RejoinReceiptSet :=
  { stateInstalledGeneration := some 0
    tokenIssuedGeneration := some 2
    membershipPublishedGeneration := some 1
    readsEnabledGeneration := none }

def reorderedMembershipRecord : DurableRejoinRecord :=
  { phase := .membershipPublished
    receipts := reorderedReceiptSet }

theorem present_but_reordered_receipts_cannot_support_membership_phase :
    reorderedMembershipRecord.receipts.tokenIssuedGeneration = some 2
      ∧ reorderedMembershipRecord.receipts.membershipPublishedGeneration =
        some 1
      ∧ ¬ DurableRejoinRecordWellFormed reorderedMembershipRecord := by
  simp
    [DurableRejoinRecordWellFormed,
      ReceiptsSupportPhase,
      reorderedMembershipRecord,
      reorderedReceiptSet]

def duplicateGenerationReceiptSet : RejoinReceiptSet :=
  { stateInstalledGeneration := some 0
    tokenIssuedGeneration := some 1
    membershipPublishedGeneration := some 1
    readsEnabledGeneration := none }

def duplicateGenerationRecord : DurableRejoinRecord :=
  { phase := .membershipPublished
    receipts := duplicateGenerationReceiptSet }

theorem duplicate_receipt_generation_cannot_establish_strict_transition :
    duplicateGenerationRecord.receipts.tokenIssuedGeneration = some 1
      ∧ duplicateGenerationRecord.receipts.membershipPublishedGeneration =
        some 1
      ∧ ¬ DurableRejoinRecordWellFormed duplicateGenerationRecord := by
  simp
    [DurableRejoinRecordWellFormed,
      ReceiptsSupportPhase,
      duplicateGenerationRecord,
      duplicateGenerationReceiptSet]

end ASPProof.SearchRouteAdmissionRetryCacheRejoinReceipts
