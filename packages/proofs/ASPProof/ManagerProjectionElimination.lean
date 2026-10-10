-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.ManagerProjectionElimination

structure AgentRoute where
  agentType : String
  hostAgentName : String
  roles : List String
  deriving DecidableEq, Repr

structure HostLifecycleAttestation where
  agentType : String
  canonicalPath : String
  childId : String
  bindingFresh : Bool
  deriving DecidableEq, Repr

structure LegacyManagerProjection where
  profilePath : String
  projectedModel : String
  projectedReasoning : String
  projectedSandbox : String
  deriving DecidableEq, Repr

def admits (route : AgentRoute) (receipt : HostLifecycleAttestation) : Bool :=
  route.agentType == receipt.agentType &&
    !route.hostAgentName.isEmpty &&
    !receipt.canonicalPath.isEmpty &&
    !receipt.childId.isEmpty &&
    receipt.bindingFresh

def legacyErasedAdmission
    (route : AgentRoute)
    (receipt : HostLifecycleAttestation)
    (_legacy : LegacyManagerProjection) : Bool :=
  admits route receipt

theorem admission_has_no_manager_projection_input
    (route : AgentRoute)
    (receipt : HostLifecycleAttestation)
    (left right : LegacyManagerProjection) :
    legacyErasedAdmission route receipt left =
      legacyErasedAdmission route receipt right := by
  rfl

def legacyShadowAdmission
    (route : AgentRoute)
    (receipt : HostLifecycleAttestation)
    (expectedModel observedModel : String) : Bool :=
  admits route receipt && expectedModel == observedModel

theorem legacy_shadow_can_reject_a_valid_host_receipt :
    admits
        { agentType := "asp_testing", hostAgentName := "asp_testing", roles := ["testing"] }
        { agentType := "asp_testing", canonicalPath := "/root/asp_testing",
          childId := "child-1", bindingFresh := true } = true ∧
      legacyShadowAdmission
        { agentType := "asp_testing", hostAgentName := "asp_testing", roles := ["testing"] }
        { agentType := "asp_testing", canonicalPath := "/root/asp_testing",
          childId := "child-1", bindingFresh := true }
        "legacy-model" "host-model" = false := by
  decide

end ASPProof.ManagerProjectionElimination
