import ASPProof.AgentSessionLifecycleProduct

namespace ASPProof.MultiAgentLifeSession

open ASPProof.AgentSessionLifecycleProduct

inductive RouteKey where
  | testing
  | explorer
  | configured (value : Nat)
  deriving DecidableEq, Repr

structure SessionName where
  value : Nat
  deriving DecidableEq, Repr

structure PlatformHostAgentName where
  value : Nat
  deriving DecidableEq, Repr

structure ResidentProfile where
  routeKey : RouteKey
  sessionName : SessionName
  platformHostAgentName : PlatformHostAgentName
  deriving DecidableEq, Repr

structure VerifiedResidentBinding where
  sessionName : SessionName
  platformHostAgentName : PlatformHostAgentName
  generation : Nat
  targetVerified : Bool
  deriving DecidableEq, Repr

def bindingMatchesProfile
    (profile : ResidentProfile)
    (binding : VerifiedResidentBinding) : Bool :=
  binding.sessionName == profile.sessionName &&
    binding.platformHostAgentName == profile.platformHostAgentName

def durableResidentDispatchAuthorized
    (profile : ResidentProfile)
    (binding : VerifiedResidentBinding)
    (lifecycle : LifecycleProduct) : Bool :=
  binding.targetVerified &&
    bindingMatchesProfile profile binding &&
    durableDispatchAuthorized lifecycle

inductive ResidentOccupancy where
  | vacant
  | incumbent (lifecycle : LifecycleProduct)
  | replacement (lifecycle : LifecycleProduct)

structure ResidentSlot where
  profile : ResidentProfile
  occupancy : ResidentOccupancy

def residentLiveCount (slot : ResidentSlot) : Nat :=
  match slot.occupancy with
  | .vacant => 0
  | .incumbent _ => 1
  | .replacement _ => 1

structure MultiAgentLifeSession where
  rootSessionId : Nat
  residentSlots : List ResidentSlot
  temporaryAgents : List LifecycleProduct

structure ResidentRegistry where
  resolve : RouteKey → Option ResidentSlot

def installResident
    (registry : ResidentRegistry)
    (slot : ResidentSlot) : ResidentRegistry :=
  { resolve := fun routeKey =>
      if routeKey == slot.profile.routeKey then some slot else registry.resolve routeKey }

def configuredResident
    (registry : ResidentRegistry)
    (routeKey : RouteKey) : Bool :=
  (registry.resolve routeKey).isSome

def emptyRegistry : ResidentRegistry :=
  ⟨fun _ => none⟩

def registryTestingProfile : ResidentProfile :=
  ⟨.testing, ⟨10⟩, ⟨100⟩⟩

def testingSlot : ResidentSlot :=
  ⟨registryTestingProfile, .vacant⟩

theorem installedTestingResidentResolvesByConfiguredRouteKey :
    (installResident emptyRegistry testingSlot).resolve registryTestingProfile.routeKey =
      some testingSlot :=
  rfl

theorem installedTestingResidentPreservesExplorerRoute :
    (installResident emptyRegistry testingSlot).resolve .explorer = none :=
  rfl

theorem unverifiedBindingCannotAuthorizeDispatch
    (profile : ResidentProfile)
    (binding : VerifiedResidentBinding)
    (lifecycle : LifecycleProduct)
    (unverified : binding.targetVerified = false) :
    durableResidentDispatchAuthorized profile binding lifecycle = false := by
  cases binding with
  | mk sessionName platformHostAgentName generation targetVerified =>
      cases targetVerified
      · rfl
      · contradiction

def testingProfile : ResidentProfile :=
  ⟨.testing, ⟨10⟩, ⟨100⟩⟩

def wrongSessionBinding : VerifiedResidentBinding :=
  ⟨⟨11⟩, ⟨100⟩, 7, true⟩

def wrongPlatformTargetBinding : VerifiedResidentBinding :=
  ⟨⟨10⟩, ⟨101⟩, 7, true⟩

theorem wrongSessionNameCannotAuthorizeDispatch
    (lifecycle : LifecycleProduct) :
    durableResidentDispatchAuthorized testingProfile wrongSessionBinding lifecycle = false :=
  rfl

theorem wrongPlatformTargetCannotAuthorizeDispatch
    (lifecycle : LifecycleProduct) :
    durableResidentDispatchAuthorized testingProfile wrongPlatformTargetBinding lifecycle = false :=
  rfl

theorem wellFormedResidentSlotHasAtMostOneLiveGeneration
    (slot : ResidentSlot) :
    residentLiveCount slot ≤ 1 := by
  cases slot with
  | mk profile occupancy =>
      cases occupancy
      · exact Nat.zero_le 1
      · exact Nat.le_refl 1
      · exact Nat.le_refl 1

theorem temporaryAgentsDoNotConsumeResidentSlots
    (session : MultiAgentLifeSession)
    (temporary : LifecycleProduct) :
    ({ session with temporaryAgents := temporary :: session.temporaryAgents }).residentSlots =
      session.residentSlots :=
  rfl

end ASPProof.MultiAgentLifeSession
