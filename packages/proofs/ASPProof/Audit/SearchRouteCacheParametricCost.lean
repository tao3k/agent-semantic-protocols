import ASPProof.Audit.Core
import ASPProof.SearchRouteCacheParametricCost

namespace ASPProof.Audit.SearchRouteCacheParametricCost

def declarationFamilies : List (String × List String) :=
  [
    (
      "cache-binding",
      [
        "cache_bound_refl",
        "semantic_receipt_drift_rejects",
        "semantic_cache_drift_rejects",
        "semantic_root_drift_rejects",
        "prefix_cache_drift_rejects"
      ]
    ),
    (
      "runtime-drift",
      [
        "model_drift_rejects",
        "prefix_state_drift_rejects",
        "cost_model_drift_rejects"
      ]
    ),
    (
      "conservative-cost",
      ["effective_tokens_le_conservative"]
    ),
    (
      "counterexample",
      [
        "route_a_hit_certificate_rejected_after_miss",
        "verified_hit_prefers_route_a",
        "miss_prefers_route_b",
        "route_order_flips_with_prefix_state"
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
      ("rfc", .str "10.05.10.00.33"),
      (
        "module",
        .str "ASPProof.SearchRouteCacheParametricCost"
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

end ASPProof.Audit.SearchRouteCacheParametricCost
