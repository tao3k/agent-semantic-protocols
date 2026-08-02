import Lean
import ASPProof.RuntimeBinaryProfileDispatch

open Lean Elab Command

namespace ASPProof.RuntimeBinaryProfileDispatchJsonAudit

structure DeclarationAudit where
  declaration : String
  axioms : Array String
  deriving ToJson

structure AuditReceipt where
  schema : String
  moduleName : String
  source : String
  declarations : Array DeclarationAudit
  axiomFreeDeclarations : Array String
  axiomDependentDeclarations : Array String
  axiomInventory : Array String
  sorryAx : Bool
  theoremFamilies : Array String
  clauseCoverage : Array String
  counterexamples : Array String
  deriving ToJson

def declarationNames : Array Name := #[
  ``ASPProof.RuntimeBinaryProfileDispatch.provider_internal_admission_requires_runtime_capability,
  ``ASPProof.RuntimeBinaryProfileDispatch.matching_live_provider_capability_is_admitted,
  ``ASPProof.RuntimeBinaryProfileDispatch.direct_provider_internal_is_denied,
  ``ASPProof.RuntimeBinaryProfileDispatch.invalid_provider_capability_is_denied,
  ``ASPProof.RuntimeBinaryProfileDispatch.wrong_root_session_is_denied,
  ``ASPProof.RuntimeBinaryProfileDispatch.wrong_active_root_session_is_denied,
  ``ASPProof.RuntimeBinaryProfileDispatch.wrong_child_session_is_denied,
  ``ASPProof.RuntimeBinaryProfileDispatch.wrong_active_child_session_is_denied,
  ``ASPProof.RuntimeBinaryProfileDispatch.wrong_binary_is_denied,
  ``ASPProof.RuntimeBinaryProfileDispatch.wrong_profile_is_denied,
  ``ASPProof.RuntimeBinaryProfileDispatch.wrong_provider_is_denied,
  ``ASPProof.RuntimeBinaryProfileDispatch.wrong_language_is_denied,
  ``ASPProof.RuntimeBinaryProfileDispatch.wrong_generation_is_denied,
  ``ASPProof.RuntimeBinaryProfileDispatch.wrong_artifact_digest_is_denied,
  ``ASPProof.RuntimeBinaryProfileDispatch.wrong_argv_digest_is_denied,
  ``ASPProof.RuntimeBinaryProfileDispatch.wrong_attempt_is_replay_denied,
  ``ASPProof.RuntimeBinaryProfileDispatch.not_yet_issued_capability_is_denied,
  ``ASPProof.RuntimeBinaryProfileDispatch.expired_capability_is_denied,
  ``ASPProof.RuntimeBinaryProfileDispatch.consumed_capability_is_replay_denied,
  ``ASPProof.RuntimeBinaryProfileDispatch.test_fixture_outside_test_runtime_is_denied,
  ``ASPProof.RuntimeBinaryProfileDispatch.facade_is_not_misclassified_as_provider_internal,
  ``ASPProof.RuntimeBinaryProfileDispatch.unregistered_runtime_binary_fails_closed,
  ``ASPProof.RuntimeBinaryProfileDispatch.profile_admission_is_not_a_binary_name_blacklist
]

def appendUnique (values additions : Array String) : Array String :=
  additions.foldl
    (fun result value => if result.contains value then result else result.push value)
    values

run_cmd do
  let declarations ← declarationNames.mapM fun declaration => do
    let axioms ← Lean.collectAxioms declaration
    pure { declaration := declaration.toString, axioms := axioms.map Name.toString }
  let axiomFreeDeclarations := declarations.foldl
    (fun result declaration =>
      if declaration.axioms.isEmpty then result.push declaration.declaration else result)
    #[]
  let axiomDependentDeclarations := declarations.foldl
    (fun result declaration =>
      if declaration.axioms.isEmpty then result else result.push declaration.declaration)
    #[]
  let axiomInventory := declarations.foldl
    (fun result declaration => appendUnique result declaration.axioms) #[]
  let receipt : AuditReceipt := {
    schema := "asp.lean-proof-audit.v1"
    moduleName := "ASPProof.RuntimeBinaryProfileDispatch"
    source := "packages/proofs/ASPProof/RuntimeBinaryProfileDispatch.lean"
    declarations
    axiomFreeDeclarations
    axiomDependentDeclarations
    axiomInventory
    sorryAx := axiomInventory.contains "sorryAx"
    theoremFamilies := #[
      "profile-based-admission",
      "session-bound-single-use-capability",
      "exact-dispatch-identity",
      "capability-liveness-and-replay-denial",
      "test-runtime-isolation",
      "facade-and-unregistered-boundaries"
    ]
    clauseCoverage := #[
      "RBPD-PROFILE-001",
      "RBPD-CAPABILITY-002",
      "RBPD-SESSION-003",
      "RBPD-IDENTITY-004",
      "RBPD-LIVENESS-005",
      "RBPD-SINGLE-USE-006",
      "RBPD-TEST-007",
      "RBPD-UNREGISTERED-008"
    ]
    counterexamples := #[
      "direct-provider-internal-denied",
      "wrong-capability-field-denied",
      "expired-or-consumed-capability-denied",
      "test-fixture-outside-test-runtime-denied",
      "unregistered-runtime-binary-denied",
      "binary-name-does-not-determine-profile"
    ]
  }
  liftIO <| IO.println (toJson receipt).compress

end ASPProof.RuntimeBinaryProfileDispatchJsonAudit
