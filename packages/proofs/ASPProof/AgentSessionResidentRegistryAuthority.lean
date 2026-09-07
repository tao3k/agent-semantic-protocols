-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.AgentSessionResidentRegistryAuthority

inductive AgentProfileId where
  | aspExplorer
  | aspTesting
  deriving DecidableEq, Repr

def profileKey : AgentProfileId → String
  | .aspExplorer => "asp_explorer"
  | .aspTesting => "asp_testing"

def profileArtifact (profile : AgentProfileId) : String :=
  profileKey profile ++ ".toml"

theorem profileArtifact_is_derived_from_profile_identity (profile : AgentProfileId) :
    profileArtifact profile = profileKey profile ++ ".toml" := by
  rfl

structure RegistryAuthority where
  daemonHandles : Nat
  clientHandles : Nat
  deriving DecidableEq, Repr

def residentAuthority : RegistryAuthority :=
  { daemonHandles := 1, clientHandles := 0 }

def authorityIsValid (authority : RegistryAuthority) : Prop :=
  authority.daemonHandles = 1 ∧ authority.clientHandles = 0

theorem resident_authority_has_one_daemon_handle :
    authorityIsValid residentAuthority := by
  exact ⟨rfl, rfl⟩

inductive RegistryRequestOrigin where
  | runtimeServer
  | clientProxy
  deriving DecidableEq, Repr

def opensDatabase : RegistryRequestOrigin → Bool
  | .runtimeServer => false
  | .clientProxy => false

theorem client_proxy_never_opens_registry_database :
    opensDatabase .clientProxy = false := by
  rfl

theorem runtime_request_reuses_resident_database :
    opensDatabase .runtimeServer = false := by
  rfl

inductive DeveloperInstallPhase where
  | capture
  | build
  | validate
  | publish
  | reconcile
  deriving DecidableEq, Repr

def holdsPublicationLock : DeveloperInstallPhase → Bool
  | .capture => false
  | .build => false
  | .validate => false
  | .publish => true
  | .reconcile => false

theorem developer_build_never_holds_publication_lock :
    holdsPublicationLock .build = false := by
  rfl

theorem daemon_reconcile_never_holds_publication_lock :
    holdsPublicationLock .reconcile = false := by
  rfl

inductive CandidateState where
  | captured
  | built
  | validated
  | published
  deriving DecidableEq, Repr

def mayReconcileDaemon : CandidateState → Bool
  | .published => true
  | _ => false

theorem daemon_reconcile_requires_atomic_publication (candidate : CandidateState) :
    mayReconcileDaemon candidate = true → candidate = .published := by
  cases candidate <;> simp [mayReconcileDaemon]

end ASPProof.AgentSessionResidentRegistryAuthority
