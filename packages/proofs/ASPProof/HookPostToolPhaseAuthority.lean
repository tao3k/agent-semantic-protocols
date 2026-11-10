-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.HookPostToolPhaseAuthority

inductive HookPhase
  | preTool
  | postTool
  deriving DecidableEq

inductive PermissionDecision
  | allow
  | deny
  deriving DecidableEq

def permissionFor : HookPhase → PermissionDecision
  | .preTool => .deny
  | .postTool => .allow

theorem postToolCannotRetroactivelyDeny :
    permissionFor .postTool = .allow := by
  rfl

theorem preToolOwnsAuthorization :
    permissionFor .preTool = .deny := by
  rfl

structure MutationIdentity where
  mutationId : String
  changedOwners : List String
  deriving DecidableEq

structure PostToolTransition where
  identity : MutationIdentity
  submissionCount : Nat
  permission : PermissionDecision

def observeMutation (identity : MutationIdentity) : PostToolTransition :=
  { identity
    submissionCount := 1
    permission := .allow }

theorem postToolMutationHasOneSubmission (identity : MutationIdentity) :
    (observeMutation identity).submissionCount = 1 := by
  rfl

theorem postToolMutationPreservesIdentity (identity : MutationIdentity) :
    (observeMutation identity).identity = identity := by
  rfl

theorem postToolMutationCannotDeny (identity : MutationIdentity) :
    (observeMutation identity).permission = .allow := by
  rfl

end ASPProof.HookPostToolPhaseAuthority
