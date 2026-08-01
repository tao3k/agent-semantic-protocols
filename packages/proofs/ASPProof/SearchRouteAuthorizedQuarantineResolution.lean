namespace ASPProof.SearchRouteAuthorizedQuarantineResolution

inductive ResolutionEvidence where
  | effectCompleted
  | noEffect
  deriving DecidableEq, Repr

structure QuarantineLedger where
  available : Nat
  consumed : Nat
  quarantined : Nat
  deriving DecidableEq, Repr

def ledgerTotal (ledger : QuarantineLedger) : Nat :=
  ledger.available + ledger.consumed + ledger.quarantined

def resolve
    (ledger : QuarantineLedger)
    (evidence : ResolutionEvidence) : QuarantineLedger :=
  match evidence with
  | .effectCompleted =>
      {
        available := ledger.available
        consumed := ledger.consumed + ledger.quarantined
        quarantined := 0
      }
  | .noEffect =>
      {
        available := ledger.available + ledger.quarantined
        consumed := ledger.consumed
        quarantined := 0
      }

theorem resolution_preserves_total
    (ledger : QuarantineLedger)
    (evidence : ResolutionEvidence) :
    ledgerTotal (resolve ledger evidence) = ledgerTotal ledger := by
  cases evidence <;>
    simp [resolve, ledgerTotal, Nat.add_comm, Nat.add_left_comm]

theorem resolution_clears_quarantine
    (ledger : QuarantineLedger)
    (evidence : ResolutionEvidence) :
    (resolve ledger evidence).quarantined = 0 := by
  cases evidence <;> rfl

theorem completed_effect_preserves_available
    (ledger : QuarantineLedger) :
    (resolve ledger .effectCompleted).available = ledger.available := by
  rfl

theorem no_effect_preserves_consumed
    (ledger : QuarantineLedger) :
    (resolve ledger .noEffect).consumed = ledger.consumed := by
  rfl

theorem resolution_evidence_is_exclusive :
    ResolutionEvidence.effectCompleted ≠ ResolutionEvidence.noEffect := by
  decide

structure ResolutionAuthority where
  quarantineDigest : Nat
  requestDigest : Nat
  effectNonce : Nat
  ledgerRevision : Nat
  deriving DecidableEq, Repr

def AuthorityBound
    (expected actual : ResolutionAuthority) : Prop :=
  expected = actual

def resolveAuthorized
    (expected actual : ResolutionAuthority)
    (_bound : AuthorityBound expected actual)
    (ledger : QuarantineLedger)
    (evidence : ResolutionEvidence) : QuarantineLedger :=
  resolve ledger evidence

theorem authorized_resolution_preserves_total
    (expected actual : ResolutionAuthority)
    (bound : AuthorityBound expected actual)
    (ledger : QuarantineLedger)
    (evidence : ResolutionEvidence) :
    ledgerTotal
        (resolveAuthorized expected actual bound ledger evidence) =
      ledgerTotal ledger :=
  resolution_preserves_total ledger evidence

def exampleAuthority : ResolutionAuthority :=
  {
    quarantineDigest := 131
    requestDigest := 111
    effectNonce := 121
    ledgerRevision := 5
  }

def mismatchedAuthority : ResolutionAuthority :=
  { exampleAuthority with ledgerRevision := 4 }

theorem authority_mismatch_rejects_resolution :
    ¬ AuthorityBound exampleAuthority mismatchedAuthority := by
  unfold AuthorityBound exampleAuthority mismatchedAuthority
  decide

def exampleLedger : QuarantineLedger :=
  {
    available := 70
    consumed := 10
    quarantined := 20
  }

theorem example_initial_total_is_one_hundred :
    ledgerTotal exampleLedger = 100 := by
  decide

theorem example_completed_total_is_one_hundred :
    ledgerTotal (resolve exampleLedger .effectCompleted) = 100 := by
  decide

theorem example_no_effect_total_is_one_hundred :
    ledgerTotal (resolve exampleLedger .noEffect) = 100 := by
  decide

theorem example_completed_resolution :
    resolve exampleLedger .effectCompleted =
      {
        available := 70
        consumed := 30
        quarantined := 0
      } := by
  decide

theorem example_no_effect_resolution :
    resolve exampleLedger .noEffect =
      {
        available := 90
        consumed := 10
        quarantined := 0
      } := by
  decide

end ASPProof.SearchRouteAuthorizedQuarantineResolution
