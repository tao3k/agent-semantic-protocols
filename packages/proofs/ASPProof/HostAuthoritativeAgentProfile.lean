namespace ASPProof.HostAuthoritativeAgentProfile

inductive ResidentDecision where
  | ready
  | residentCommandBlocked
deriving DecidableEq, Repr

structure HostReceipt where
  typedRoleMatches : Bool
  uniqueCanonicalPath : Bool
  liveBindingFresh : Bool
deriving DecidableEq, Repr

structure LegacyManagerProjection where
  expectedModel : String
  expectedReasoning : String
deriving DecidableEq, Repr

def admit (receipt : HostReceipt) : ResidentDecision :=
  if receipt.typedRoleMatches && receipt.uniqueCanonicalPath && receipt.liveBindingFresh then
    .ready
  else
    .residentCommandBlocked

def admitWithLegacyIgnored
    (receipt : HostReceipt) (_projection : LegacyManagerProjection) : ResidentDecision :=
  admit receipt

theorem admission_independent_of_legacy_projection
    (receipt : HostReceipt) (left right : LegacyManagerProjection) :
    admitWithLegacyIgnored receipt left = admitWithLegacyIgnored receipt right := rfl

theorem valid_host_receipt_is_ready
    (receipt : HostReceipt)
    (role : receipt.typedRoleMatches = true)
    (unique : receipt.uniqueCanonicalPath = true)
    (fresh : receipt.liveBindingFresh = true) :
    admit receipt = .ready := by
  simp [admit, role, unique, fresh]

theorem missing_host_evidence_blocks_only_resident_command
    (receipt : HostReceipt)
    (missing : receipt.liveBindingFresh = false) :
    admit receipt = .residentCommandBlocked := by
  simp [admit, missing]

def legacyAdmit
    (receipt : HostReceipt)
    (projection : LegacyManagerProjection)
    (observedModel observedReasoning : String) : ResidentDecision :=
  if receipt.typedRoleMatches
      && receipt.uniqueCanonicalPath
      && receipt.liveBindingFresh
      && projection.expectedModel == observedModel
      && projection.expectedReasoning == observedReasoning then
    .ready
  else
    .residentCommandBlocked

theorem legacy_shadow_projection_can_block_valid_host_receipt :
    let receipt : HostReceipt := {
      typedRoleMatches := true
      uniqueCanonicalPath := true
      liveBindingFresh := true
    }
    let stale : LegacyManagerProjection := {
      expectedModel := "legacy-model"
      expectedReasoning := "low"
    }
    admit receipt = .ready ∧
      legacyAdmit receipt stale "host-model" "low" = .residentCommandBlocked := by
  decide

end ASPProof.HostAuthoritativeAgentProfile
