import ASPProof.Audit.Core
import ASPProof.SearchRouteMonotoneInspectRefinement

namespace ASPProof.Audit.SearchRouteMonotoneInspectRefinement

def declarationFamilies : List (String × List String) :=
  [
    (
      "local-progress",
      [
        "inspect_self_loop_is_rejected",
        "unchanged_ambiguity_is_rejected",
        "resolved_state_has_no_refinement",
        "example_refinement_is_valid",
        "budget_consumption_without_information_is_invalid",
        "resolved_example_has_no_next_step"
      ]
    ),
    (
      "bounded-run",
      [
        "run_length_le_initial_ambiguity",
        "run_length_le_initial_rounds",
        "run_length_le_initial_transitions"
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
      ("rfc", .str "10.05.10.00.41"),
      (
        "module",
        .str "ASPProof.SearchRouteMonotoneInspectRefinement"
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

end ASPProof.Audit.SearchRouteMonotoneInspectRefinement

