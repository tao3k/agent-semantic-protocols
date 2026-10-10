-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.RuntimeObservationAuthority

inductive Component where
  | observationCore
  | processObservation
  | clientDb
  | runtimeServer
deriving DecidableEq, Repr

inductive Authority where
  | observationModelV1
  | activeSinkRegistry
  | processProbe
  | unsafeOsBoundary
  | sharedDatabasePool
  | samplerLifecycle
  | tursoExporter
deriving DecidableEq, Repr

def Owns : Component → Authority → Prop
  | .observationCore, .observationModelV1 => True
  | .observationCore, .activeSinkRegistry => True
  | .processObservation, .processProbe => True
  | .processObservation, .unsafeOsBoundary => True
  | .clientDb, .sharedDatabasePool => True
  | .runtimeServer, .samplerLifecycle => True
  | .runtimeServer, .tursoExporter => True
  | _, _ => False

def unregister (active : Option Nat) (registration : Nat) : Option Nat :=
  match active with
  | none => none
  | some current => if current = registration then none else some current

theorem client_db_does_not_own_exporter :
    ¬ Owns .clientDb .tursoExporter := by
  simp [Owns]

theorem producer_model_does_not_own_exporter :
    ¬ Owns .observationCore .tursoExporter := by
  simp [Owns]

theorem runtime_server_does_not_own_unsafe_os_boundary :
    ¬ Owns .runtimeServer .unsafeOsBoundary := by
  simp [Owns]

theorem process_observation_does_not_own_sampler_lifecycle :
    ¬ Owns .processObservation .samplerLifecycle := by
  simp [Owns]

theorem runtime_server_does_not_own_shared_database_pool :
    ¬ Owns .runtimeServer .sharedDatabasePool := by
  simp [Owns]

theorem stale_registration_cannot_revoke_current_sink
    (stale current : Nat) (different : current ≠ stale) :
    unregister (some current) stale = some current := by
  simp [unregister, different]

theorem current_registration_revokes_exact_sink (current : Nat) :
    unregister (some current) current = none := by
  simp [unregister]

end ASPProof.RuntimeObservationAuthority
