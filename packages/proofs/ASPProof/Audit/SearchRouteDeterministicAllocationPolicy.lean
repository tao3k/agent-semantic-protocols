import ASPProof.SearchRouteDeterministicAllocationPolicy
import Lean

namespace ASPProof.Audit.SearchRouteDeterministicAllocationPolicy

open Lean

private def strings (values : Array String) : Json :=
  Json.arr (values.map Json.str)

private def declaration
    (name theoremFamily declarationType : String)
    (rfcClauseIds axioms : Array String) : Json :=
  Json.mkObj
    [ ("name", Json.str name)
    , ("kind", Json.str "theorem")
    , ("type", Json.str declarationType)
    , ("theoremFamily", Json.str theoremFamily)
    , ("rfcClauseIds", strings rfcClauseIds)
    , ("axioms", strings axioms)
    , ("hasSorryAx", Json.bool false)
    ]

def declarations : Array Json :=
  #[ declaration
       "ASPProof.SearchRouteDeterministicAllocationPolicy.policy_digest_is_injective"
       "policy-digest-injectivity"
       "the resolved finite policy digest uniquely identifies allocation semantics"
       #["ASP-RFC-10.05-DAP-INJECTIVE"] #["propext"]
   , declaration
       "ASPProof.SearchRouteDeterministicAllocationPolicy.evaluated_allocation_conserves_total"
       "policy-evaluation-conservation"
       "every resolved policy evaluation conserves complete batch token cost"
       #["ASP-RFC-10.05-DAP-CONSERVES"] #["propext"]
   , declaration
       "ASPProof.SearchRouteDeterministicAllocationPolicy.certified_allocation_conserves_total"
       "certified-policy-conservation"
       "a certified allocation inherits conservation from its resolved policy"
       #["ASP-RFC-10.05-DAP-CERTIFIED"] #["propext"]
   , declaration
       "ASPProof.SearchRouteDeterministicAllocationPolicy.same_digest_and_facts_replay_same_allocation"
       "deterministic-allocation-replay"
       "same resolved policy digest and identical batch facts replay the same allocation"
       #["ASP-RFC-10.05-DAP-REPLAY"] #["propext"]
   , declaration
       "ASPProof.SearchRouteDeterministicAllocationPolicy.replay_comparability_is_reflexive"
       "replay-comparability-reflexivity"
       "every allocation certificate is replay comparable with itself"
       #["ASP-RFC-10.05-DAP-REFLEXIVE"] #[]
   , declaration
       "ASPProof.SearchRouteDeterministicAllocationPolicy.unsafe_digest_is_not_injective"
       "noninjective-digest-counterexample"
       "a constant raw digest cannot identify distinct policy semantics"
       #["ASP-RFC-10.05-DAP-NONINJECTIVE"] #[]
   , declaration
       "ASPProof.SearchRouteDeterministicAllocationPolicy.colliding_digest_breaks_allocation_replay"
       "digest-collision-counterexample"
       "colliding policy digests produce different allocations for identical facts"
       #["ASP-RFC-10.05-DAP-COLLISION"] #[]
   , declaration
       "ASPProof.SearchRouteDeterministicAllocationPolicy.same_policy_digest_without_same_facts_is_insufficient"
       "batch-facts-identity"
       "same policy digest without equal batch facts does not determine allocation"
       #["ASP-RFC-10.05-DAP-FACTS"] #[]
   , declaration
       "ASPProof.SearchRouteDeterministicAllocationPolicy.cache_context_change_rejects_replay_comparison"
       "cache-context-replay-rejection"
       "a cache-context change rejects deterministic replay comparison"
       #["ASP-RFC-10.05-DAP-CACHE"] #[]
   , declaration
       "ASPProof.SearchRouteDeterministicAllocationPolicy.deterministic_replay_preserves_pareto_cost"
       "deterministic-pareto-cost"
       "deterministic replay preserves the allocated route cost used by Pareto comparison"
       #["ASP-RFC-10.05-DAP-PARETO"] #["propext"]
   ]

def receipt : Json :=
  Json.mkObj
    [ ("schemaId", Json.str "asp.lean-proof-audit.v1")
    , ("schemaVersion", Json.str "1")
    , ("leanVersion", Json.str "4.32.2")
    , ("proofPackage", Json.str "ASPProof")
    , ("module", Json.str "ASPProof.SearchRouteDeterministicAllocationPolicy")
    , ("sourcePath",
        Json.str "packages/proofs/ASPProof/SearchRouteDeterministicAllocationPolicy.lean")
    , ("declarations", Json.arr declarations)
    , ("declarationCount", Json.num 10)
    , ("axiomFreeDeclarationCount", Json.num 5)
    , ("axiomDependentDeclarationCount", Json.num 5)
    , ("axiomInventory", strings #["propext"])
    , ("hasSorryAx", Json.bool false)
    ]

end ASPProof.Audit.SearchRouteDeterministicAllocationPolicy
