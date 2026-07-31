import ASPProof.Audit.Core
import ASPProof.SearchRouteAndOrEvidenceGraph

namespace ASPProof.Audit.SearchRouteAndOrEvidenceGraph

def declarationFamilies : List (String × List String) :=
  [
    (
      "realization-semantics",
      [
        "realizes_all_iff",
        "realizes_any_iff",
        "plan_certifies_implies_realizes"
      ]
    ),
    (
      "conjunctive-counterexample",
      [
        "provider_atom_is_reachable",
        "provider_only_realizes_alternative",
        "provider_only_does_not_realize_conjunction",
        "atomic_reachability_does_not_realize_required_evidence",
        "no_plan_certifies_missing_conjunct"
      ]
    )
  ]

def expectedAxiomFree : List String :=
  declarationFamilies.flatMap Prod.snd

def familyJson (family : String × List String) : Lean.Json :=
  Lean.Json.mkObj
    [
      ("family", .str family.1),
      ("declarations", .arr (family.2.map Lean.Json.str).toArray)
    ]

def auditManifest : Lean.Json :=
  Lean.Json.mkObj
    [
      ("schema", .str "asp.lean-audit-manifest.v1"),
      ("rfc", .str "10.05.10.00.37"),
      (
        "module",
        .str "ASPProof.SearchRouteAndOrEvidenceGraph"
      ),
      (
        "families",
        .arr (declarationFamilies.map familyJson).toArray
      ),
      (
        "expectedAxiomFree",
        .arr (expectedAxiomFree.map Lean.Json.str).toArray
      )
    ]

end ASPProof.Audit.SearchRouteAndOrEvidenceGraph

