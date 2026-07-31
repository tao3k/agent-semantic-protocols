import ASPProof.SearchRouteExecutableVerifierConformance
import Lean

open Lean

namespace ASPProof.Audit.SearchRouteExecutableVerifierConformance

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
       "valid_executable_verification_binds_all_stages"
       "stage-closure"
       "Executable verification binds all six conformance stages"
       "ASP-RFC-10.05-EVFC-STAGE-CLOSURE"
   , theoremDeclaration
       "valid_executable_verification_has_distinct_replay_executor"
       "replay-independence"
       "Executable verification requires a distinct replay executor"
       "ASP-RFC-10.05-EVFC-INDEPENDENT-REPLAY"
   , theoremDeclaration
       "noncanonical_statement_rejects_executable_verification"
       "canonical-serialization"
       "A noncanonical statement rejects executable verification"
       "ASP-RFC-10.05-EVFC-CANONICAL-SERIALIZATION"
   , theoremDeclaration
       "unresolved_key_rejects_executable_verification"
       "key-registry-resolution"
       "An unresolved signer key rejects executable verification"
       "ASP-RFC-10.05-EVFC-KEY-REGISTRY"
   , theoremDeclaration
       "wrong_registry_snapshot_rejects_executable_verification"
       "key-registry-resolution"
       "A mismatched key-registry snapshot rejects verification"
       "ASP-RFC-10.05-EVFC-KEY-REGISTRY"
   , theoremDeclaration
       "unresolved_verifier_rejects_executable_verification"
       "verifier-resolution"
       "An unresolved verifier artifact rejects executable verification"
       "ASP-RFC-10.05-EVFC-VERIFIER-RESOLUTION"
   , theoremDeclaration
       "failed_invocation_rejects_executable_verification"
       "invocation-conformance"
       "A failed verifier invocation rejects executable verification"
       "ASP-RFC-10.05-EVFC-INVOCATION"
   , theoremDeclaration
       "undecoded_response_rejects_executable_verification"
       "decode-conformance"
       "An undecoded verifier response rejects executable verification"
       "ASP-RFC-10.05-EVFC-DECODE"
   , theoremDeclaration
       "rejected_decision_rejects_executable_verification"
       "decode-conformance"
       "A decoded rejected decision rejects executable verification"
       "ASP-RFC-10.05-EVFC-DECODE"
   , theoremDeclaration
       "missing_replay_rejects_executable_verification"
       "replay-independence"
       "Missing independent replay rejects executable verification"
       "ASP-RFC-10.05-EVFC-INDEPENDENT-REPLAY"
   , theoremDeclaration
       "same_executor_replay_rejects_independence"
       "replay-independence"
       "Replay by the invocation executor is not independent"
       "ASP-RFC-10.05-EVFC-INDEPENDENT-REPLAY"
   , theoremDeclaration
       "valid_executable_transfer_verifies_both_signatures"
       "stage-closure"
       "Executable transfer verification closes both signature paths"
       "ASP-RFC-10.05-EVFC-STAGE-CLOSURE"
   , theoremDeclaration
       "valid_executable_edge_is_valid_cryptographic_edge"
       "cryptographic-edge-projection"
       "An executable edge projects to a valid cryptographic edge"
       "ASP-RFC-10.05-EVFC-CHAIN-PROJECTION"
   , theoremDeclaration
       "valid_executable_chain_projects_cryptographic_chain"
       "chain-projection"
       "An executable chain projects to the cryptographic trust chain"
       "ASP-RFC-10.05-EVFC-CHAIN-PROJECTION"
   , theoremDeclaration
       "valid_executable_chain_exact_epoch_span"
       "successor-continuity"
       "An executable chain retains the exact registry epoch span"
       "ASP-RFC-10.05-EVFC-CHAIN-PROJECTION"
       ["propext"]
   , theoremDeclaration
       "executable_chain_admitted_vector_coverage_lifts_global_pareto"
       "admitted-global-lift"
       "Executable-chain-admitted coverage lifts Pareto minimality globally"
       "ASP-RFC-10.05-EVFC-GLOBAL-LIFT"
   , theoremDeclaration
       "example_old_executable_verification_is_valid"
       "stage-closure"
       "The old-authority example closes all executable stages"
       "ASP-RFC-10.05-EVFC-STAGE-CLOSURE"
       ["propext"]
   , theoremDeclaration
       "example_new_executable_verification_is_valid"
       "stage-closure"
       "The new-authority example closes all executable stages"
       "ASP-RFC-10.05-EVFC-STAGE-CLOSURE"
       ["propext"]
   , theoremDeclaration
       "example_executable_transfer_verifies_both_signatures"
       "stage-closure"
       "The example transfer closes both executable signature paths"
       "ASP-RFC-10.05-EVFC-STAGE-CLOSURE"
       ["propext"]
   , theoremDeclaration
       "digest_and_resolved_prefix_do_not_prove_execution"
       "prefix-insufficiency"
       "Digest binding without verifier resolution is not execution closure"
       "ASP-RFC-10.05-EVFC-PREFIX-INSUFFICIENT"
   , theoremDeclaration
       "same_executor_replay_is_not_independent"
       "prefix-insufficiency"
       "Invocation and decode do not close same-executor replay"
       "ASP-RFC-10.05-EVFC-PREFIX-INSUFFICIENT"
   , theoremDeclaration
       "wrong_registry_snapshot_example_is_rejected"
       "key-registry-resolution"
       "The example rejects a mismatched key-registry snapshot"
       "ASP-RFC-10.05-EVFC-KEY-REGISTRY"
   , theoremDeclaration
       "undecoded_example_response_is_rejected"
       "decode-conformance"
       "The example rejects an undecoded verifier response"
       "ASP-RFC-10.05-EVFC-DECODE"
   , theoremDeclaration
       "example_executable_cryptographic_edge_is_valid"
       "cryptographic-edge-projection"
       "The fully executable example strengthens its cryptographic edge"
       "ASP-RFC-10.05-EVFC-CHAIN-PROJECTION"
       ["propext"]
   ]

def manifest : Json :=
  Json.mkObj
    [ ("schemaId", toJson "asp.lean-proof-audit.v1")
    , ("schemaVersion", toJson "1")
    , ("leanVersion", toJson "4.32.2")
    , ("proofPackage", toJson "ASPProof")
    , ("module",
        toJson "ASPProof.SearchRouteExecutableVerifierConformance")
    , ("sourcePath",
        toJson
          "packages/proofs/ASPProof/SearchRouteExecutableVerifierConformance.lean")
    , ("declarationCount", toJson declarations.size)
    , ("axiomFreeDeclarationCount", toJson 19)
    , ("axiomDependentDeclarationCount", toJson 5)
    , ("declarations", toJson declarations)
    , ("axiomInventory", toJson ["propext"])
    , ("hasSorryAx", toJson false)
    , ("rfc",
        toJson "01.17-searchroute-executable-verifier-conformance")
    , ("status", toJson "kernel-compiled")
    ]

end ASPProof.Audit.SearchRouteExecutableVerifierConformance
