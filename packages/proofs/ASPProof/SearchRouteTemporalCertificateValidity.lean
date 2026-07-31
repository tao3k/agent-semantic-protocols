namespace ASPProof.SearchRouteTemporalCertificateValidity

structure CertificateLifecycle where
  issuedAt : Nat
  revokedAt : Nat
  archiveAvailable : Bool

def ValidAtExecution
    (lifecycle : CertificateLifecycle)
    (executionAt : Nat) : Prop :=
  lifecycle.issuedAt ≤ executionAt ∧ executionAt < lifecycle.revokedAt

def ValidForHistoricalAudit
    (lifecycle : CertificateLifecycle)
    (executionAt now : Nat) : Prop :=
  ValidAtExecution lifecycle executionAt ∧ executionAt ≤ now

def ValidForHistoricalReplay
    (lifecycle : CertificateLifecycle)
    (executionAt now : Nat) : Prop :=
  ValidForHistoricalAudit lifecycle executionAt now ∧
  lifecycle.archiveAvailable = true

def ValidForNewIssuance
    (lifecycle : CertificateLifecycle)
    (now : Nat) : Prop :=
  lifecycle.issuedAt ≤ now ∧ now < lifecycle.revokedAt

def AuthorizedForNewParetoDecision :=
  ValidForNewIssuance

theorem historical_validity_survives_later_revocation
    {lifecycle : CertificateLifecycle}
    {executionAt now : Nat}
    (validExecution : ValidAtExecution lifecycle executionAt)
    (timeAdvanced : executionAt ≤ now) :
    ValidForHistoricalAudit lifecycle executionAt now :=
  ⟨validExecution, timeAdvanced⟩

theorem revocation_rejects_new_issuance
    {lifecycle : CertificateLifecycle}
    {now : Nat}
    (revoked : lifecycle.revokedAt ≤ now) :
    ¬ ValidForNewIssuance lifecycle now := by
  intro valid
  exact Nat.not_lt_of_ge revoked valid.2

def historicalLifecycle : CertificateLifecycle where
  issuedAt := 5
  revokedAt := 10
  archiveAvailable := true

def missingArchiveLifecycle : CertificateLifecycle where
  issuedAt := 5
  revokedAt := 10
  archiveAvailable := false

theorem execution_before_revocation_is_valid :
    ValidAtExecution historicalLifecycle 7 := by
  exact ⟨by decide, by decide⟩

theorem historical_audit_remains_valid_after_revocation :
    ValidForHistoricalAudit historicalLifecycle 7 12 :=
  historical_validity_survives_later_revocation
    execution_before_revocation_is_valid
    (by decide)

theorem archived_snapshot_enables_historical_replay :
    ValidForHistoricalReplay historicalLifecycle 7 12 :=
  ⟨historical_audit_remains_valid_after_revocation, rfl⟩

theorem revoked_certificate_cannot_issue_new_decision :
    ¬ ValidForNewIssuance historicalLifecycle 12 :=
  revocation_rejects_new_issuance (by decide)

theorem revoked_certificate_cannot_authorize_new_pareto :
    ¬ AuthorizedForNewParetoDecision historicalLifecycle 12 :=
  revoked_certificate_cannot_issue_new_decision

theorem execution_after_revocation_is_invalid :
    ¬ ValidAtExecution historicalLifecycle 12 := by
  intro valid
  exact (by decide : ¬12 < 10) valid.2

theorem missing_archive_blocks_historical_replay :
    ¬ ValidForHistoricalReplay missingArchiveLifecycle 7 12 := by
  intro replay
  exact Bool.noConfusion replay.2

theorem pre_revocation_issuance_is_authorized :
    ValidForNewIssuance historicalLifecycle 8 :=
  ⟨by decide, by decide⟩

end ASPProof.SearchRouteTemporalCertificateValidity
