import ASPProof.ASPAgentLifeSessionSingleResident

namespace ASPProof.ASPAgentSessionRegistryAuthority

open ASPAgentSessionPortal
open ASPAgentLifeSessionClose
open ASPAgentLifeSessionSingleResident

inductive RuntimeMode where
  | normal
  | offlineRecovery
  deriving DecidableEq

inductive RegistryOwner where
  | unowned
  | server
  | cli
  deriving DecidableEq

inductive RegistryRoute where
  | serverIpc
  | directFile
  deriving DecidableEq

inductive RegistryTransport where
  | ready
  | locked
  | unavailable
  deriving DecidableEq

def accessAdmitted
    (mode : RuntimeMode)
    (owner : RegistryOwner)
    (route : RegistryRoute)
    (transport : RegistryTransport) : Flag :=
  match mode with
  | .normal =>
      match owner with
      | .unowned => .no
      | .cli => .no
      | .server =>
          match route with
          | .directFile => .no
          | .serverIpc =>
              match transport with
              | .ready => .yes
              | .locked => .no
              | .unavailable => .no
  | .offlineRecovery =>
      match owner with
      | .unowned => .no
      | .server => .no
      | .cli =>
          match route with
          | .serverIpc => .no
          | .directFile =>
              match transport with
              | .ready => .yes
              | .locked => .no
              | .unavailable => .no

structure ControlPlaneState where
  serverGeneration : Nat
  sessionGeneration : Nat
  bindingDelivered : Flag
  pathReleaseReceipt : Flag

def restartServer (state : ControlPlaneState) (newServerGeneration : Nat) : ControlPlaneState :=
  { state with serverGeneration := newServerGeneration }

theorem normalServerIpcIsAdmitted :
    accessAdmitted .normal .server .serverIpc .ready = .yes := by
  rfl

theorem normalServerOwnerRejectsDirectFile :
    accessAdmitted .normal .server .directFile .ready = .no := by
  rfl

theorem serverLockRejectsDirectFile :
    accessAdmitted .normal .server .directFile .locked = .no := by
  rfl

theorem offlineDirectFileRequiresCliOwnership :
    accessAdmitted .offlineRecovery .unowned .directFile .ready = .no ∧
      accessAdmitted .offlineRecovery .server .directFile .ready = .no ∧
      accessAdmitted .offlineRecovery .cli .directFile .ready = .yes := by
  exact ⟨rfl, rfl, rfl⟩

theorem restartPreservesSessionAuthority
    (state : ControlPlaneState)
    (newServerGeneration : Nat) :
    (restartServer state newServerGeneration).sessionGeneration = state.sessionGeneration ∧
      (restartServer state newServerGeneration).bindingDelivered = state.bindingDelivered ∧
      (restartServer state newServerGeneration).pathReleaseReceipt = state.pathReleaseReceipt := by
  exact ⟨rfl, rfl, rfl⟩

theorem registryLockFailureCannotAuthorizeReplacement
    (state : CloseState)
    (transport : RegistryTransport)
    (_locked : transport = .locked)
    (notReleased : state.pathReleaseReceipt = .no) :
    replacementAuthorized state = .no := by
  exact missingPathReleaseRejectsReplacement state notReleased

theorem registryLockFailureCannotReserveSameName
    (state : CloseState)
    (newGeneration : Nat)
    (transport : RegistryTransport)
    (_locked : transport = .locked)
    (notReleased : state.pathReleaseReceipt = .no) :
    reserveReplacement state newGeneration = none := by
  exact missingPathReleaseCannotReserveSameName state newGeneration notReleased

end ASPProof.ASPAgentSessionRegistryAuthority
