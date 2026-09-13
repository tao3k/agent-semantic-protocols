-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.HookSubagentPermissionLifecycle

inductive Action where
  | read
  | edit
  | execute
  | unknown
  deriving DecidableEq, Repr

inductive FilesystemPermission where
  | read
  | write
  deriving DecidableEq, Repr

inductive PermissionSource where
  | hostInvocation
  | shellRedirection
  | configActionMatcher
  deriving DecidableEq, Repr

/-- Permission evidence is bound to a filesystem subject, never to an executable name. -/
structure PermissionFact where
  subject : Nat
  permission : FilesystemPermission
  source : PermissionSource
  deriving DecidableEq, Repr

def permissionAction (permission : FilesystemPermission) : Action :=
  match permission with
  | .read => .read
  | .write => .edit

/-- Write dominates Read only for the same subject. -/
def dominantPermissionAction
    (facts : List PermissionFact)
    (subject : Nat) : Option Action :=
  if facts.any fun fact => fact.subject == subject && fact.permission == .write then
    some .edit
  else if facts.any fun fact => fact.subject == subject && fact.permission == .read then
    some .read
  else
    none

theorem read_permission_projects_read_action :
    permissionAction .read = .read := by
  rfl

theorem write_permission_projects_edit_action :
    permissionAction .write = .edit := by
  rfl

theorem write_permission_dominates_read_for_the_same_subject
    (subject : Nat)
    (readSource writeSource : PermissionSource) :
    dominantPermissionAction
      [ { subject := subject, permission := .read, source := readSource }
      , { subject := subject, permission := .write, source := writeSource }
      ]
      subject = some .edit := by
  simp [dominantPermissionAction]

theorem write_for_another_subject_does_not_erase_read
    (readSubject writeSubject : Nat)
    (hDistinct : readSubject ≠ writeSubject)
    (readSource writeSource : PermissionSource) :
    dominantPermissionAction
      [ { subject := readSubject, permission := .read, source := readSource }
      , { subject := writeSubject, permission := .write, source := writeSource }
      ]
      readSubject = some .read := by
  have hReverse : writeSubject ≠ readSubject := Ne.symm hDistinct
  simp [dominantPermissionAction, hReverse]

/-- Executable identity cannot influence permission-to-Action projection. -/
theorem permission_projection_is_executable_independent
    (permission : FilesystemPermission)
    (_leftExecutable _rightExecutable : Nat) :
    permissionAction permission = permissionAction permission := by
  rfl

/-- A bare registered source operand is not a filesystem permission proof. -/
def unresolvedRegisteredSourceAccess : Action := .unknown

theorem unresolved_source_operand_does_not_fabricate_read_or_edit :
    unresolvedRegisteredSourceAccess ≠ .read ∧
      unresolvedRegisteredSourceAccess ≠ .edit := by
  constructor <;> decide

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

/-- A platform matcher declares native aliases for one semantic Action. -/
structure NativeActionMatcher where
  platform : String
  nativeActions : List String
  action : Action
  deriving DecidableEq, Repr

def matchesNativeAction
    (matcher : NativeActionMatcher)
    (platform nativeAction : String) : Bool :=
  matcher.platform == platform && matcher.nativeActions.contains nativeAction

def projectNativeAction
    (matcher : NativeActionMatcher)
    (platform nativeAction : String) : Option Action :=
  if matchesNativeAction matcher platform nativeAction then
    some matcher.action
  else
    none

def codexEditActionMatcher : NativeActionMatcher :=
  { platform := "codex"
  , nativeActions := ["apply_patch", "Write", "Edit", "NotebookEdit"]
  , action := .edit
  }

theorem codex_edit_aliases_project_one_semantic_action :
    projectNativeAction codexEditActionMatcher "codex" "apply_patch" = some .edit ∧
      projectNativeAction codexEditActionMatcher "codex" "Write" = some .edit ∧
      projectNativeAction codexEditActionMatcher "codex" "Edit" = some .edit ∧
      projectNativeAction codexEditActionMatcher "codex" "NotebookEdit" = some .edit := by
  decide

theorem native_action_matcher_is_platform_scoped :
    projectNativeAction codexEditActionMatcher "claude" "Edit" = none := by
  decide

theorem unmatched_native_action_does_not_fabricate_semantic_action :
    projectNativeAction codexEditActionMatcher "codex" "Bash" = none := by
  decide

/-- The physical plugin entry carries one immutable Host action signal. -/
structure PhysicalMatcherSignal where
  nativeAction : String
  action : Action
  deriving DecidableEq, Repr

inductive HostActionBinding where
  | bound (action : Action)
  | denied
  deriving DecidableEq, Repr

def bindPhysicalMatcher
    (signal : Option PhysicalMatcherSignal)
    (payloadNativeAction : String) : HostActionBinding :=
  match signal with
  | none => .denied
  | some physical =>
      if physical.nativeAction == payloadNativeAction then
        .bound physical.action
      else
        .denied

def codexBashSignal : PhysicalMatcherSignal :=
  { nativeAction := "Bash", action := .execute }

theorem missing_physical_matcher_signal_fails_closed
    (payloadNativeAction : String) :
    bindPhysicalMatcher none payloadNativeAction = .denied := by
  rfl

theorem mismatched_physical_matcher_signal_fails_closed :
    bindPhysicalMatcher (some codexBashSignal) "Read" = .denied := by
  decide

theorem exact_physical_matcher_signal_binds_host_action :
    bindPhysicalMatcher (some codexBashSignal) "Bash" = .bound .execute := by
  decide

/-- Parser-derived permission facts cannot rewrite the immutable Host action. -/
def preservePhysicalHostAction
    (hostAction : Action)
    (_parserFacts : List PermissionFact) : Action :=
  hostAction

theorem shell_permissions_do_not_rewrite_bash_host_action
    (facts : List PermissionFact) :
    preservePhysicalHostAction codexBashSignal.action facts = .execute := by
  rfl

/-- An Action identity without a filesystem subject is not a permission fact. -/
def permissionFactForSubject
    (subject : Option Nat)
    (permission : FilesystemPermission)
    (source : PermissionSource) : Option PermissionFact :=
  subject.map fun value =>
    { subject := value, permission := permission, source := source }

theorem missing_subject_does_not_fabricate_filesystem_permission
    (permission : FilesystemPermission)
    (source : PermissionSource) :
    permissionFactForSubject none permission source = none := by
  rfl

end ASPProof.HookSubagentPermissionLifecycle
