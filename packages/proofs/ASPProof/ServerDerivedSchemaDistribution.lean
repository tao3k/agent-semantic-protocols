-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.ServerDerivedSchemaDistribution

inductive SchemaClass
  | languageSemantic
  | clientBootstrap
  | sharedClosure
  deriving DecidableEq

def packageLocalAdmitted : SchemaClass → Bool
  | .languageSemantic => true
  | .clientBootstrap => true
  | .sharedClosure => false

def serverProjectionAdmitted (schemaClass : SchemaClass) (canonicalDigestMatches : Bool) : Bool :=
  schemaClass == .sharedClosure && canonicalDigestMatches

def legacyReplicaCount (languages shared providerOwned bootstrap : Nat) : Nat :=
  languages * shared + providerOwned + languages * bootstrap

def targetReplicaCount (_languages _shared providerOwned bootstrap : Nat) : Nat :=
  providerOwned + bootstrap

theorem shared_closure_is_not_package_local :
    packageLocalAdmitted .sharedClosure = false := rfl

theorem provider_semantics_remain_package_local :
    packageLocalAdmitted .languageSemantic = true := rfl

theorem server_cannot_invent_shared_schema :
    serverProjectionAdmitted .sharedClosure false = false := rfl

theorem exact_canonical_shared_schema_is_projectable :
    serverProjectionAdmitted .sharedClosure true = true := rfl

theorem provider_schema_cannot_be_reclassified_as_server_projection
    (digestMatches : Bool) :
    serverProjectionAdmitted .languageSemantic digestMatches = false := by
  cases digestMatches <;> rfl

theorem target_removes_language_multiplier
    (languages shared providerOwned bootstrap : Nat) :
    targetReplicaCount languages shared providerOwned bootstrap = providerOwned + bootstrap := rfl

theorem legacy_replication_never_smaller_when_language_exists
    (languages shared providerOwned bootstrap : Nat)
    (hasLanguage : 1 ≤ languages) :
    targetReplicaCount languages shared providerOwned bootstrap ≤
      legacyReplicaCount languages shared providerOwned bootstrap := by
  have bootstrapBound : bootstrap ≤ languages * bootstrap := by
    simpa using Nat.mul_le_mul_right bootstrap hasLanguage
  calc
    targetReplicaCount languages shared providerOwned bootstrap = providerOwned + bootstrap := rfl
    _ ≤ providerOwned + languages * bootstrap := Nat.add_le_add_left bootstrapBound providerOwned
    _ ≤ languages * shared + providerOwned + languages * bootstrap := by omega

end ASPProof.ServerDerivedSchemaDistribution
