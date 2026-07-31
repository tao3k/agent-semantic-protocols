import ASPProof.SearchRouteCryptographicTransferReceiptBinding
import Lean

open Lean

namespace ASPProof.Audit.SearchRouteCryptographicTransferReceiptBinding

def theoremDeclaration
    (name theoremFamily type rfcClauseId : String)
    (axioms : List String := []) : Json :=
  Json.mkObj
    [ ("name", toJson name)
    , ("kind", toJson "theorem")
    , ("theoremFamily", toJson theoremFamily)
    , ("type", toJson type)
    , ("rfcClauseIds", toJson [rfcClauseId])
    , ("axioms", toJson axioms)
    , ("hasSorryAx", toJson false)
    ]

def declarations : Array Json :=
  #[ theoremDeclaration
       "valid_receipt_binds_payload"
       "payload-binding"
       "A valid receipt binds its canonical payload and payload digest"
       "ASP-RFC-10.05-CTRB-CANONICAL-PAYLOAD"
   , theoremDeclaration
       "valid_receipt_binds_replay_domain"
       "replay-domain-binding"
       "Payload and both signatures bind the active replay domain"
       "ASP-RFC-10.05-CTRB-REPLAY-DOMAIN"
   , theoremDeclaration
       "valid_receipt_is_temporally_scoped"
       "temporal-scope"
       "A valid receipt satisfies its issuance and expiry interval"
       "ASP-RFC-10.05-CTRB-TEMPORAL-SCOPE"
   , theoremDeclaration
       "expired_payload_rejects_receipt"
       "temporal-scope"
       "An expired transfer payload rejects receipt admission"
       "ASP-RFC-10.05-CTRB-TEMPORAL-SCOPE"
   , theoremDeclaration
       "revoked_old_key_rejects_receipt"
       "key-revocation"
       "Revocation of the old signer key rejects receipt admission"
       "ASP-RFC-10.05-CTRB-KEY-EPOCH"
   , theoremDeclaration
       "revoked_new_key_rejects_receipt"
       "key-revocation"
       "Revocation of the new signer key rejects receipt admission"
       "ASP-RFC-10.05-CTRB-KEY-EPOCH"
   , theoremDeclaration
       "changed_replay_domain_rejects_receipt"
       "replay-domain-binding"
       "A changed replay domain rejects an otherwise verified receipt"
       "ASP-RFC-10.05-CTRB-REPLAY-DOMAIN"
   , theoremDeclaration
       "old_algorithm_substitution_rejects_receipt"
       "algorithm-binding"
       "Substitution of the old signature algorithm rejects the receipt"
       "ASP-RFC-10.05-CTRB-ALGORITHM"
   , theoremDeclaration
       "new_key_epoch_substitution_rejects_receipt"
       "key-epoch-binding"
       "Substitution of the new signer key epoch rejects the receipt"
       "ASP-RFC-10.05-CTRB-KEY-EPOCH"
   , theoremDeclaration
       "injective_payload_digest_makes_bound_payload_unique"
       "digest-uniqueness"
       "Injective canonical digest binding makes the signed payload unique"
       "ASP-RFC-10.05-CTRB-DIGEST-BOUNDARY"
   , theoremDeclaration
       "valid_cryptographic_rotation_edge_is_valid_rotation"
       "cryptographic-edge-admission"
       "A cryptographic rotation edge remains a valid logical rotation edge"
       "ASP-RFC-10.05-CTRB-CHAIN-PROJECTION"
   , theoremDeclaration
       "valid_cryptographic_chain_projects_authority_rotation_chain"
       "registry-chain-bridge"
       "A cryptographic chain projects to the authority-rotation trust chain"
       "ASP-RFC-10.05-CTRB-CHAIN-PROJECTION"
   , theoremDeclaration
       "valid_cryptographic_chain_exact_epoch_span"
       "successor-continuity"
       "A cryptographic rotation chain retains the exact registry epoch span"
       "ASP-RFC-10.05-CTRB-CHAIN-PROJECTION"
       ["propext"]
   , theoremDeclaration
       "cryptographic_chain_admitted_vector_coverage_lifts_global_pareto"
       "admitted-global-lift"
       "Cryptographic-chain-admitted coverage lifts Pareto minimality globally"
       "ASP-RFC-10.05-CTRB-GLOBAL-LIFT"
   , theoremDeclaration
       "constant_digest_collision_does_not_identify_payload"
       "digest-collision-gap"
       "Equal digests do not identify payloads without a collision assumption"
       "ASP-RFC-10.05-CTRB-DIGEST-BOUNDARY"
   , theoremDeclaration
       "example_cryptographic_receipt_is_valid"
       "cryptographic-edge-admission"
       "A fully context-bound dual-signature receipt is valid"
       "ASP-RFC-10.05-CTRB-DUAL-SIGNATURE-CONTEXT"
       ["propext"]
   , theoremDeclaration
       "signature_verification_only_allows_cross_domain_replay"
       "weak-verification-gap"
       "Signature-byte verification alone permits cross-domain replay"
       "ASP-RFC-10.05-CTRB-REPLAY-DOMAIN"
       ["propext"]
   , theoremDeclaration
       "expired_example_receipt_is_rejected"
       "temporal-scope"
       "The example receipt is rejected after expiry"
       "ASP-RFC-10.05-CTRB-TEMPORAL-SCOPE"
   , theoremDeclaration
       "algorithm_migration_rejects_old_receipt"
       "algorithm-binding"
       "Algorithm-policy migration rejects an old receipt"
       "ASP-RFC-10.05-CTRB-ALGORITHM"
   , theoremDeclaration
       "revoked_new_key_rejects_example_receipt"
       "key-revocation"
       "The example receipt is rejected after new-key revocation"
       "ASP-RFC-10.05-CTRB-KEY-EPOCH"
   , theoremDeclaration
       "example_cryptographic_rotation_edge_is_valid"
       "cryptographic-edge-admission"
       "A valid signed receipt strengthens a valid authority-rotation edge"
       "ASP-RFC-10.05-CTRB-CHAIN-PROJECTION"
       ["propext"]
   ]

def manifest : Json :=
  Json.mkObj
    [ ("schemaId", toJson "asp.lean-proof-audit.v1")
    , ("schemaVersion", toJson "1")
    , ("leanVersion", toJson "4.32.2")
    , ("proofPackage", toJson "ASPProof")
    , ("module",
        toJson "ASPProof.SearchRouteCryptographicTransferReceiptBinding")
    , ("sourcePath",
        toJson
          "packages/proofs/ASPProof/SearchRouteCryptographicTransferReceiptBinding.lean")
    , ("declarationCount", toJson declarations.size)
    , ("axiomFreeDeclarationCount", toJson 17)
    , ("axiomDependentDeclarationCount", toJson 4)
    , ("declarations", toJson declarations)
    , ("axiomInventory", toJson ["propext"])
    , ("hasSorryAx", toJson false)
    , ("rfc",
        toJson "01.16-searchroute-cryptographic-transfer-receipt-binding")
    , ("status", toJson "kernel-compiled")
    ]

end ASPProof.Audit.SearchRouteCryptographicTransferReceiptBinding
