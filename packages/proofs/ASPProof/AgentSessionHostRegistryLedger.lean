import ASPProof.AgentSessionHostAcceptanceDurability

namespace ASPProof.AgentSessionHostRegistryLedger

open ASPProof.AgentSessionDispatchTransaction
open ASPProof.AgentSessionHostAcceptanceDurability

abbrev LedgerRevision := Nat
abbrev SnapshotRevision := Nat

structure RegistrySnapshot (intent : DispatchIntent) where
  snapshotRevision : SnapshotRevision
  registry : DurableRegistry intent
  deriving DecidableEq, Repr

structure LedgerHead (intent : DispatchIntent) where
  revision : LedgerRevision
  snapshot : RegistrySnapshot intent
  deriving DecidableEq, Repr

inductive SafeSnapshotSuccessor (intent : DispatchIntent) :
    RegistrySnapshot intent → RegistrySnapshot intent → Prop where
  | firstAccept
      (authority : HostAuthorityId)
      (epoch : HostAuthorityEpoch)
      (generation : RegistryGeneration)
      (snapshotRevision : SnapshotRevision)
      (effectId : EffectId) :
      SafeSnapshotSuccessor intent
        {
          snapshotRevision := snapshotRevision
          registry := {
            authority := authority
            epoch := epoch
            generation := generation
            slot := .empty
          }
        }
        {
          snapshotRevision := snapshotRevision + 1
          registry := {
            authority := authority
            epoch := epoch
            generation := generation
            slot := .accepted
              (firstDurableReceipt
                intent authority epoch generation effectId)
          }
        }
  | rotateEmpty
      (oldAuthority newAuthority : HostAuthorityId)
      (oldEpoch : HostAuthorityEpoch)
      (oldGeneration : RegistryGeneration)
      (snapshotRevision : SnapshotRevision) :
      SafeSnapshotSuccessor intent
        {
          snapshotRevision := snapshotRevision
          registry := {
            authority := oldAuthority
            epoch := oldEpoch
            generation := oldGeneration
            slot := .empty
          }
        }
        {
          snapshotRevision := snapshotRevision + 1
          registry := {
            authority := newAuthority
            epoch := oldEpoch + 1
            generation := oldGeneration + 1
            slot := .empty
          }
        }
  | rotateAccepted
      (oldAuthority newAuthority : HostAuthorityId)
      (oldEpoch : HostAuthorityEpoch)
      (oldGeneration : RegistryGeneration)
      (snapshotRevision : SnapshotRevision)
      (receipt : DurableHostReceipt
        intent oldAuthority oldEpoch oldGeneration) :
      SafeSnapshotSuccessor intent
        {
          snapshotRevision := snapshotRevision
          registry := {
            authority := oldAuthority
            epoch := oldEpoch
            generation := oldGeneration
            slot := .accepted receipt
          }
        }
        {
          snapshotRevision := snapshotRevision + 1
          registry := {
            authority := newAuthority
            epoch := oldEpoch + 1
            generation := oldGeneration + 1
            slot := .accepted (migrateReceipt newAuthority receipt)
          }
        }

inductive UnanchoredLedgerStep (intent : DispatchIntent) :
    LedgerRevision → LedgerHead intent → LedgerHead intent → Prop where
  | publish
      (revision : LedgerRevision)
      (before after : RegistrySnapshot intent) :
      UnanchoredLedgerStep intent revision
        { revision := revision, snapshot := before }
        { revision := revision + 1, snapshot := after }

inductive UnanchoredRecovery {intent : DispatchIntent}
    (head : LedgerHead intent) :
    RegistrySnapshot intent → RecoveryDisposition → Prop where
  | allowAny (candidate : RegistrySnapshot intent) :
      UnanchoredRecovery head candidate .resume

inductive ConflictingSnapshots (intent : DispatchIntent) :
    RegistrySnapshot intent → RegistrySnapshot intent → Prop where
  | emptyAccepted
      (snapshotRevision : SnapshotRevision)
      (authority : HostAuthorityId)
      (epoch : HostAuthorityEpoch)
      (generation : RegistryGeneration)
      (receipt : DurableHostReceipt intent authority epoch generation) :
      ConflictingSnapshots intent
        {
          snapshotRevision := snapshotRevision
          registry := {
            authority := authority
            epoch := epoch
            generation := generation
            slot := .empty
          }
        }
        {
          snapshotRevision := snapshotRevision
          registry := {
            authority := authority
            epoch := epoch
            generation := generation
            slot := .accepted receipt
          }
        }

inductive OlderSnapshot (intent : DispatchIntent) :
    RegistrySnapshot intent → RegistrySnapshot intent → Prop where
  | oneStep
      (snapshotRevision : SnapshotRevision)
      (olderRegistry currentRegistry : DurableRegistry intent) :
      OlderSnapshot intent
        {
          snapshotRevision := snapshotRevision
          registry := olderRegistry
        }
        {
          snapshotRevision := snapshotRevision + 1
          registry := currentRegistry
        }

abbrev CanonicalSnapshotChoice (intent : DispatchIntent) :=
  LedgerRevision → RegistrySnapshot intent

inductive CanonicalLedgerCas
    (intent : DispatchIntent)
    (choice : CanonicalSnapshotChoice intent) :
    LedgerRevision → LedgerHead intent → LedgerHead intent → Prop where
  | commit
      (revision : LedgerRevision)
      (before : RegistrySnapshot intent)
      (safe : SafeSnapshotSuccessor intent before (choice (revision + 1))) :
      CanonicalLedgerCas intent choice revision
        { revision := revision, snapshot := before }
        { revision := revision + 1, snapshot := choice (revision + 1) }

inductive AnchoredRecovery {intent : DispatchIntent}
    (head : LedgerHead intent) :
    RegistrySnapshot intent → RecoveryDisposition → Prop where
  | exact : AnchoredRecovery head head.snapshot .resume
  | mismatch
      (candidate : RegistrySnapshot intent)
      (different : candidate ≠ head.snapshot) :
      AnchoredRecovery head candidate .quarantine

structure LedgerBoundReceipt
    (intent : DispatchIntent)
    (ledgerRevision : LedgerRevision)
    (snapshot : RegistrySnapshot intent) where
  durableReceipt : DurableHostReceipt intent
    snapshot.registry.authority
    snapshot.registry.epoch
    snapshot.registry.generation
  deriving DecidableEq, Repr

inductive HeadContainsBoundReceipt {intent : DispatchIntent} :
    (head : LedgerHead intent) →
    LedgerBoundReceipt intent head.revision head.snapshot → Prop where
  | accepted
      (ledgerRevision : LedgerRevision)
      (snapshotRevision : SnapshotRevision)
      (authority : HostAuthorityId)
      (epoch : HostAuthorityEpoch)
      (generation : RegistryGeneration)
      (receipt : DurableHostReceipt intent authority epoch generation) :
      HeadContainsBoundReceipt
        {
          revision := ledgerRevision
          snapshot := {
            snapshotRevision := snapshotRevision
            registry := {
              authority := authority
              epoch := epoch
              generation := generation
              slot := .accepted receipt
            }
          }
        }
        { durableReceipt := receipt }

inductive LedgerVerifiedDeliveredCasStep (intent : DispatchIntent) :
    (head : LedgerHead intent) →
    Revision →
    DispatchTransaction intent →
    LedgerBoundReceipt intent head.revision head.snapshot →
    DispatchTransaction intent → Prop where
  | commit
      (ledgerRevision : LedgerRevision)
      (snapshotRevision : SnapshotRevision)
      (authority : HostAuthorityId)
      (epoch : HostAuthorityEpoch)
      (generation : RegistryGeneration)
      (transactionRevision : Revision)
      (receipt : DurableHostReceipt intent authority epoch generation) :
      LedgerVerifiedDeliveredCasStep intent
        {
          revision := ledgerRevision
          snapshot := {
            snapshotRevision := snapshotRevision
            registry := {
              authority := authority
              epoch := epoch
              generation := generation
              slot := .accepted receipt
            }
          }
        }
        transactionRevision
        ({
          revision := transactionRevision
          phase := TransactionPhase.attempting
        } : DispatchTransaction intent)
        { durableReceipt := receipt }
        ({
          revision := transactionRevision + 1
          phase := TransactionPhase.delivered receipt.hostReceipt
        } : DispatchTransaction intent)

theorem accepted_snapshot_ne_empty_snapshot
    (intent : DispatchIntent)
    (authority : HostAuthorityId)
    (epoch : HostAuthorityEpoch)
    (generation : RegistryGeneration)
    (snapshotRevision : SnapshotRevision)
    (receipt : DurableHostReceipt intent authority epoch generation) :
    ({
      snapshotRevision := snapshotRevision
      registry := {
        authority := authority
        epoch := epoch
        generation := generation
        slot := .accepted receipt
      }
    } : RegistrySnapshot intent) ≠
    ({
      snapshotRevision := snapshotRevision
      registry := {
        authority := authority
        epoch := epoch
        generation := generation
        slot := .empty
      }
    } : RegistrySnapshot intent) := by
  intro equal
  cases equal

theorem unanchored_split_brain_constructible
    (intent : DispatchIntent)
    (authority : HostAuthorityId)
    (epoch : HostAuthorityEpoch)
    (generation : RegistryGeneration)
    (effectId : EffectId) :
    ∃ before left right,
      UnanchoredLedgerStep intent 0 before left ∧
      UnanchoredLedgerStep intent 0 before right ∧
      ConflictingSnapshots intent left.snapshot right.snapshot := by
  let emptySnapshot : RegistrySnapshot intent := {
    snapshotRevision := 0
    registry := {
      authority := authority
      epoch := epoch
      generation := generation
      slot := .empty
    }
  }
  let acceptedReceipt :=
    firstDurableReceipt intent authority epoch generation effectId
  let acceptedSnapshot : RegistrySnapshot intent := {
    snapshotRevision := 0
    registry := {
      authority := authority
      epoch := epoch
      generation := generation
      slot := .accepted acceptedReceipt
    }
  }
  exact ⟨
    { revision := 0, snapshot := emptySnapshot },
    { revision := 1, snapshot := emptySnapshot },
    { revision := 1, snapshot := acceptedSnapshot },
    UnanchoredLedgerStep.publish 0 emptySnapshot emptySnapshot,
    UnanchoredLedgerStep.publish 0 emptySnapshot acceptedSnapshot,
    ConflictingSnapshots.emptyAccepted
      0 authority epoch generation acceptedReceipt
  ⟩

theorem unanchored_rollback_resume_constructible
    (intent : DispatchIntent)
    (authority : HostAuthorityId)
    (epoch : HostAuthorityEpoch)
    (generation : RegistryGeneration)
    (effectId : EffectId) :
    ∃ (head : LedgerHead intent) (stale : RegistrySnapshot intent),
      UnanchoredRecovery head stale .resume ∧
      OlderSnapshot intent stale head.snapshot := by
  let receipt :=
    firstDurableReceipt intent authority epoch generation effectId
  let stale : RegistrySnapshot intent := {
    snapshotRevision := 0
    registry := {
      authority := authority
      epoch := epoch
      generation := generation
      slot := .empty
    }
  }
  let current : RegistrySnapshot intent := {
    snapshotRevision := 1
    registry := {
      authority := authority
      epoch := epoch
      generation := generation
      slot := .accepted receipt
    }
  }
  exact ⟨
    { revision := 1, snapshot := current },
    stale,
    UnanchoredRecovery.allowAny stale,
    OlderSnapshot.oneStep 0 stale.registry current.registry
  ⟩

theorem accepted_snapshot_cannot_advance_to_empty
    (intent : DispatchIntent)
    (beforeRevision afterRevision : SnapshotRevision)
    (beforeAuthority afterAuthority : HostAuthorityId)
    (beforeEpoch afterEpoch : HostAuthorityEpoch)
    (beforeGeneration afterGeneration : RegistryGeneration)
    (receipt : DurableHostReceipt
      intent beforeAuthority beforeEpoch beforeGeneration) :
    ¬ SafeSnapshotSuccessor intent
      {
        snapshotRevision := beforeRevision
        registry := {
          authority := beforeAuthority
          epoch := beforeEpoch
          generation := beforeGeneration
          slot := .accepted receipt
        }
      }
      {
        snapshotRevision := afterRevision
        registry := {
          authority := afterAuthority
          epoch := afterEpoch
          generation := afterGeneration
          slot := .empty
        }
      } := by
  intro successor
  cases successor

theorem canonical_first_accept_constructible
    (intent : DispatchIntent)
    (authority : HostAuthorityId)
    (epoch : HostAuthorityEpoch)
    (generation : RegistryGeneration)
    (effectId : EffectId) :
    let before : RegistrySnapshot intent := {
      snapshotRevision := 0
      registry := {
        authority := authority
        epoch := epoch
        generation := generation
        slot := .empty
      }
    }
    let after : RegistrySnapshot intent := {
      snapshotRevision := 1
      registry := {
        authority := authority
        epoch := epoch
        generation := generation
        slot := .accepted
          (firstDurableReceipt intent authority epoch generation effectId)
      }
    }
    let choice : CanonicalSnapshotChoice intent := fun _ => after
    CanonicalLedgerCas intent choice 0
      { revision := 0, snapshot := before }
      { revision := 1, snapshot := after } := by
  exact CanonicalLedgerCas.commit 0 _
    (SafeSnapshotSuccessor.firstAccept
      authority epoch generation 0 effectId)

theorem stale_ledger_revision_cannot_commit
    (intent : DispatchIntent)
    (choice : CanonicalSnapshotChoice intent)
    (expected current : LedgerRevision)
    (snapshot : RegistrySnapshot intent)
    (stale : expected ≠ current) :
    ¬ ∃ after,
      CanonicalLedgerCas intent choice expected
        { revision := current, snapshot := snapshot }
        after := by
  intro witness
  rcases witness with ⟨after, step⟩
  cases step
  exact stale rfl

theorem canonical_same_head_commits_are_unique
    (intent : DispatchIntent)
    (choice : CanonicalSnapshotChoice intent)
    (expected : LedgerRevision)
    (before : LedgerHead intent)
    (left right : LedgerHead intent)
    (leftCommit : CanonicalLedgerCas intent choice expected before left)
    (rightCommit : CanonicalLedgerCas intent choice expected before right) :
    left = right := by
  cases leftCommit
  cases rightCommit
  rfl

theorem canonical_commit_advances_ledger_revision
    (intent : DispatchIntent)
    (choice : CanonicalSnapshotChoice intent)
    (revision : LedgerRevision)
    (before after : LedgerHead intent)
    (commit : CanonicalLedgerCas intent choice revision before after) :
    after.revision = revision + 1 := by
  cases commit
  rfl

theorem exact_head_snapshot_resumes
    {intent : DispatchIntent}
    (head : LedgerHead intent) :
    AnchoredRecovery head head.snapshot .resume := by
  exact AnchoredRecovery.exact

theorem stale_snapshot_quarantines
    {intent : DispatchIntent}
    (head : LedgerHead intent)
    (candidate : RegistrySnapshot intent)
    (stale : candidate ≠ head.snapshot) :
    AnchoredRecovery head candidate .quarantine := by
  exact AnchoredRecovery.mismatch candidate stale

theorem stale_snapshot_cannot_resume
    {intent : DispatchIntent}
    (head : LedgerHead intent)
    (candidate : RegistrySnapshot intent)
    (stale : candidate ≠ head.snapshot) :
    ¬ AnchoredRecovery head candidate .resume := by
  intro recovery
  cases recovery
  exact stale rfl

theorem accepted_head_contains_bound_receipt
    (intent : DispatchIntent)
    (ledgerRevision : LedgerRevision)
    (snapshotRevision : SnapshotRevision)
    (authority : HostAuthorityId)
    (epoch : HostAuthorityEpoch)
    (generation : RegistryGeneration)
    (receipt : DurableHostReceipt intent authority epoch generation) :
    HeadContainsBoundReceipt
      {
        revision := ledgerRevision
        snapshot := {
          snapshotRevision := snapshotRevision
          registry := {
            authority := authority
            epoch := epoch
            generation := generation
            slot := .accepted receipt
          }
        }
      }
      { durableReceipt := receipt } := by
  exact HeadContainsBoundReceipt.accepted
    ledgerRevision snapshotRevision authority epoch generation receipt

theorem empty_head_cannot_contain_bound_receipt
    (intent : DispatchIntent)
    (ledgerRevision : LedgerRevision)
    (snapshotRevision : SnapshotRevision)
    (authority : HostAuthorityId)
    (epoch : HostAuthorityEpoch)
    (generation : RegistryGeneration)
    (receipt : DurableHostReceipt intent authority epoch generation) :
    ¬ HeadContainsBoundReceipt
      {
        revision := ledgerRevision
        snapshot := {
          snapshotRevision := snapshotRevision
          registry := {
            authority := authority
            epoch := epoch
            generation := generation
            slot := .empty
          }
        }
      }
      { durableReceipt := receipt } := by
  intro contained
  cases contained

theorem ledger_bound_receipt_can_finalize
    (intent : DispatchIntent)
    (ledgerRevision : LedgerRevision)
    (snapshotRevision : SnapshotRevision)
    (authority : HostAuthorityId)
    (epoch : HostAuthorityEpoch)
    (generation : RegistryGeneration)
    (transactionRevision : Revision)
    (receipt : DurableHostReceipt intent authority epoch generation) :
    LedgerVerifiedDeliveredCasStep intent
      {
        revision := ledgerRevision
        snapshot := {
          snapshotRevision := snapshotRevision
          registry := {
            authority := authority
            epoch := epoch
            generation := generation
            slot := .accepted receipt
          }
        }
      }
      transactionRevision
      ({
        revision := transactionRevision
        phase := TransactionPhase.attempting
      } : DispatchTransaction intent)
      { durableReceipt := receipt }
      ({
        revision := transactionRevision + 1
        phase := TransactionPhase.delivered receipt.hostReceipt
      } : DispatchTransaction intent) := by
  exact LedgerVerifiedDeliveredCasStep.commit
    ledgerRevision snapshotRevision authority epoch generation
    transactionRevision receipt

theorem empty_head_cannot_finalize
    (intent : DispatchIntent)
    (ledgerRevision : LedgerRevision)
    (snapshotRevision : SnapshotRevision)
    (authority : HostAuthorityId)
    (epoch : HostAuthorityEpoch)
    (generation : RegistryGeneration)
    (transactionRevision : Revision)
    (receipt : DurableHostReceipt intent authority epoch generation) :
    ¬ ∃ after,
      LedgerVerifiedDeliveredCasStep intent
        {
          revision := ledgerRevision
          snapshot := {
            snapshotRevision := snapshotRevision
            registry := {
              authority := authority
              epoch := epoch
              generation := generation
              slot := .empty
            }
          }
        }
        transactionRevision
        ({
          revision := transactionRevision
          phase := TransactionPhase.attempting
        } : DispatchTransaction intent)
        { durableReceipt := receipt }
        after := by
  intro witness
  rcases witness with ⟨after, step⟩
  cases step

end ASPProof.AgentSessionHostRegistryLedger
