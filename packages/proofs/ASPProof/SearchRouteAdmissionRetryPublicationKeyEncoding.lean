-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteAdmissionRetryPublicationKey

namespace ASPProof.SearchRouteAdmissionRetryPublicationKeyEncoding

open ASPProof.SearchRouteAdmissionRetryPublicationKey

/--
Encoding-schema identity is distinct from the retry contract version. It names
the canonical field framing and decoding rules.
-/
structure VersionedRetryPublicationKey where
  encodingSchemaIdentity : Nat
  semanticKey : RetryPublicationKey
deriving DecidableEq, Repr

/--
This typed record is the mathematical form of a tagged, length-delimited wire
encoding. Each semantic dimension has one fixed decoding slot.
-/
structure CanonicalKeyEncoding where
  encodingSchemaIdentity : Nat
  workspaceIdentity : Nat
  ledgerGeneration : Nat
  contractVersion : Nat
  tenantScope : Nat
  retryIdentity : Nat
deriving DecidableEq, Repr

def canonicalEncode
    (key : VersionedRetryPublicationKey) :
    CanonicalKeyEncoding :=
  { encodingSchemaIdentity := key.encodingSchemaIdentity
    workspaceIdentity := key.semanticKey.workspaceIdentity
    ledgerGeneration := key.semanticKey.ledgerGeneration
    contractVersion := key.semanticKey.contractVersion
    tenantScope := key.semanticKey.tenantScope
    retryIdentity := key.semanticKey.retryIdentity }

def canonicalDecode
    (encoding : CanonicalKeyEncoding) :
    VersionedRetryPublicationKey :=
  { encodingSchemaIdentity := encoding.encodingSchemaIdentity
    semanticKey :=
      { workspaceIdentity := encoding.workspaceIdentity
        ledgerGeneration := encoding.ledgerGeneration
        contractVersion := encoding.contractVersion
        tenantScope := encoding.tenantScope
        retryIdentity := encoding.retryIdentity } }

theorem canonical_decode_encode_round_trip
    (key : VersionedRetryPublicationKey) :
    canonicalDecode (canonicalEncode key) = key := by
  cases key
  rfl

theorem canonical_encoding_is_injective
    {left right : VersionedRetryPublicationKey}
    (equalEncoding : canonicalEncode left = canonicalEncode right) :
    left = right := by
  have decodedEquality :=
    congrArg canonicalDecode equalEncoding
  simpa [canonical_decode_encode_round_trip] using decodedEquality

structure BitPair where
  left : List Bool
  right : List Bool
deriving DecidableEq, Repr

def delimiterFreeEncode (pair : BitPair) : List Bool :=
  pair.left ++ pair.right

def delimiterPairA : BitPair :=
  { left := [true]
    right := [false, true] }

def delimiterPairB : BitPair :=
  { left := [true, false]
    right := [true] }

theorem delimiter_free_concatenation_is_not_injective :
    delimiterPairA ≠ delimiterPairB
      ∧ delimiterFreeEncode delimiterPairA =
        delimiterFreeEncode delimiterPairB := by
  decide

def unorderedTwoFieldEncode (left right : Nat) : Nat :=
  left + right

theorem unordered_field_encoding_is_not_injective :
    (1, 2) ≠ (2, 1)
      ∧ unorderedTwoFieldEncode 1 2 =
        unorderedTwoFieldEncode 2 1 := by
  decide

def schemaOneKey : VersionedRetryPublicationKey :=
  { encodingSchemaIdentity := 1
    semanticKey := baseKey }

def schemaTwoKey : VersionedRetryPublicationKey :=
  { encodingSchemaIdentity := 2
    semanticKey := baseKey }

def omitEncodingSchema
    (key : VersionedRetryPublicationKey) :
    RetryPublicationKey :=
  key.semanticKey

theorem omitting_encoding_schema_aliases_distinct_decoding_domains :
    schemaOneKey ≠ schemaTwoKey
      ∧ omitEncodingSchema schemaOneKey =
        omitEncodingSchema schemaTwoKey := by
  decide

def lossyNormalize (identity : Nat) : Nat :=
  identity % 10

theorem lossy_normalization_is_not_identity_preserving :
    (1 : Nat) ≠ 11
      ∧ lossyNormalize 1 = lossyNormalize 11 := by
  decide

end ASPProof.SearchRouteAdmissionRetryPublicationKeyEncoding
