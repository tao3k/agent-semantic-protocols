-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteAdmissionRetryCacheRejoinLinearization

namespace ASPProof.SearchRouteAdmissionRetryCacheRejoinPublicationOutbox

open ASPProof.SearchRouteAdmissionRetryCacheRejoinLinearization
open ASPProof.SearchRouteAdmissionRetryCacheRejoinReplayProtection

inductive PublicationPhase where
  | pending
  | published
  | acknowledged
  deriving DecidableEq, Repr

structure PublicationObligation (Digest : Type) where
  edge : PublishedAdmissionEdge Digest
  phase : PublicationPhase

structure PublicationBundle (Digest : Type) where
  admission : RevisionedAdmissionState Digest
  obligation : PublicationObligation Digest

def commitActivationWithOutbox {Digest : Type}
    (current : RevisionedAdmissionState Digest)
    (claim : AdmissionClaim Digest)
    (allowed :
      CanCommit current
        (activationProposal current.revision claim)) :
    PublicationBundle Digest where
  admission :=
    commit current
      (activationProposal current.revision claim)
      allowed
  obligation :=
    { edge := activationEdge current claim
    , phase := .pending
    }

def PublishablePhase : PublicationPhase → Prop
  | .pending => True
  | .published => True
  | .acknowledged => False

def CanPublish {Digest : Type}
    (bundle : PublicationBundle Digest) : Prop :=
  EdgeAuthorized bundle.admission bundle.obligation.edge ∧
  PublishablePhase bundle.obligation.phase

def publish {Digest : Type}
    (bundle : PublicationBundle Digest)
    (_ : CanPublish bundle) :
    PublicationBundle Digest where
  admission := bundle.admission
  obligation :=
    { edge := bundle.obligation.edge
    , phase := .published
    }

def CanAcknowledge {Digest : Type}
    (bundle : PublicationBundle Digest) : Prop :=
  EdgeAuthorized bundle.admission bundle.obligation.edge ∧
  bundle.obligation.phase = .published

def acknowledge {Digest : Type}
    (bundle : PublicationBundle Digest)
    (_ : CanAcknowledge bundle) :
    PublicationBundle Digest where
  admission := bundle.admission
  obligation :=
    { edge := bundle.obligation.edge
    , phase := .acknowledged
    }

theorem atomic_activation_commit_creates_authorized_pending_obligation
    {Digest : Type}
    (current : RevisionedAdmissionState Digest)
    (claim : AdmissionClaim Digest)
    (allowed :
      CanCommit current
        (activationProposal current.revision claim)) :
    EdgeAuthorized
      (commitActivationWithOutbox current claim allowed).admission
      (commitActivationWithOutbox current claim allowed).obligation.edge ∧
    (commitActivationWithOutbox current claim allowed).obligation.phase =
      .pending := by
  constructor
  · exact activation_commit_authorizes_bound_graph_edge current claim allowed
  · rfl

theorem pending_commit_is_recoverably_publishable
    {Digest : Type}
    (current : RevisionedAdmissionState Digest)
    (claim : AdmissionClaim Digest)
    (allowed :
      CanCommit current
        (activationProposal current.revision claim)) :
    CanPublish (commitActivationWithOutbox current claim allowed) := by
  constructor
  · exact
      (atomic_activation_commit_creates_authorized_pending_obligation
        current claim allowed).1
  · trivial

theorem publication_preserves_edge_and_marks_published
    {Digest : Type}
    (bundle : PublicationBundle Digest)
    (publishable : CanPublish bundle) :
    (publish bundle publishable).obligation.edge =
        bundle.obligation.edge ∧
    (publish bundle publishable).obligation.phase = .published :=
  ⟨rfl, rfl⟩

theorem published_bundle_remains_retryable
    {Digest : Type}
    (bundle : PublicationBundle Digest)
    (publishable : CanPublish bundle) :
    CanPublish (publish bundle publishable) := by
  constructor
  · exact publishable.1
  · trivial

theorem publication_retry_is_idempotent
    {Digest : Type}
    (bundle : PublicationBundle Digest)
    (firstAllowed : CanPublish bundle)
    (retryAllowed : CanPublish (publish bundle firstAllowed)) :
    publish (publish bundle firstAllowed) retryAllowed =
      publish bundle firstAllowed :=
  rfl

theorem stale_revision_obligation_cannot_publish
    {Digest : Type}
    (bundle : PublicationBundle Digest)
    (stale :
      bundle.obligation.edge.committedRevision <
        bundle.admission.revision) :
    ¬ CanPublish bundle := by
  intro publishable
  exact
    (stale_revision_edge_is_not_authorized
      bundle.admission bundle.obligation.edge stale)
      publishable.1

theorem published_bundle_can_be_acknowledged
    {Digest : Type}
    (bundle : PublicationBundle Digest)
    (publishable : CanPublish bundle) :
    CanAcknowledge (publish bundle publishable) := by
  constructor
  · exact publishable.1
  · rfl

theorem acknowledgement_preserves_edge_and_closes_obligation
    {Digest : Type}
    (bundle : PublicationBundle Digest)
    (acknowledgeable : CanAcknowledge bundle) :
    (acknowledge bundle acknowledgeable).obligation.edge =
        bundle.obligation.edge ∧
    (acknowledge bundle acknowledgeable).obligation.phase =
        .acknowledged :=
  ⟨rfl, rfl⟩

theorem acknowledged_bundle_cannot_publish_again
    {Digest : Type}
    (bundle : PublicationBundle Digest)
    (acknowledgeable : CanAcknowledge bundle) :
    ¬ CanPublish (acknowledge bundle acknowledgeable) := by
  intro publishable
  exact publishable.2

theorem acknowledged_bundle_cannot_close_twice
    {Digest : Type}
    (bundle : PublicationBundle Digest)
    (acknowledgeable : CanAcknowledge bundle) :
    ¬ CanAcknowledge (acknowledge bundle acknowledgeable) := by
  intro secondAcknowledgement
  cases secondAcknowledgement.2

end ASPProof.SearchRouteAdmissionRetryCacheRejoinPublicationOutbox
