-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.AgentSessionCapabilityAdmissibility

inductive AgentKind where
  | explorer
  | testing
  | temporary
  deriving DecidableEq, Repr

inductive AgentCapability where
  | semanticSearch
  | testExecution
  | workspaceWrite
  deriving DecidableEq, Repr

structure CapabilityRegistration where
  kind : AgentKind
  hostBound : Bool
  temporaryKindRegistered : Bool
  allows : AgentCapability → Bool

def registeredKind (registration : CapabilityRegistration) : Bool :=
  match registration.kind with
  | .temporary => registration.temporaryKindRegistered
  | .explorer | .testing => true

def dispatchAdmissible
    (registration : Option CapabilityRegistration)
    (requested : AgentCapability) : Bool :=
  match registration with
  | none => false
  | some current =>
      current.hostBound && registeredKind current && current.allows requested

theorem unregistered_agent_is_not_admissible (requested : AgentCapability) :
    dispatchAdmissible none requested = false := rfl

theorem unbound_registration_is_not_admissible
    (registration : CapabilityRegistration)
    (requested : AgentCapability)
    (unbound : registration.hostBound = false) :
    dispatchAdmissible (some registration) requested = false := by
  simp [dispatchAdmissible, unbound]

theorem unregistered_temporary_kind_is_not_admissible
    (registration : CapabilityRegistration)
    (isTemporary : registration.kind = .temporary)
    (unregistered : registration.temporaryKindRegistered = false)
    (requested : AgentCapability) :
    dispatchAdmissible (some registration) requested = false := by
  simp [dispatchAdmissible, registeredKind, isTemporary, unregistered]

theorem missing_capability_is_not_admissible
    (registration : CapabilityRegistration)
    (requested : AgentCapability)
    (missing : registration.allows requested = false) :
    dispatchAdmissible (some registration) requested = false := by
  simp [dispatchAdmissible, missing]

theorem admitted_dispatch_has_host_binding
    (registration : CapabilityRegistration)
    (requested : AgentCapability)
    (admitted : dispatchAdmissible (some registration) requested = true) :
    registration.hostBound = true := by
  simp [dispatchAdmissible] at admitted
  exact admitted.1.1

theorem admitted_dispatch_has_registered_kind
    (registration : CapabilityRegistration)
    (requested : AgentCapability)
    (admitted : dispatchAdmissible (some registration) requested = true) :
    registeredKind registration = true := by
  simp [dispatchAdmissible] at admitted
  exact admitted.1.2

theorem admitted_dispatch_has_requested_capability
    (registration : CapabilityRegistration)
    (requested : AgentCapability)
    (admitted : dispatchAdmissible (some registration) requested = true) :
    registration.allows requested = true := by
  simp [dispatchAdmissible] at admitted
  exact admitted.2

end ASPProof.AgentSessionCapabilityAdmissibility
