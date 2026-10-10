-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.ServerDerivedSchemaDistribution

inductive SchemaClass
  | languageSemantic
  | clientBootstrap
  | sharedClosure
  deriving DecidableEq

inductive CacheDigestState
  | absent
  | current
  | stale
  | malformed
  deriving DecidableEq

inductive SocketProjectionState
  | ready
  | unchanged
  | failed
  deriving DecidableEq

def packageLocalAdmitted : SchemaClass → Bool
  | .languageSemantic => true
  | .clientBootstrap => true
  | .sharedClosure => false

def serverProjectionAdmitted (schemaClass : SchemaClass) (canonicalDigestMatches : Bool) : Bool :=
  schemaClass == .sharedClosure && canonicalDigestMatches

def socketProjectionState
    (languageRegistered rootSetMatches : Bool)
    (digestState : CacheDigestState) : SocketProjectionState :=
  if !languageRegistered || !rootSetMatches then
    .failed
  else
    match digestState with
    | .absent => .ready
    | .current => .unchanged
    | .stale => .ready
    | .malformed => .failed

def catalogProjectionSteps (_schemaCount : Nat) : Nat := 1

def wireDocumentCount (state : SocketProjectionState) (schemaCount : Nat) : Nat :=
  match state with
  | .ready => schemaCount
  | .unchanged | .failed => 0

def bundleReceiptAdmitted (expected actual : Nat) : Bool := decide (expected = actual)

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

theorem registered_absent_digest_is_ready :
    socketProjectionState true true .absent = .ready := rfl

theorem registered_current_digest_is_unchanged :
    socketProjectionState true true .current = .unchanged := rfl

theorem registered_stale_digest_is_ready :
    socketProjectionState true true .stale = .ready := rfl

theorem unknown_language_fails_closed
    (rootSetMatches : Bool)
    (digestState : CacheDigestState) :
    socketProjectionState false rootSetMatches digestState = .failed := by
  cases rootSetMatches <;> cases digestState <;> rfl

theorem root_set_drift_fails_closed
    (digestState : CacheDigestState) :
    socketProjectionState true false digestState = .failed := by
  cases digestState <;> rfl

theorem malformed_digest_fails_closed :
    socketProjectionState true true .malformed = .failed := rfl

theorem immutable_catalog_projection_is_schema_count_independent
    (leftSchemaCount rightSchemaCount : Nat) :
    catalogProjectionSteps leftSchemaCount = catalogProjectionSteps rightSchemaCount := rfl

theorem unchanged_transfers_no_documents (schemaCount : Nat) :
    wireDocumentCount .unchanged schemaCount = 0 := rfl

theorem ready_transfer_is_linear_in_document_count (schemaCount : Nat) :
    wireDocumentCount .ready schemaCount = schemaCount := rfl

theorem corrupted_bundle_receipt_is_rejected
    (expected actual : Nat)
    (digestMismatch : expected ≠ actual) :
    bundleReceiptAdmitted expected actual = false := by
  simp [bundleReceiptAdmitted, digestMismatch]

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
