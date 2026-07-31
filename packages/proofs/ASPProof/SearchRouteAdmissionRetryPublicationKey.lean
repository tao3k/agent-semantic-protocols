import ASPProof.SearchRouteAdmissionRetryPublicationLedger

namespace ASPProof.SearchRouteAdmissionRetryPublicationKey

/--
Every field is part of the authoritative idempotency domain. The concrete Nat
representation keeps the proof focused on identity separation; wire encodings
may use content-addressed identities instead.
-/
structure RetryPublicationKey where
  workspaceIdentity : Nat
  ledgerGeneration : Nat
  contractVersion : Nat
  tenantScope : Nat
  retryIdentity : Nat
deriving DecidableEq, Repr

theorem full_key_equality_preserves_all_scope_dimensions
    {left right : RetryPublicationKey}
    (equal : left = right) :
    left.workspaceIdentity = right.workspaceIdentity
      ∧ left.ledgerGeneration = right.ledgerGeneration
      ∧ left.contractVersion = right.contractVersion
      ∧ left.tenantScope = right.tenantScope
      ∧ left.retryIdentity = right.retryIdentity := by
  cases equal
  simp

def omitWorkspace
    (key : RetryPublicationKey) :
    Nat × Nat × Nat × Nat :=
  ( key.ledgerGeneration
  , key.contractVersion
  , key.tenantScope
  , key.retryIdentity )

def omitGeneration
    (key : RetryPublicationKey) :
    Nat × Nat × Nat × Nat :=
  ( key.workspaceIdentity
  , key.contractVersion
  , key.tenantScope
  , key.retryIdentity )

def omitContractVersion
    (key : RetryPublicationKey) :
    Nat × Nat × Nat × Nat :=
  ( key.workspaceIdentity
  , key.ledgerGeneration
  , key.tenantScope
  , key.retryIdentity )

def omitTenantScope
    (key : RetryPublicationKey) :
    Nat × Nat × Nat × Nat :=
  ( key.workspaceIdentity
  , key.ledgerGeneration
  , key.contractVersion
  , key.retryIdentity )

def omitRetryIdentity
    (key : RetryPublicationKey) :
    Nat × Nat × Nat × Nat :=
  ( key.workspaceIdentity
  , key.ledgerGeneration
  , key.contractVersion
  , key.tenantScope )

def baseKey : RetryPublicationKey :=
  { workspaceIdentity := 0
    ledgerGeneration := 0
    contractVersion := 0
    tenantScope := 0
    retryIdentity := 0 }

def otherWorkspaceKey : RetryPublicationKey :=
  { baseKey with workspaceIdentity := 1 }

def otherGenerationKey : RetryPublicationKey :=
  { baseKey with ledgerGeneration := 1 }

def otherContractVersionKey : RetryPublicationKey :=
  { baseKey with contractVersion := 1 }

def otherTenantScopeKey : RetryPublicationKey :=
  { baseKey with tenantScope := 1 }

def otherRetryIdentityKey : RetryPublicationKey :=
  { baseKey with retryIdentity := 1 }

theorem omitting_workspace_aliases_distinct_publication_domains :
    baseKey ≠ otherWorkspaceKey
      ∧ omitWorkspace baseKey = omitWorkspace otherWorkspaceKey := by
  decide

theorem omitting_generation_aliases_distinct_publication_domains :
    baseKey ≠ otherGenerationKey
      ∧ omitGeneration baseKey = omitGeneration otherGenerationKey := by
  decide

theorem omitting_contract_version_aliases_distinct_publication_domains :
    baseKey ≠ otherContractVersionKey
      ∧ omitContractVersion baseKey =
        omitContractVersion otherContractVersionKey := by
  decide

theorem omitting_tenant_scope_aliases_distinct_publication_domains :
    baseKey ≠ otherTenantScopeKey
      ∧ omitTenantScope baseKey = omitTenantScope otherTenantScopeKey := by
  decide

theorem omitting_retry_identity_aliases_distinct_publication_domains :
    baseKey ≠ otherRetryIdentityKey
      ∧ omitRetryIdentity baseKey =
        omitRetryIdentity otherRetryIdentityKey := by
  decide

end ASPProof.SearchRouteAdmissionRetryPublicationKey
