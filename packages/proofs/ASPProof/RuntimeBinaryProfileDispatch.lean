-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.RuntimeBinaryProfileDispatch

inductive BinaryProfile where
  | facade
  | providerInternal
  | supportTool
  | hostTool
  | testFixture
  deriving DecidableEq, Repr

structure BinaryRegistration where
  binary : String
  profile : BinaryProfile
  provider : String
  language : String
  generation : Nat
  artifactDigest : String
  registered : Bool
  deriving DecidableEq, Repr

structure DispatchRequest where
  rootSession : String
  childSession : String
  binary : String
  profile : BinaryProfile
  provider : String
  language : String
  generation : Nat
  artifactDigest : String
  argvDigest : String
  attempt : Nat
  directAgent : Bool
  facadePolicyAuthorized : Bool
  supportPolicyAuthorized : Bool
  hostPolicyAuthorized : Bool
  testRuntime : Bool
  deriving DecidableEq, Repr

structure DispatchCapability where
  dispatchId : String
  rootSession : String
  childSession : String
  binary : String
  profile : BinaryProfile
  provider : String
  language : String
  generation : Nat
  artifactDigest : String
  argvDigest : String
  attempt : Nat
  issuedAt : Nat
  expiresAt : Nat
  consumed : Bool
  deriving DecidableEq, Repr

structure SessionClock where
  rootSession : String
  childSession : String
  now : Nat
  deriving DecidableEq, Repr

def RegistrationMatches
    (registration : BinaryRegistration)
    (request : DispatchRequest) : Prop :=
  registration.registered = true ∧
    registration.binary = request.binary ∧
    registration.profile = request.profile ∧
    registration.provider = request.provider ∧
    registration.language = request.language ∧
    registration.generation = request.generation ∧
    registration.artifactDigest = request.artifactDigest

def SessionMatches
    (session : SessionClock)
    (request : DispatchRequest)
    (capability : DispatchCapability) : Prop :=
  session.rootSession = request.rootSession ∧
    session.childSession = request.childSession ∧
    capability.rootSession = request.rootSession ∧
    capability.childSession = request.childSession

def DispatchIdentityMatches
    (request : DispatchRequest)
    (capability : DispatchCapability) : Prop :=
  capability.binary = request.binary ∧
    capability.profile = request.profile ∧
    capability.provider = request.provider ∧
    capability.language = request.language ∧
    capability.generation = request.generation ∧
    capability.artifactDigest = request.artifactDigest ∧
    capability.argvDigest = request.argvDigest ∧
    capability.attempt = request.attempt

def CapabilityLive
    (session : SessionClock)
    (capability : DispatchCapability) : Prop :=
  capability.issuedAt ≤ session.now ∧
    session.now < capability.expiresAt ∧
    capability.consumed = false

def CapabilityMatches
    (session : SessionClock)
    (request : DispatchRequest)
    (capability : DispatchCapability) : Prop :=
  SessionMatches session request capability ∧
    DispatchIdentityMatches request capability ∧
    CapabilityLive session capability

def HasMatchingCapability
    (session : SessionClock)
    (request : DispatchRequest)
    (candidate : Option DispatchCapability) : Prop :=
  ∃ capability,
    candidate = some capability ∧ CapabilityMatches session request capability

def ProfileAdmission
    (session : SessionClock)
    (request : DispatchRequest)
    (capability : Option DispatchCapability) : Prop :=
  match request.profile with
  | .facade => request.facadePolicyAuthorized = true
  | .providerInternal =>
      request.directAgent = false ∧
        HasMatchingCapability session request capability
  | .supportTool =>
      request.supportPolicyAuthorized = true ∨
        HasMatchingCapability session request capability
  | .hostTool => request.hostPolicyAuthorized = true
  | .testFixture =>
      request.testRuntime = true ∧
        HasMatchingCapability session request capability

def Admitted
    (registration : Option BinaryRegistration)
    (session : SessionClock)
    (request : DispatchRequest)
    (capability : Option DispatchCapability) : Prop :=
  ∃ registered,
    registration = some registered ∧
      RegistrationMatches registered request ∧
      ProfileAdmission session request capability

theorem provider_internal_admission_requires_runtime_capability
    (registration : BinaryRegistration)
    (session : SessionClock)
    (request : DispatchRequest)
    (capability : DispatchCapability)
    (hProfile : request.profile = .providerInternal)
    (hAdmitted : Admitted (some registration) session request (some capability)) :
    request.directAgent = false ∧ CapabilityMatches session request capability := by
  rcases hAdmitted with ⟨registered, hRegistration, _, hAdmission⟩
  cases hRegistration
  rw [ProfileAdmission, hProfile] at hAdmission
  rcases hAdmission with ⟨hDirect, admittedCapability, hSome, hMatches⟩
  cases hSome
  exact ⟨hDirect, hMatches⟩

theorem matching_live_provider_capability_is_admitted
    (registration : BinaryRegistration)
    (session : SessionClock)
    (request : DispatchRequest)
    (capability : DispatchCapability)
    (hRegistration : RegistrationMatches registration request)
    (hProfile : request.profile = .providerInternal)
    (hRuntimeDispatch : request.directAgent = false)
    (hCapability : CapabilityMatches session request capability) :
    Admitted (some registration) session request (some capability) := by
  refine ⟨registration, rfl, hRegistration, ?_⟩
  rw [ProfileAdmission, hProfile]
  exact ⟨hRuntimeDispatch, capability, rfl, hCapability⟩

theorem direct_provider_internal_is_denied
    (registration : BinaryRegistration)
    (session : SessionClock)
    (request : DispatchRequest)
    (capability : Option DispatchCapability)
    (hProfile : request.profile = .providerInternal)
    (hDirect : request.directAgent = true) :
    ¬ Admitted (some registration) session request capability := by
  intro hAdmitted
  rcases hAdmitted with ⟨registered, hRegistration, _, hAdmission⟩
  cases hRegistration
  rw [ProfileAdmission, hProfile, hDirect] at hAdmission
  exact Bool.noConfusion hAdmission.1

theorem invalid_provider_capability_is_denied
    (registration : BinaryRegistration)
    (session : SessionClock)
    (request : DispatchRequest)
    (capability : DispatchCapability)
    (hProfile : request.profile = .providerInternal)
    (hInvalid : ¬ CapabilityMatches session request capability) :
    ¬ Admitted (some registration) session request (some capability) := by
  intro hAdmitted
  exact hInvalid
    (provider_internal_admission_requires_runtime_capability
      registration session request capability hProfile hAdmitted).2

theorem wrong_root_session_is_denied
    (registration : BinaryRegistration) (session : SessionClock)
    (request : DispatchRequest) (capability : DispatchCapability)
    (hProfile : request.profile = .providerInternal)
    (hWrong : capability.rootSession ≠ request.rootSession) :
    ¬ Admitted (some registration) session request (some capability) := by
  apply invalid_provider_capability_is_denied
    registration session request capability hProfile
  intro h
  exact hWrong h.1.2.2.1

theorem wrong_active_root_session_is_denied
    (registration : BinaryRegistration) (session : SessionClock)
    (request : DispatchRequest) (capability : DispatchCapability)
    (hProfile : request.profile = .providerInternal)
    (hWrong : session.rootSession ≠ request.rootSession) :
    ¬ Admitted (some registration) session request (some capability) := by
  apply invalid_provider_capability_is_denied
    registration session request capability hProfile
  intro h
  exact hWrong h.1.1

theorem wrong_child_session_is_denied
    (registration : BinaryRegistration) (session : SessionClock)
    (request : DispatchRequest) (capability : DispatchCapability)
    (hProfile : request.profile = .providerInternal)
    (hWrong : capability.childSession ≠ request.childSession) :
    ¬ Admitted (some registration) session request (some capability) := by
  apply invalid_provider_capability_is_denied
    registration session request capability hProfile
  intro h
  exact hWrong h.1.2.2.2

theorem wrong_active_child_session_is_denied
    (registration : BinaryRegistration) (session : SessionClock)
    (request : DispatchRequest) (capability : DispatchCapability)
    (hProfile : request.profile = .providerInternal)
    (hWrong : session.childSession ≠ request.childSession) :
    ¬ Admitted (some registration) session request (some capability) := by
  apply invalid_provider_capability_is_denied
    registration session request capability hProfile
  intro h
  exact hWrong h.1.2.1

theorem wrong_binary_is_denied
    (registration : BinaryRegistration) (session : SessionClock)
    (request : DispatchRequest) (capability : DispatchCapability)
    (hProfile : request.profile = .providerInternal)
    (hWrong : capability.binary ≠ request.binary) :
    ¬ Admitted (some registration) session request (some capability) := by
  apply invalid_provider_capability_is_denied
    registration session request capability hProfile
  intro h
  exact hWrong h.2.1.1

theorem wrong_profile_is_denied
    (registration : BinaryRegistration) (session : SessionClock)
    (request : DispatchRequest) (capability : DispatchCapability)
    (hProfile : request.profile = .providerInternal)
    (hWrong : capability.profile ≠ request.profile) :
    ¬ Admitted (some registration) session request (some capability) := by
  apply invalid_provider_capability_is_denied
    registration session request capability hProfile
  intro h
  exact hWrong h.2.1.2.1

theorem wrong_provider_is_denied
    (registration : BinaryRegistration) (session : SessionClock)
    (request : DispatchRequest) (capability : DispatchCapability)
    (hProfile : request.profile = .providerInternal)
    (hWrong : capability.provider ≠ request.provider) :
    ¬ Admitted (some registration) session request (some capability) := by
  apply invalid_provider_capability_is_denied
    registration session request capability hProfile
  intro h
  exact hWrong h.2.1.2.2.1

theorem wrong_language_is_denied
    (registration : BinaryRegistration) (session : SessionClock)
    (request : DispatchRequest) (capability : DispatchCapability)
    (hProfile : request.profile = .providerInternal)
    (hWrong : capability.language ≠ request.language) :
    ¬ Admitted (some registration) session request (some capability) := by
  apply invalid_provider_capability_is_denied
    registration session request capability hProfile
  intro h
  exact hWrong h.2.1.2.2.2.1

theorem wrong_generation_is_denied
    (registration : BinaryRegistration) (session : SessionClock)
    (request : DispatchRequest) (capability : DispatchCapability)
    (hProfile : request.profile = .providerInternal)
    (hWrong : capability.generation ≠ request.generation) :
    ¬ Admitted (some registration) session request (some capability) := by
  apply invalid_provider_capability_is_denied
    registration session request capability hProfile
  intro h
  exact hWrong h.2.1.2.2.2.2.1

theorem wrong_artifact_digest_is_denied
    (registration : BinaryRegistration) (session : SessionClock)
    (request : DispatchRequest) (capability : DispatchCapability)
    (hProfile : request.profile = .providerInternal)
    (hWrong : capability.artifactDigest ≠ request.artifactDigest) :
    ¬ Admitted (some registration) session request (some capability) := by
  apply invalid_provider_capability_is_denied
    registration session request capability hProfile
  intro h
  exact hWrong h.2.1.2.2.2.2.2.1

theorem wrong_argv_digest_is_denied
    (registration : BinaryRegistration) (session : SessionClock)
    (request : DispatchRequest) (capability : DispatchCapability)
    (hProfile : request.profile = .providerInternal)
    (hWrong : capability.argvDigest ≠ request.argvDigest) :
    ¬ Admitted (some registration) session request (some capability) := by
  apply invalid_provider_capability_is_denied
    registration session request capability hProfile
  intro h
  exact hWrong h.2.1.2.2.2.2.2.2.1

theorem wrong_attempt_is_replay_denied
    (registration : BinaryRegistration) (session : SessionClock)
    (request : DispatchRequest) (capability : DispatchCapability)
    (hProfile : request.profile = .providerInternal)
    (hWrong : capability.attempt ≠ request.attempt) :
    ¬ Admitted (some registration) session request (some capability) := by
  apply invalid_provider_capability_is_denied
    registration session request capability hProfile
  intro h
  exact hWrong h.2.1.2.2.2.2.2.2.2

theorem not_yet_issued_capability_is_denied
    (registration : BinaryRegistration) (session : SessionClock)
    (request : DispatchRequest) (capability : DispatchCapability)
    (hProfile : request.profile = .providerInternal)
    (hFuture : session.now < capability.issuedAt) :
    ¬ Admitted (some registration) session request (some capability) := by
  apply invalid_provider_capability_is_denied
    registration session request capability hProfile
  intro h
  exact (Nat.not_lt_of_ge h.2.2.1) hFuture

theorem expired_capability_is_denied
    (registration : BinaryRegistration) (session : SessionClock)
    (request : DispatchRequest) (capability : DispatchCapability)
    (hProfile : request.profile = .providerInternal)
    (hExpired : capability.expiresAt ≤ session.now) :
    ¬ Admitted (some registration) session request (some capability) := by
  apply invalid_provider_capability_is_denied
    registration session request capability hProfile
  intro h
  exact (Nat.not_lt_of_ge hExpired) h.2.2.2.1

theorem consumed_capability_is_replay_denied
    (registration : BinaryRegistration) (session : SessionClock)
    (request : DispatchRequest) (capability : DispatchCapability)
    (hProfile : request.profile = .providerInternal)
    (hConsumed : capability.consumed = true) :
    ¬ Admitted (some registration) session request (some capability) := by
  apply invalid_provider_capability_is_denied
    registration session request capability hProfile
  intro h
  have hUnconsumed : capability.consumed = false := h.2.2.2.2
  rw [hConsumed] at hUnconsumed
  exact Bool.noConfusion hUnconsumed

theorem test_fixture_outside_test_runtime_is_denied
    (registration : BinaryRegistration) (session : SessionClock)
    (request : DispatchRequest) (capability : Option DispatchCapability)
    (hProfile : request.profile = .testFixture)
    (hProduction : request.testRuntime = false) :
    ¬ Admitted (some registration) session request capability := by
  intro hAdmitted
  rcases hAdmitted with ⟨registered, hRegistration, _, hAdmission⟩
  cases hRegistration
  rw [ProfileAdmission, hProfile, hProduction] at hAdmission
  exact Bool.noConfusion hAdmission.1

theorem facade_is_not_misclassified_as_provider_internal
    (registration : BinaryRegistration) (session : SessionClock)
    (request : DispatchRequest)
    (hRegistration : RegistrationMatches registration request)
    (hProfile : request.profile = .facade)
    (hPolicy : request.facadePolicyAuthorized = true) :
    Admitted (some registration) session request none := by
  refine ⟨registration, rfl, hRegistration, ?_⟩
  rw [ProfileAdmission, hProfile]
  exact hPolicy

theorem unregistered_runtime_binary_fails_closed
    (session : SessionClock) (request : DispatchRequest)
    (capability : Option DispatchCapability) :
    ¬ Admitted none session request capability := by
  intro hAdmitted
  rcases hAdmitted with ⟨registered, hRegistration, _, _⟩
  cases hRegistration

theorem profile_admission_is_not_a_binary_name_blacklist
    (first second : DispatchRequest)
    (hSameProfile : first.profile = second.profile)
    (hSameFacadePolicy : first.facadePolicyAuthorized = second.facadePolicyAuthorized)
    (hFacade : first.profile = .facade) :
    ProfileAdmission { rootSession := "r", childSession := "c", now := 0 }
      first none ↔
    ProfileAdmission { rootSession := "r", childSession := "c", now := 0 }
      second none := by
  have hSecondFacade : second.profile = .facade := hSameProfile ▸ hFacade
  unfold ProfileAdmission
  rw [hFacade, hSecondFacade, hSameFacadePolicy]

end ASPProof.RuntimeBinaryProfileDispatch
