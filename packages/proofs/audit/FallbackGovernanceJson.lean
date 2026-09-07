-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import Lean
import ASPProof.FallbackGovernance

open Lean Elab Command

namespace ASPProof.FallbackGovernanceJsonAudit

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
  deriving ToJson

def declarationNames : Array Name := #[
  ``ASPProof.FallbackGovernance.proof_lane_rejects_operational_fallback,
  ``ASPProof.FallbackGovernance.no_eager_fallback,
  ``ASPProof.FallbackGovernance.no_unauthorized_fallback,
  ``ASPProof.FallbackGovernance.no_silent_degradation,
  ``ASPProof.FallbackGovernance.guarded_fallback_does_not_inflate_guarantees,
  ``ASPProof.FallbackGovernance.non_equivalent_fallback_separates_cache,
  ``ASPProof.FallbackGovernance.missing_required_guarantee_denies_acceptance,
  ``ASPProof.FallbackGovernance.no_zero_budget_fallback,
  ``ASPProof.FallbackGovernance.no_rank_cycle,
  ``ASPProof.FallbackGovernance.rank_step_well_founded
]

def appendUnique (values additions : Array String) : Array String :=
  additions.foldl
    (fun result value => if result.contains value then result else result.push value)
    values

run_cmd do
  let declarations ← declarationNames.mapM fun declaration => do
    let axioms ← Lean.collectAxioms declaration
    pure {
      declaration := declaration.toString
      axioms := axioms.map Name.toString
    }
  let axiomFreeDeclarations := declarations.foldl
    (fun result declaration =>
      if declaration.axioms.isEmpty then
        result.push declaration.declaration
      else
        result)
    #[]
  let axiomDependentDeclarations := declarations.foldl
    (fun result declaration =>
      if declaration.axioms.isEmpty then
        result
      else
        result.push declaration.declaration)
    #[]
  let axiomInventory := declarations.foldl
    (fun result declaration => appendUnique result declaration.axioms)
    #[]
  let receipt : AuditReceipt := {
    schema := "asp.lean-proof-audit.v1"
    moduleName := "ASPProof.FallbackGovernance"
    source := "packages/proofs/ASPProof/FallbackGovernance.lean"
    declarations
    axiomFreeDeclarations
    axiomDependentDeclarations
    axiomInventory
    sorryAx := axiomInventory.contains "sorryAx"
    theoremFamilies := #[
      "proof-lane-closure",
      "fallback-authorization",
      "guarantee-preservation",
      "cache-domain-separation",
      "fallback-termination"
    ]
    clauseCoverage := #[
      "FG-PROOF-001",
      "FG-PROOF-002",
      "FG-PROOF-003",
      "runtime-failure-observed",
      "runtime-authorized",
      "runtime-degradation-declared",
      "runtime-rank-decreases",
      "runtime-budget-decreases",
      "runtime-required-guarantees",
      "runtime-cache-separated"
    ]
  }
  liftIO <| IO.println (toJson receipt).compress

end ASPProof.FallbackGovernanceJsonAudit
