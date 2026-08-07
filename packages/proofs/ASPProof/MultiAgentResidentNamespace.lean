import ASPProof.MultiAgentHostIdentityBinding

namespace ASPProof.MultiAgentResidentNamespace

open ASPProof.MultiAgentChoicePlane

def configuredResidentIds : List ConfiguredResidentId :=
  [.aspExplorer, .aspTesting]

theorem configured_resident_ids_are_exact :
    configuredResidentIds = [.aspExplorer, .aspTesting] := by
  rfl

theorem configured_resident_ids_are_unique : configuredResidentIds.Nodup := by
  decide

structure RootResidentNamespace where
  rootSessionId : Nat
  aspExplorer : Option ResidentGenerationFact
  aspTesting : Option ResidentGenerationFact
  deriving DecidableEq, Repr

def RootResidentNamespace.lookup
    (rootState : RootResidentNamespace)
    (residentId : ConfiguredResidentId) : Option ResidentGenerationFact :=
  match residentId with
  | .aspExplorer => rootState.aspExplorer
  | .aspTesting => rootState.aspTesting

theorem root_namespace_has_one_slot_per_configured_resident
    (rootState : RootResidentNamespace) :
    rootState.lookup .aspExplorer = rootState.aspExplorer ∧
      rootState.lookup .aspTesting = rootState.aspTesting := by
  constructor <;> rfl

inductive AspMatchDecision where
  | none
  | resident (residentId : ConfiguredResidentId)
  deriving DecidableEq, Repr

structure ExplicitTemporaryAuthority where
  parentBinding : ExactAgentBinding
  reasonDigest : Nat
  issuedAtMs : Nat
  expiresAtMs : Nat
  deriving DecidableEq, Repr

def ExplicitTemporaryAuthority.validAt
    (authority : ExplicitTemporaryAuthority)
    (expectedParent : ExactAgentBinding)
    (nowMs : Nat) : Bool :=
  authority.parentBinding == expectedParent &&
    decide (authority.reasonDigest ≠ 0) &&
    decide (authority.issuedAtMs ≤ nowMs) &&
    decide (nowMs < authority.expiresAtMs)

structure RootDispatchObservation where
  rootState : RootResidentNamespace
  matchDecision : AspMatchDecision
  expectedBinding : Option ExactAgentBinding
  temporaryAuthority : Option ExplicitTemporaryAuthority
  nowMs : Nat
  deriving DecidableEq, Repr

inductive RootDispatchAdmission where
  | nonAspWorker
  | resumeResident (residentId : ConfiguredResidentId)
  | createResident (residentId : ConfiguredResidentId)
  | createTemporary (residentId : ConfiguredResidentId)
  | rejectBindingDrift (residentId : ConfiguredResidentId)
  | rejectTemporaryAuthority (residentId : ConfiguredResidentId)
  deriving DecidableEq, Repr

def rootDispatchAdmission
    (observation : RootDispatchObservation) : RootDispatchAdmission :=
  match observation.matchDecision with
  | .none => .nonAspWorker
  | .resident residentId =>
      match observation.expectedBinding with
      | none => .rejectBindingDrift residentId
      | some expected =>
          match observation.rootState.lookup residentId with
          | some resident =>
              if registrationMatches expected resident then
                .resumeResident residentId
              else
                .rejectBindingDrift residentId
          | none =>
              match observation.temporaryAuthority with
              | none => .createResident residentId
              | some authority =>
                  if authority.validAt expected observation.nowMs then
                    .createTemporary residentId
                  else
                    .rejectTemporaryAuthority residentId

theorem matching_live_resident_is_always_resumed
    (observation : RootDispatchObservation)
    (residentId : ConfiguredResidentId)
    (expected : ExactAgentBinding)
    (resident : ResidentGenerationFact)
    (matched : observation.matchDecision = .resident residentId)
    (expectedPresent : observation.expectedBinding = some expected)
    (residentPresent : observation.rootState.lookup residentId = some resident)
    (registered : registeredBinding expected resident) :
    rootDispatchAdmission observation = .resumeResident residentId := by
  simp [rootDispatchAdmission, matched, expectedPresent, residentPresent,
    registrationMatches, registered]

theorem matching_live_resident_cannot_create_temporary
    (observation : RootDispatchObservation)
    (residentId : ConfiguredResidentId)
    (expected : ExactAgentBinding)
    (resident : ResidentGenerationFact)
    (matched : observation.matchDecision = .resident residentId)
    (expectedPresent : observation.expectedBinding = some expected)
    (residentPresent : observation.rootState.lookup residentId = some resident)
    (registered : registeredBinding expected resident) :
    rootDispatchAdmission observation ≠ .createTemporary residentId := by
  rw [matching_live_resident_is_always_resumed observation residentId expected resident
    matched expectedPresent residentPresent registered]
  intro impossible
  cases impossible

theorem absent_resident_without_temporary_authority_creates_resident
    (observation : RootDispatchObservation)
    (residentId : ConfiguredResidentId)
    (expected : ExactAgentBinding)
    (matched : observation.matchDecision = .resident residentId)
    (expectedPresent : observation.expectedBinding = some expected)
    (residentAbsent : observation.rootState.lookup residentId = none)
    (temporaryAbsent : observation.temporaryAuthority = none) :
    rootDispatchAdmission observation = .createResident residentId := by
  simp [rootDispatchAdmission, matched, expectedPresent, residentAbsent, temporaryAbsent]

theorem temporary_without_explicit_authority_cannot_be_created
    (observation : RootDispatchObservation)
    (residentId : ConfiguredResidentId)
    (expected : ExactAgentBinding)
    (matched : observation.matchDecision = .resident residentId)
    (expectedPresent : observation.expectedBinding = some expected)
    (residentAbsent : observation.rootState.lookup residentId = none)
    (temporaryAbsent : observation.temporaryAuthority = none) :
    rootDispatchAdmission observation ≠ .createTemporary residentId := by
  simp [rootDispatchAdmission, matched, expectedPresent, residentAbsent, temporaryAbsent]

theorem valid_explicit_temporary_authority_is_required_and_sufficient
    (observation : RootDispatchObservation)
    (residentId : ConfiguredResidentId)
    (expected : ExactAgentBinding)
    (authority : ExplicitTemporaryAuthority)
    (matched : observation.matchDecision = .resident residentId)
    (expectedPresent : observation.expectedBinding = some expected)
    (residentAbsent : observation.rootState.lookup residentId = none)
    (temporaryPresent : observation.temporaryAuthority = some authority)
    (valid : authority.validAt expected observation.nowMs = true) :
    rootDispatchAdmission observation = .createTemporary residentId := by
  simp [rootDispatchAdmission, matched, expectedPresent, residentAbsent, temporaryPresent, valid]

theorem nonmatched_work_never_acquires_asp_resident_authority
    (observation : RootDispatchObservation)
    (notMatched : observation.matchDecision = .none) :
    rootDispatchAdmission observation = .nonAspWorker := by
  unfold rootDispatchAdmission
  rw [notMatched]

end ASPProof.MultiAgentResidentNamespace
