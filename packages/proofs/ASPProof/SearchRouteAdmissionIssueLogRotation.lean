import ASPProof.SearchRouteAdmissionLedgerOrder

namespace ASPProof.SearchRouteAdmissionIssueLogRotation

universe u

/--
The authoritative issue log keeps origin evidence and revocation evidence as
separate predicates. Issuance is historical; acceptability is current policy.
-/
structure IssueLogState (Token : Type u) where
  issued : Token → Prop
  revoked : Token → Prop

def AuthoritativelyIssued
    {Token : Type u}
    (state : IssueLogState Token)
    (token : Token) : Prop :=
  state.issued token

def Acceptable
    {Token : Type u}
    (state : IssueLogState Token)
    (token : Token) : Prop :=
  AuthoritativelyIssued state token ∧ ¬ state.revoked token

/--
A safe rotation may compact representation, but it cannot fabricate issuance,
drop an issued token that remains live, or forget an existing revocation.
-/
structure SafeIssueLogRotation
    {Token : Type u}
    (before after : IssueLogState Token)
    (liveReference : Token → Prop) : Prop where
  noFabrication :
    ∀ token, after.issued token → before.issued token
  liveCoverage :
    ∀ token,
      liveReference token →
      before.issued token →
      after.issued token
  revocationMonotone :
    ∀ token, before.revoked token → after.revoked token

theorem live_reference_resolution_preserved
    {Token : Type u}
    {before after : IssueLogState Token}
    {liveReference : Token → Prop}
    (rotation : SafeIssueLogRotation before after liveReference)
    {token : Token}
    (live : liveReference token)
    (issued : AuthoritativelyIssued before token) :
    AuthoritativelyIssued after token := by
  exact rotation.liveCoverage token live issued

theorem accepted_after_rotation_was_authoritatively_issued_before
    {Token : Type u}
    {before after : IssueLogState Token}
    {liveReference : Token → Prop}
    (rotation : SafeIssueLogRotation before after liveReference)
    {token : Token}
    (accepted : Acceptable after token) :
    AuthoritativelyIssued before token := by
  exact rotation.noFabrication token accepted.1

theorem revoked_token_cannot_be_reaccepted_after_rotation
    {Token : Type u}
    {before after : IssueLogState Token}
    {liveReference : Token → Prop}
    (rotation : SafeIssueLogRotation before after liveReference)
    {token : Token}
    (revoked : before.revoked token) :
    ¬ Acceptable after token := by
  intro accepted
  exact accepted.2 (rotation.revocationMonotone token revoked)

def issuedAllState : IssueLogState Bool :=
  { issued := fun _token => True
    revoked := fun _token => False }

def publicationOnlyState : IssueLogState Bool :=
  { issued := fun token => token = false
    revoked := fun _token => False }

def currentPublicationReference (token : Bool) : Prop :=
  token = false

def pendingRetryReference (token : Bool) : Prop :=
  token = true

theorem publication_only_compaction_can_drop_retry_reference :
    (∀ token,
      currentPublicationReference token →
      issuedAllState.issued token →
      publicationOnlyState.issued token)
      ∧ pendingRetryReference true
      ∧ issuedAllState.issued true
      ∧ ¬ publicationOnlyState.issued true := by
  simp
    [issuedAllState,
      publicationOnlyState,
      currentPublicationReference,
      pendingRetryReference]

def emptyIssueLog : IssueLogState Bool :=
  { issued := fun _token => False
    revoked := fun _token => False }

def fabricatedIssueLog : IssueLogState Bool :=
  { issued := fun _token => True
    revoked := fun _token => False }

def noLiveReference (_token : Bool) : Prop :=
  False

theorem live_coverage_without_no_fabrication_admits_forged_issue_record :
    (∀ token,
      noLiveReference token →
      emptyIssueLog.issued token →
      fabricatedIssueLog.issued token)
      ∧ fabricatedIssueLog.issued true
      ∧ ¬ emptyIssueLog.issued true
      ∧ ¬ SafeIssueLogRotation
        emptyIssueLog
        fabricatedIssueLog
        noLiveReference := by
  refine ⟨?_, by simp [fabricatedIssueLog], by simp [emptyIssueLog], ?_⟩
  · intro token noLive
    exact False.elim noLive
  · intro rotation
    exact
      (by simp [emptyIssueLog] :
        ¬ emptyIssueLog.issued true)
        (rotation.noFabrication true (by simp [fabricatedIssueLog]))

def revokedBeforeRotation : IssueLogState Bool :=
  { issued := fun token => token = true
    revoked := fun token => token = true }

def revocationLostAfterRotation : IssueLogState Bool :=
  { issued := fun token => token = true
    revoked := fun _token => False }

def revokedTokenLiveReference (token : Bool) : Prop :=
  token = true

theorem rotation_that_forgets_revocation_reaccepts_token :
    Acceptable revocationLostAfterRotation true
      ∧ revokedBeforeRotation.revoked true
      ∧ ¬ SafeIssueLogRotation
        revokedBeforeRotation
        revocationLostAfterRotation
        revokedTokenLiveReference := by
  refine
    ⟨by
      simp
        [Acceptable,
          AuthoritativelyIssued,
          revocationLostAfterRotation],
      by simp [revokedBeforeRotation],
      ?_⟩
  intro rotation
  exact
    (by simp [revocationLostAfterRotation] :
      ¬ revocationLostAfterRotation.revoked true)
      (rotation.revocationMonotone true (by simp [revokedBeforeRotation]))

end ASPProof.SearchRouteAdmissionIssueLogRotation
