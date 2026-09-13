-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteCertifiedRegistryEpochTransition

namespace ASPProof.SearchRouteCertifiedTransitionChainCompression

open ASPProof.SearchRouteCertifiedRegistryEpochTransition

inductive ValidChain
    (authorizedSuccessor : ClockDomain → ClockDomain → Prop) :
    ClockDomain → ClockDomain → List TransitionCertificate → Prop
  | nil (domain : ClockDomain) :
      ValidChain authorizedSuccessor domain domain []
  | cons
      {fromDomain nextDomain finalDomain : ClockDomain}
      {certificate : TransitionCertificate}
      {rest : List TransitionCertificate}
      (fromMatches : certificate.fromDomain = fromDomain)
      (toMatches : certificate.toDomain = nextDomain)
      (valid : ValidTransition authorizedSuccessor certificate)
      (tail : ValidChain authorizedSuccessor nextDomain finalDomain rest) :
      ValidChain authorizedSuccessor
        fromDomain finalDomain (certificate :: rest)

def AllTransitionsValid
    (authorizedSuccessor : ClockDomain → ClockDomain → Prop) :
    List TransitionCertificate → Prop
  | [] => True
  | certificate :: rest =>
      ValidTransition authorizedSuccessor certificate ∧
        AllTransitionsValid authorizedSuccessor rest

def AllOldSnapshotsArchived : List TransitionCertificate → Prop
  | [] => True
  | certificate :: rest =>
      certificate.oldSnapshotArchived = true ∧
        AllOldSnapshotsArchived rest

def FullyReplayableChain
    (authorizedSuccessor : ClockDomain → ClockDomain → Prop)
    (startDomain endDomain : ClockDomain)
    (certificates : List TransitionCertificate) : Prop :=
  ValidChain authorizedSuccessor startDomain endDomain certificates ∧
    AllOldSnapshotsArchived certificates

structure ChainSummary where
  startDomain : ClockDomain
  endDomain : ClockDomain
  edgeCount : Nat
  chainDigest : Digest
deriving DecidableEq, Repr

def SummaryMatches
    (digestChain : List TransitionCertificate → Digest)
    (summary : ChainSummary)
    (startDomain endDomain : ClockDomain)
    (certificates : List TransitionCertificate) : Prop :=
  summary.startDomain = startDomain ∧
    summary.endDomain = endDomain ∧
    summary.edgeCount = certificates.length ∧
    summary.chainDigest = digestChain certificates

def CertifiedSummary
    (authorizedSuccessor : ClockDomain → ClockDomain → Prop)
    (digestChain : List TransitionCertificate → Digest)
    (summary : ChainSummary)
    (startDomain endDomain : ClockDomain)
    (certificates : List TransitionCertificate) : Prop :=
  ValidChain authorizedSuccessor startDomain endDomain certificates ∧
    SummaryMatches digestChain summary startDomain endDomain certificates

def TrustEdgeCount (certificates : List TransitionCertificate) : Nat :=
  certificates.length

def SummaryPresentationHops
    (certificates : List TransitionCertificate) : Nat :=
  if certificates = [] then 0 else 1

def SameCacheIdentity (left right : ChainSummary) : Prop :=
  left.chainDigest = right.chainDigest ∧
    left.startDomain = right.startDomain ∧
    left.endDomain = right.endDomain ∧
    left.edgeCount = right.edgeCount

theorem valid_chain_append
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    {startDomain middleDomain endDomain : ClockDomain}
    {left right : List TransitionCertificate}
    (leftValid :
      ValidChain authorizedSuccessor startDomain middleDomain left)
    (rightValid :
      ValidChain authorizedSuccessor middleDomain endDomain right) :
    ValidChain authorizedSuccessor
      startDomain endDomain (left ++ right) := by
  induction leftValid with
  | nil =>
      simpa using rightValid
  | cons fromMatches toMatches valid tail inductionHypothesis =>
      exact ValidChain.cons
        fromMatches toMatches valid (inductionHypothesis rightValid)

theorem valid_chain_covers_every_transition
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    {startDomain endDomain : ClockDomain}
    {certificates : List TransitionCertificate}
    (chain :
      ValidChain authorizedSuccessor startDomain endDomain certificates) :
    AllTransitionsValid authorizedSuccessor certificates := by
  induction chain with
  | nil =>
      trivial
  | cons _ _ valid _ inductionHypothesis =>
      exact ⟨valid, inductionHypothesis⟩

theorem valid_chain_edge_count_matches_epoch_span
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    {startDomain endDomain : ClockDomain}
    {certificates : List TransitionCertificate}
    (chain :
      ValidChain authorizedSuccessor startDomain endDomain certificates) :
    startDomain.epoch + certificates.length = endDomain.epoch := by
  induction chain with
  | nil =>
      simp
  | @cons fromDomain nextDomain finalDomain certificate rest
      fromMatches toMatches valid tail inductionHypothesis =>
      have stepEpoch : fromDomain.epoch + 1 = nextDomain.epoch := by
        rw [← fromMatches, ← toMatches]
        exact valid.2.2.2
      calc
        fromDomain.epoch + (certificate :: rest).length =
            (fromDomain.epoch + 1) + rest.length := by
              simp [Nat.add_comm, Nat.add_left_comm]
        _ = nextDomain.epoch + rest.length := by rw [stepEpoch]
        _ = finalDomain.epoch := inductionHypothesis

theorem distinct_epoch_span_requires_nonempty_chain
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    {startDomain endDomain : ClockDomain}
    {certificates : List TransitionCertificate}
    (chain :
      ValidChain authorizedSuccessor startDomain endDomain certificates)
    (differentEpoch : startDomain.epoch ≠ endDomain.epoch) :
    certificates ≠ [] := by
  intro empty
  have countEquation :=
    valid_chain_edge_count_matches_epoch_span chain
  rw [empty] at countEquation
  simp at countEquation
  exact differentEpoch countEquation

theorem missing_head_archive_blocks_full_replay
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    {startDomain endDomain : ClockDomain}
    {certificate : TransitionCertificate}
    {rest : List TransitionCertificate}
    (missing : certificate.oldSnapshotArchived = false) :
    ¬FullyReplayableChain authorizedSuccessor
      startDomain endDomain (certificate :: rest) := by
  intro replayable
  have archived : certificate.oldSnapshotArchived = true :=
    replayable.2.1
  have impossible : false = true := missing.symm.trans archived
  cases impossible

theorem archived_valid_chain_is_fully_replayable
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    {startDomain endDomain : ClockDomain}
    {certificates : List TransitionCertificate}
    (chain :
      ValidChain authorizedSuccessor startDomain endDomain certificates)
    (archived : AllOldSnapshotsArchived certificates) :
    FullyReplayableChain authorizedSuccessor
      startDomain endDomain certificates :=
  ⟨chain, archived⟩

theorem certified_summary_preserves_every_trust_edge
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    {digestChain : List TransitionCertificate → Digest}
    {summary : ChainSummary}
    {startDomain endDomain : ClockDomain}
    {certificates : List TransitionCertificate}
    (certified :
      CertifiedSummary authorizedSuccessor digestChain summary
        startDomain endDomain certificates) :
    AllTransitionsValid authorizedSuccessor certificates :=
  valid_chain_covers_every_transition certified.1

theorem certified_summary_reports_exact_trust_edge_count
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    {digestChain : List TransitionCertificate → Digest}
    {summary : ChainSummary}
    {startDomain endDomain : ClockDomain}
    {certificates : List TransitionCertificate}
    (certified :
      CertifiedSummary authorizedSuccessor digestChain summary
        startDomain endDomain certificates) :
    summary.edgeCount = TrustEdgeCount certificates :=
  certified.2.2.2.1

theorem nonempty_chain_summary_has_one_presentation_hop
    {certificates : List TransitionCertificate}
    (nonempty : certificates ≠ []) :
    SummaryPresentationHops certificates = 1 := by
  simp [SummaryPresentationHops, nonempty]

theorem cache_identity_binds_digest_domains_and_edge_count
    {left right : ChainSummary}
    (sameIdentity : SameCacheIdentity left right) :
    left.chainDigest = right.chainDigest ∧
      left.startDomain = right.startDomain ∧
      left.endDomain = right.endDomain ∧
      left.edgeCount = right.edgeCount :=
  sameIdentity

theorem injective_digest_makes_summary_chain_unique
    {digestChain : List TransitionCertificate → Digest}
    (digestInjective : Function.Injective digestChain)
    {summary : ChainSummary}
    {startDomain endDomain : ClockDomain}
    {left right : List TransitionCertificate}
    (leftMatches :
      SummaryMatches digestChain summary startDomain endDomain left)
    (rightMatches :
      SummaryMatches digestChain summary startDomain endDomain right) :
    left = right := by
  apply digestInjective
  exact leftMatches.2.2.2.symm.trans rightMatches.2.2.2

theorem digest_collision_blocks_unique_chain_identity
    {digestChain : List TransitionCertificate → Digest}
    (collision :
      ∃ left right,
        left ≠ right ∧ digestChain left = digestChain right) :
    ¬Function.Injective digestChain := by
  intro injective
  obtain ⟨left, right, different, sameDigest⟩ := collision
  exact different (injective sameDigest)

theorem certified_summary_cannot_skip_epoch_edges
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    {digestChain : List TransitionCertificate → Digest}
    {summary : ChainSummary}
    {startDomain endDomain : ClockDomain}
    {certificates : List TransitionCertificate}
    (certified :
      CertifiedSummary authorizedSuccessor digestChain summary
        startDomain endDomain certificates) :
    startDomain.epoch + summary.edgeCount = endDomain.epoch := by
  rw [certified_summary_reports_exact_trust_edge_count certified]
  exact valid_chain_edge_count_matches_epoch_span certified.1

end ASPProof.SearchRouteCertifiedTransitionChainCompression
