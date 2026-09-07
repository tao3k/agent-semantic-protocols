-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import Lean
import ASPProof.SearchRoutePythonLeanSubagentTriad

open Lean Elab Command

private def declarations : List Name := [
  ``ASPProof.SearchRoutePythonLeanSubagentTriad.complete_evidence_is_admitted,
  ``ASPProof.SearchRoutePythonLeanSubagentTriad.missing_lean_receipt_blocks_admission,
  ``ASPProof.SearchRoutePythonLeanSubagentTriad.missing_golden_agreement_blocks_admission,
  ``ASPProof.SearchRoutePythonLeanSubagentTriad.parent_confirmation_does_not_close_nested_receipt,
  ``ASPProof.SearchRoutePythonLeanSubagentTriad.observed_nested_receipt_is_not_admitted,
  ``ASPProof.SearchRoutePythonLeanSubagentTriad.observed_parent_receipt_is_admitted
]

run_cmd do
  let environment ← getEnv
  let mut declarationJson : Array Json := #[]
  let mut inventory : NameHashSet := {}
  let mut axiomFreeCount := 0
  for name in declarations do
    let axioms ← collectAxioms name
    inventory := inventory.insertMany axioms
    if axioms.isEmpty then
      axiomFreeCount := axiomFreeCount + 1
    let typeText := match environment.find? name with
      | some info => toString info.type
      | none => "missing declaration"
    declarationJson := declarationJson.push <| Json.mkObj [
      ("name", toJson name.toString),
      ("kind", toJson "theorem"),
      ("theoremFamily", toJson "subagent-triad-admission"),
      ("rfcClauseIds", toJson ["ASP-RFC-10.05.64-SUBAGENT-TRIAD-ADMISSION"]),
      ("type", toJson typeText),
      ("axioms", toJson (axioms.toList.map Name.toString)),
      ("hasSorryAx", toJson (axioms.contains ``sorryAx))
    ]
  let receipt := Json.mkObj [
    ("schemaId", toJson "asp.lean-proof-audit.v1"),
    ("schemaVersion", toJson "1"),
    ("proofPackage", toJson "ASPProof"),
    ("module", toJson "ASPProof.SearchRoutePythonLeanSubagentTriad"),
    ("sourcePath", toJson "ASPProof/SearchRoutePythonLeanSubagentTriad.lean"),
    ("leanVersion", toJson versionString),
    ("declarationCount", toJson declarations.length),
    ("axiomFreeDeclarationCount", toJson axiomFreeCount),
    ("axiomDependentDeclarationCount",
      toJson (declarations.length - axiomFreeCount)),
    ("axiomInventory", toJson (inventory.toList.map Name.toString)),
    ("hasSorryAx", toJson (inventory.contains ``sorryAx)),
    ("declarations", Json.arr declarationJson)
  ]
  liftIO <| IO.println receipt.pretty
