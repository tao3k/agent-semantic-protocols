import Lean
import ASPProof.SearchRouteTriadRelationshipContract

open Lean Elab Command

private structure AuditSpec where
  name : Name
  family : String

private def specs : List AuditSpec := [
  ⟨``ASPProof.SearchRouteTriadRelationshipContract.exact_binding_has_identifier_binding,
    "exact-relationship-binding"⟩,
  ⟨``ASPProof.SearchRouteTriadRelationshipContract.changed_clause_digest_invalidates,
    "relationship-semantic-invalidation"⟩,
  ⟨``ASPProof.SearchRouteTriadRelationshipContract.changed_dependency_set_invalidates,
    "relationship-dependency-invalidation"⟩,
  ⟨``ASPProof.SearchRouteTriadRelationshipContract.identifier_only_binding_accepts_changed_semantics,
    "identifier-only-counterexample"⟩,
  ⟨``ASPProof.SearchRouteTriadRelationshipContract.exact_binding_rejects_changed_semantics,
    "relationship-semantic-invalidation"⟩,
  ⟨``ASPProof.SearchRouteTriadRelationshipContract.subset_check_accepts_underdeclared_dependencies,
    "coverage-subset-counterexample"⟩,
  ⟨``ASPProof.SearchRouteTriadRelationshipContract.exact_coverage_rejects_underdeclared_dependencies,
    "exact-audit-coverage"⟩,
  ⟨``ASPProof.SearchRouteTriadRelationshipContract.underdeclared_coverage_blocks_gate,
    "exact-audit-coverage"⟩,
  ⟨``ASPProof.SearchRouteTriadRelationshipContract.stale_clause_blocks_gate,
    "relationship-semantic-invalidation"⟩,
  ⟨``ASPProof.SearchRouteTriadRelationshipContract.admitted_gate_has_org_contract,
    "org-contract-relationship-binding"⟩,
  ⟨``ASPProof.SearchRouteTriadRelationshipContract.exact_digests_without_org_contract_are_not_admitted,
    "org-contract-relationship-binding"⟩,
  ⟨``ASPProof.SearchRouteTriadRelationshipContract.contract_and_exact_evidence_admit,
    "org-contract-relationship-binding"⟩,
  ⟨``ASPProof.SearchRouteTriadRelationshipContract.unrelated_clause_change_preserves_local_binding,
    "frontier-local-reuse"⟩
]

run_cmd do
  let environment ← getEnv
  let mut declarations : Array Json := #[]
  let mut inventory : NameHashSet := {}
  let mut axiomFreeCount := 0
  for spec in specs do
    let axioms ← collectAxioms spec.name
    inventory := inventory.insertMany axioms
    if axioms.isEmpty then
      axiomFreeCount := axiomFreeCount + 1
    let typeText := match environment.find? spec.name with
      | some info => toString info.type
      | none => "missing declaration"
    declarations := declarations.push <| Json.mkObj [
      ("name", toJson spec.name.toString),
      ("kind", toJson "theorem"),
      ("theoremFamily", toJson spec.family),
      ("rfcClauseIds", toJson ["ASP-RFC-10.05.64-RELATIONSHIP-CONTRACT"]),
      ("type", toJson typeText),
      ("axioms", toJson (axioms.toList.map Name.toString)),
      ("hasSorryAx", toJson (axioms.contains ``sorryAx))
    ]
  let receipt := Json.mkObj [
    ("schemaId", toJson "asp.lean-proof-audit.v1"),
    ("schemaVersion", toJson "1"),
    ("proofPackage", toJson "ASPProof"),
    ("module", toJson "ASPProof.SearchRouteTriadRelationshipContract"),
    ("sourcePath", toJson "ASPProof/SearchRouteTriadRelationshipContract.lean"),
    ("leanVersion", toJson versionString),
    ("declarationCount", toJson specs.length),
    ("axiomFreeDeclarationCount", toJson axiomFreeCount),
    ("axiomDependentDeclarationCount", toJson (specs.length - axiomFreeCount)),
    ("axiomInventory", toJson (inventory.toList.map Name.toString)),
    ("hasSorryAx", toJson (inventory.contains ``sorryAx)),
    ("declarations", Json.arr declarations)
  ]
  liftIO <| IO.println receipt.pretty
