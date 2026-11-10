-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchLoopCacheIdentity

namespace SearchLoopCacheDiagnostics

open SearchLoopCacheIdentity

inductive InvalidationReason where
  | obligation
  | workspace
  | witnessSet
  | sourceSnapshot
  | provider
  | schema
  | policy
  | providerArtifact
  | selector
  | rfc
  | projection
  | budgetClass
  deriving DecidableEq, Repr

def semanticMismatch
    (cached current : SemanticKey) :
    InvalidationReason :=
  if cached.obligation ≠ current.obligation then
    InvalidationReason.obligation
  else if cached.workspace ≠ current.workspace then
    InvalidationReason.workspace
  else if cached.witnessSet ≠ current.witnessSet then
    InvalidationReason.witnessSet
  else if cached.domain.sourceSnapshot ≠ current.domain.sourceSnapshot then
    InvalidationReason.sourceSnapshot
  else if cached.domain.provider ≠ current.domain.provider then
    InvalidationReason.provider
  else if cached.domain.schema ≠ current.domain.schema then
    InvalidationReason.schema
  else if cached.domain.policy ≠ current.domain.policy then
    InvalidationReason.policy
  else if cached.providerArtifact ≠ current.providerArtifact then
    InvalidationReason.providerArtifact
  else if cached.selector ≠ current.selector then
    InvalidationReason.selector
  else if cached.rfc ≠ current.rfc then
    InvalidationReason.rfc
  else if cached.projection ≠ current.projection then
    InvalidationReason.projection
  else
    InvalidationReason.budgetClass

def diagnoseSemantic
    (cached current : SemanticKey) :
    Option InvalidationReason :=
  if cached = current then
    none
  else
    some (semanticMismatch cached current)

theorem no_diagnostic_iff_exact_key
    (cached current : SemanticKey) :
    diagnoseSemantic cached current = none ↔ cached = current := by
  simp [diagnoseSemantic]

theorem diagnostic_implies_invalidate
    (cached current : SemanticKey)
    (reason : InvalidationReason)
    (diagnostic : diagnoseSemantic cached current = some reason) :
    decideSemanticReuse cached current = CacheDecision.invalidate := by
  apply semantic_key_drift_invalidates
  intro equal
  subst current
  simp [diagnoseSemantic] at diagnostic

theorem provider_artifact_drift_has_typed_reason :
    diagnoseSemantic baseSemanticKey providerArtifactDriftKey =
      some InvalidationReason.providerArtifact := by
  rfl

theorem policy_drift_has_typed_reason :
    diagnoseSemantic baseSemanticKey policyDriftKey =
      some InvalidationReason.policy := by
  rfl

end SearchLoopCacheDiagnostics
