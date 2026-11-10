-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import Lean
import ASPProof.SearchRouteTriadHandoffIdentity

open Lean Elab Command

private structure AuditSpec where
  name : Name
  clauses : List String

private def identityClause : String :=
  "ASP-RFC-10.05.64-TRIAD-HANDOFF-IDENTITY"

private def preflightClause : String :=
  "ASP-RFC-10.05.64-TRIAD-PREFLIGHT-VALIDITY"

private def specs : List AuditSpec := [
  ⟨``ASPProof.SearchRouteTriadHandoffIdentity.admitted_handoff_has_one_receipt_chain,
    [identityClause]⟩,
  ⟨``ASPProof.SearchRouteTriadHandoffIdentity.admitted_handoff_has_all_gates,
    [identityClause, preflightClause]⟩,
  ⟨``ASPProof.SearchRouteTriadHandoffIdentity.same_chain_and_ready_gates_admit,
    [identityClause, preflightClause]⟩,
  ⟨``ASPProof.SearchRouteTriadHandoffIdentity.invalid_execution_configuration_blocks_admission,
    [preflightClause]⟩,
  ⟨``ASPProof.SearchRouteTriadHandoffIdentity.unauthenticated_chain_identity_blocks_admission,
    [identityClause]⟩,
  ⟨``ASPProof.SearchRouteTriadHandoffIdentity.unreplayed_normalized_audit_blocks_admission,
    [preflightClause]⟩,
  ⟨``ASPProof.SearchRouteTriadHandoffIdentity.all_green_but_spliced_receipts_are_not_admitted,
    [identityClause]⟩,
  ⟨``ASPProof.SearchRouteTriadHandoffIdentity.fully_ready_handoff_is_admitted,
    [identityClause, preflightClause]⟩,
  ⟨``ASPProof.SearchRouteTriadHandoffIdentity.generator_only_audit_is_not_admitted,
    [preflightClause]⟩,
  ⟨``ASPProof.SearchRouteTriadHandoffIdentity.observed_v1_preflight_failure_is_not_admitted,
    [preflightClause]⟩
]

run_cmd do
  let environment ← getEnv
  let mut declarationJson : Array Json := #[]
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
    declarationJson := declarationJson.push <| Json.mkObj [
      ("name", toJson spec.name.toString),
      ("kind", toJson "theorem"),
      ("theoremFamily", toJson "triad-handoff-identity"),
      ("rfcClauseIds", toJson spec.clauses),
      ("type", toJson typeText),
      ("axioms", toJson (axioms.toList.map Name.toString)),
      ("hasSorryAx", toJson (axioms.contains ``sorryAx))
    ]
  let receipt := Json.mkObj [
    ("schemaId", toJson "asp.lean-proof-audit.v1"),
    ("schemaVersion", toJson "1"),
    ("proofPackage", toJson "ASPProof"),
    ("module", toJson "ASPProof.SearchRouteTriadHandoffIdentity"),
    ("sourcePath", toJson "ASPProof/SearchRouteTriadHandoffIdentity.lean"),
    ("leanVersion", toJson versionString),
    ("declarationCount", toJson specs.length),
    ("axiomFreeDeclarationCount", toJson axiomFreeCount),
    ("axiomDependentDeclarationCount",
      toJson (specs.length - axiomFreeCount)),
    ("axiomInventory", toJson (inventory.toList.map Name.toString)),
    ("hasSorryAx", toJson (inventory.contains ``sorryAx)),
    ("declarations", Json.arr declarationJson)
  ]
  liftIO <| IO.println receipt.pretty
