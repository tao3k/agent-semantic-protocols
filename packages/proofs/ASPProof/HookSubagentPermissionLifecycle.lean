namespace ASPProof.HookSubagentPermissionLifecycle

inductive Action where
  | read
  | edit
  | execute
  deriving DecidableEq, Repr

inductive Decision where
  | allow
  | denyMissingReceipt
  | denyReadOnlyEdit
  | denyProfileScope
  | denyOtherPolicy
  deriving DecidableEq, Repr

inductive RuleIntent where
  | reasoningSearch
  | testBuildCommand
  deriving DecidableEq, Repr

/-- The typed Runtime IPC result committed by the SubagentStart lifecycle. -/
structure DbRegistration where
  childSessionId : Nat
  agentName : Nat
  profileDigest : Nat
  denyEdit : Bool
  allowedIntent : RuleIntent
  deriving DecidableEq, Repr

/-- The synchronous local projection consumed by PreTool. -/
structure PermissionReceipt where
  childSessionId : Nat
  agentName : Nat
  profileDigest : Nat
  verified : Bool
  denyEdit : Bool
  allowedIntent : RuleIntent
  deriving DecidableEq, Repr

def publishPermissionReceipt
    (registration : Option DbRegistration) : Option PermissionReceipt :=
  registration.map fun record =>
    { childSessionId := record.childSessionId
      agentName := record.agentName
      profileDigest := record.profileDigest
      verified := true
      denyEdit := record.denyEdit
      allowedIntent := record.allowedIntent }

/-- PreTool is deliberately a pure receipt and Action IR decision. It has no DB input. -/
def enforcePreTool
    (receipt : Option PermissionReceipt)
    (action : Action)
    (intent : Option RuleIntent)
    (lowerDecision : Decision) : Decision :=
  match receipt with
  | none => .denyMissingReceipt
  | some permission =>
      if permission.verified && permission.denyEdit && action == .edit then
        .denyReadOnlyEdit
      else if permission.verified && intent == some permission.allowedIntent then
        lowerDecision
      else
        .denyProfileScope

theorem missing_db_registration_cannot_publish_permission_receipt :
    publishPermissionReceipt none = none := by
  rfl

theorem db_registration_projects_exact_permission_identity
    (registration : DbRegistration) :
    publishPermissionReceipt (some registration) = some {
      childSessionId := registration.childSessionId
      agentName := registration.agentName
      profileDigest := registration.profileDigest
      verified := true
      denyEdit := registration.denyEdit
      allowedIntent := registration.allowedIntent
    } := by
  rfl

theorem missing_receipt_fails_closed_for_edit
    (lowerDecision : Decision) :
    enforcePreTool none .edit none lowerDecision = .denyMissingReceipt := by
  rfl

theorem verified_read_only_edit_dominates_lower_policy
    (receipt : PermissionReceipt)
    (hVerified : receipt.verified = true)
    (hDenyEdit : receipt.denyEdit = true)
    (intent : Option RuleIntent)
    (lowerDecision : Decision) :
    enforcePreTool (some receipt) .edit intent lowerDecision = .denyReadOnlyEdit := by
  simp [enforcePreTool, hVerified, hDenyEdit]

theorem verified_profile_allows_only_its_declared_rule_intent
    (receipt : PermissionReceipt)
    (hVerified : receipt.verified = true)
    (lowerDecision : Decision) :
    enforcePreTool (some receipt) .read (some receipt.allowedIntent) lowerDecision =
      lowerDecision := by
  simp [enforcePreTool, hVerified]

theorem verified_profile_denies_unscoped_execution
    (receipt : PermissionReceipt)
    (hVerified : receipt.verified = true)
    (lowerDecision : Decision) :
    enforcePreTool (some receipt) .execute none lowerDecision = .denyProfileScope := by
  simp [enforcePreTool, hVerified]

theorem pretool_decision_is_independent_of_runtime_state
    (receipt : Option PermissionReceipt)
    (action : Action)
    (intent : Option RuleIntent)
    (lowerDecision : Decision)
    (_leftRuntimeState _rightRuntimeState : Nat) :
    enforcePreTool receipt action intent lowerDecision =
      enforcePreTool receipt action intent lowerDecision := by
  rfl

end ASPProof.HookSubagentPermissionLifecycle
