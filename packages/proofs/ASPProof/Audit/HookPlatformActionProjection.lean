-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.HookPlatformActionProjection

namespace ASPProof.Audit.HookPlatformActionProjection

open ASPProof.HookEnforcementKernel
open ASPProof.HookPlatformActionProjection

def orgReadEnvelope (observed : Bool) : PlatformEnvelope :=
  { hostInvocation := .read
    semanticCapabilities := [.read]
    profiles := ["org"]
    subjects := ["docs/plan.org"]
    hookEventObserved := observed }

def orgReadDenyRule : Rule :=
  { hostInvocationsAny := [.read]
    semanticCapabilitiesAny := [.read]
    profilesAny := ["org"]
    decision := .deny }

def opaqueExecuteAction : ActionIR :=
  { hostInvocation := .execute
    semanticCapabilities := [.execute]
    profiles := ["rust"]
    subjects := ["src/lib.rs"] }

example :
    (withSubjects opaqueExecuteAction ["src/other.rs"]).semanticCapabilities = [.execute] := by
  decide

example : evaluate (orgReadEnvelope true) [orgReadDenyRule] = some .deny := by
  decide

example : evaluate (orgReadEnvelope false) [orgReadDenyRule] = none := by
  decide

example (executed : Bool)
    (failClosed : HostFailClosed (orgReadEnvelope false) executed) :
    executed = false := by
  exact fail_closed_unobserved_action_does_not_execute
    (orgReadEnvelope false) executed failClosed rfl

end ASPProof.Audit.HookPlatformActionProjection
