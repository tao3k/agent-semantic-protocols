-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteAdmissionRetryCacheRejoinReceipts

namespace ASPProof.SearchRouteAdmissionRetryCacheRejoinReceiptBinding

abbrev AttemptId := Nat
abbrev ReplicaId := Nat
abbrev ResultDigest := Nat

inductive ReceiptStep where
  | stateInstalled
  | tokenIssued
  | membershipPublished
  | readsEnabled
  deriving DecidableEq, Repr

structure ReceiptContent (Digest : Type) where
  attemptId : AttemptId
  replicaId : ReplicaId
  step : ReceiptStep
  generation : Nat
  predecessorDigest : Option Digest
  resultDigest : ResultDigest

structure DigestScheme where
  Digest : Type
  commit : ReceiptContent Digest → Digest
  collisionFree : Function.Injective commit

structure DurableReceipt (scheme : DigestScheme) where
  content : ReceiptContent scheme.Digest
  digest : scheme.Digest
  bound : digest = scheme.commit content

def RootedStateReceipt {scheme : DigestScheme}
    (receipt : DurableReceipt scheme) : Prop :=
  receipt.content.step = .stateInstalled ∧
  receipt.content.predecessorDigest = none

def LinkedReceipts {scheme : DigestScheme}
    (predecessor successor : DurableReceipt scheme)
    (predecessorStep successorStep : ReceiptStep) : Prop :=
  predecessor.content.step = predecessorStep ∧
  successor.content.step = successorStep ∧
  predecessor.content.attemptId = successor.content.attemptId ∧
  predecessor.content.replicaId = successor.content.replicaId ∧
  predecessor.content.generation < successor.content.generation ∧
  successor.content.predecessorDigest = some predecessor.digest

structure ReadsEnabledReceiptChain (scheme : DigestScheme) where
  stateInstalled : DurableReceipt scheme
  tokenIssued : DurableReceipt scheme
  membershipPublished : DurableReceipt scheme
  readsEnabled : DurableReceipt scheme

def SupportsReadsEnabled {scheme : DigestScheme}
    (chain : ReadsEnabledReceiptChain scheme) : Prop :=
  RootedStateReceipt chain.stateInstalled ∧
  LinkedReceipts
    chain.stateInstalled
    chain.tokenIssued
    .stateInstalled
    .tokenIssued ∧
  LinkedReceipts
    chain.tokenIssued
    chain.membershipPublished
    .tokenIssued
    .membershipPublished ∧
  LinkedReceipts
    chain.membershipPublished
    chain.readsEnabled
    .membershipPublished
    .readsEnabled

theorem supported_reads_chain_has_rooted_state_receipt
    {scheme : DigestScheme}
    (chain : ReadsEnabledReceiptChain scheme)
    (supported : SupportsReadsEnabled chain) :
    chain.stateInstalled.content.step = .stateInstalled ∧
    chain.stateInstalled.content.predecessorDigest = none :=
  supported.1

theorem supported_reads_chain_has_single_attempt_and_replica
    {scheme : DigestScheme}
    (chain : ReadsEnabledReceiptChain scheme)
    (supported : SupportsReadsEnabled chain) :
    chain.stateInstalled.content.attemptId =
        chain.tokenIssued.content.attemptId ∧
    chain.tokenIssued.content.attemptId =
        chain.membershipPublished.content.attemptId ∧
    chain.membershipPublished.content.attemptId =
        chain.readsEnabled.content.attemptId ∧
    chain.stateInstalled.content.replicaId =
        chain.tokenIssued.content.replicaId ∧
    chain.tokenIssued.content.replicaId =
        chain.membershipPublished.content.replicaId ∧
    chain.membershipPublished.content.replicaId =
        chain.readsEnabled.content.replicaId := by
  rcases supported with ⟨_, stateToken, tokenMembership, membershipReads⟩
  rcases stateToken with ⟨_, _, stateAttempt, stateReplica, _, _⟩
  rcases tokenMembership with ⟨_, _, tokenAttempt, tokenReplica, _, _⟩
  rcases membershipReads with
    ⟨_, _, membershipAttempt, membershipReplica, _, _⟩
  exact
    ⟨ stateAttempt
    , tokenAttempt
    , membershipAttempt
    , stateReplica
    , tokenReplica
    , membershipReplica
    ⟩

theorem supported_reads_chain_is_predecessor_digest_linked
    {scheme : DigestScheme}
    (chain : ReadsEnabledReceiptChain scheme)
    (supported : SupportsReadsEnabled chain) :
    chain.tokenIssued.content.predecessorDigest =
        some chain.stateInstalled.digest ∧
    chain.membershipPublished.content.predecessorDigest =
        some chain.tokenIssued.digest ∧
    chain.readsEnabled.content.predecessorDigest =
        some chain.membershipPublished.digest := by
  rcases supported with ⟨_, stateToken, tokenMembership, membershipReads⟩
  exact ⟨stateToken.2.2.2.2.2, tokenMembership.2.2.2.2.2,
    membershipReads.2.2.2.2.2⟩

theorem cross_attempt_receipts_cannot_support_reads
    {scheme : DigestScheme}
    (chain : ReadsEnabledReceiptChain scheme)
    (crossAttempt :
      chain.stateInstalled.content.attemptId ≠
        chain.tokenIssued.content.attemptId) :
    ¬ SupportsReadsEnabled chain := by
  intro supported
  exact crossAttempt
    (supported_reads_chain_has_single_attempt_and_replica chain supported).1

theorem cross_replica_receipts_cannot_support_reads
    {scheme : DigestScheme}
    (chain : ReadsEnabledReceiptChain scheme)
    (crossReplica :
      chain.tokenIssued.content.replicaId ≠
        chain.membershipPublished.content.replicaId) :
    ¬ SupportsReadsEnabled chain := by
  intro supported
  exact crossReplica
    (supported_reads_chain_has_single_attempt_and_replica chain supported).2.2.2.2.1

theorem detached_predecessor_digest_cannot_support_reads
    {scheme : DigestScheme}
    (chain : ReadsEnabledReceiptChain scheme)
    (detached :
      chain.readsEnabled.content.predecessorDigest ≠
        some chain.membershipPublished.digest) :
    ¬ SupportsReadsEnabled chain := by
  intro supported
  exact detached
    (supported_reads_chain_is_predecessor_digest_linked chain supported).2.2

theorem equal_receipt_digest_implies_equal_content
    {scheme : DigestScheme}
    (left right : DurableReceipt scheme)
    (sameDigest : left.digest = right.digest) :
    left.content = right.content := by
  apply scheme.collisionFree
  calc
    scheme.commit left.content = left.digest := left.bound.symm
    _ = right.digest := sameDigest
    _ = scheme.commit right.content := right.bound

theorem changed_result_cannot_reuse_receipt_digest
    {scheme : DigestScheme}
    (original changed : DurableReceipt scheme)
    (resultChanged :
      original.content.resultDigest ≠ changed.content.resultDigest) :
    original.digest ≠ changed.digest := by
  intro sameDigest
  apply resultChanged
  exact congrArg ReceiptContent.resultDigest
    (equal_receipt_digest_implies_equal_content original changed sameDigest)

end ASPProof.SearchRouteAdmissionRetryCacheRejoinReceiptBinding
