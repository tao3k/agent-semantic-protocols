-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.SearchRouteRegistrySnapshotResolution

inductive PolicySemantics where
  | sharedOnly
  | alphaAll
  deriving DecidableEq

inductive RegistrySnapshot where
  | v1
  | v2
  | revoked
  deriving DecidableEq

def snapshotDigest : RegistrySnapshot → Nat
  | .v1 => 101
  | .v2 => 102
  | .revoked => 103

theorem snapshot_digest_is_injective :
    Function.Injective snapshotDigest := by
  intro left right equalDigest
  cases left <;> cases right <;>
    simp [snapshotDigest] at equalDigest <;>
    rfl

def resolvePolicy
    (snapshot : RegistrySnapshot)
    (policyDigest : Nat) : Option PolicySemantics :=
  match snapshot, policyDigest with
  | .v1, 7 => some .sharedOnly
  | .v2, 7 => some .alphaAll
  | _, _ => none

structure ResolutionCertificate where
  snapshot : RegistrySnapshot
  claimedSnapshotDigest : Nat
  policyDigest : Nat
  semantics : PolicySemantics
  snapshotMatches :
    claimedSnapshotDigest = snapshotDigest snapshot
  resolves :
    resolvePolicy snapshot policyDigest = some semantics

def ReplayComparable
    (left right : ResolutionCertificate) : Prop :=
  left.claimedSnapshotDigest = right.claimedSnapshotDigest ∧
  left.policyDigest = right.policyDigest

theorem resolution_is_unique_within_snapshot
    {snapshot : RegistrySnapshot}
    {policyDigest : Nat}
    {left right : PolicySemantics}
    (leftResolution :
      resolvePolicy snapshot policyDigest = some left)
    (rightResolution :
      resolvePolicy snapshot policyDigest = some right) :
    left = right := by
  rw [leftResolution] at rightResolution
  exact Option.some.inj rightResolution

theorem comparable_certificates_resolve_same_semantics
    {left right : ResolutionCertificate}
    (comparable : ReplayComparable left right) :
    left.semantics = right.semantics := by
  have sameSnapshot : left.snapshot = right.snapshot :=
    snapshot_digest_is_injective <| by
      calc
        snapshotDigest left.snapshot =
            left.claimedSnapshotDigest :=
              left.snapshotMatches.symm
        _ = right.claimedSnapshotDigest := comparable.1
        _ = snapshotDigest right.snapshot := right.snapshotMatches
  apply resolution_is_unique_within_snapshot left.resolves
  simpa [sameSnapshot, comparable.2] using right.resolves

def v1Certificate : ResolutionCertificate where
  snapshot := .v1
  claimedSnapshotDigest := 101
  policyDigest := 7
  semantics := .sharedOnly
  snapshotMatches := rfl
  resolves := rfl

def v2Certificate : ResolutionCertificate where
  snapshot := .v2
  claimedSnapshotDigest := 102
  policyDigest := 7
  semantics := .alphaAll
  snapshotMatches := rfl
  resolves := rfl

theorem policy_digest_alone_is_insufficient :
    v1Certificate.policyDigest = v2Certificate.policyDigest ∧
    v1Certificate.semantics ≠ v2Certificate.semantics :=
  ⟨rfl, by decide⟩

theorem snapshot_upgrade_rejects_stale_replay :
    ¬ ReplayComparable v1Certificate v2Certificate := by
  intro comparable
  exact (by decide : (101 : Nat) ≠ 102) comparable.1

theorem revoked_snapshot_resolves_no_policy :
    resolvePolicy .revoked 7 = none :=
  rfl

theorem revoked_policy_has_no_resolution_certificate :
    ¬ ∃ semantics,
      resolvePolicy .revoked 7 = some semantics := by
  intro witness
  rcases witness with ⟨semantics, resolution⟩
  cases semantics <;> simp [resolvePolicy] at resolution

theorem certificate_is_replay_comparable_with_itself :
    ReplayComparable v1Certificate v1Certificate :=
  ⟨rfl, rfl⟩

end ASPProof.SearchRouteRegistrySnapshotResolution
