import Lean
import ASPProof.SearchRouteScipyMixedRadix

open Lean Elab Command

private structure AuditSpec where
  name : Name
  family : String
  clauses : List String

private def specs : List AuditSpec := [
  ⟨``ASPProof.SearchRouteScipyMixedRadix.appendDigit_lt_product,
    "mixed-radix-bound", ["ASP-RFC-10.05.64-BOUNDED-RADIX"]⟩,
  ⟨``ASPProof.SearchRouteScipyMixedRadix.cacheTail_lt_base,
    "mixed-radix-bound", ["ASP-RFC-10.05.64-BOUNDED-RADIX"]⟩,
  ⟨``ASPProof.SearchRouteScipyMixedRadix.tokenTail_lt_base,
    "mixed-radix-bound", ["ASP-RFC-10.05.64-BOUNDED-RADIX"]⟩,
  ⟨``ASPProof.SearchRouteScipyMixedRadix.lowerTail_lt_base,
    "mixed-radix-bound", ["ASP-RFC-10.05.64-BOUNDED-RADIX"]⟩,
  ⟨``ASPProof.SearchRouteScipyMixedRadix.appendDigit_priority,
    "lexicographic-priority", ["ASP-RFC-10.05.64-LEXICOGRAPHIC-ORDER"]⟩,
  ⟨``ASPProof.SearchRouteScipyMixedRadix.fewer_hops_dominate_all_bounded_lower_costs,
    "lexicographic-priority", ["ASP-RFC-10.05.64-LEXICOGRAPHIC-ORDER"]⟩,
  ⟨``ASPProof.SearchRouteScipyMixedRadix.fewer_rounds_dominate_bounded_token_and_cache_costs,
    "lexicographic-priority", ["ASP-RFC-10.05.64-LEXICOGRAPHIC-ORDER"]⟩,
  ⟨``ASPProof.SearchRouteScipyMixedRadix.first_unsafe_integer_exceeds_float64_exact_cap,
    "float64-exactness", ["ASP-RFC-10.05.64-FLOAT64-EXACTNESS"]⟩,
  ⟨``ASPProof.SearchRouteScipyMixedRadix.search_cache_and_model_cache_are_distinct_digits,
    "cache-domain-separation", ["ASP-RFC-10.05.64-COST-VECTOR"]⟩,
  ⟨``ASPProof.SearchRouteScipyMixedRadix.python_direct_route_encoding_is_904,
    "python-lean-conformance", ["ASP-RFC-10.05.64-PYTHON-LEAN-CONFORMANCE"]⟩,
  ⟨``ASPProof.SearchRouteScipyMixedRadix.python_direct_route_beats_longer_route,
    "python-lean-conformance", ["ASP-RFC-10.05.64-PYTHON-LEAN-CONFORMANCE"]⟩,
  ⟨``ASPProof.SearchRouteScipyMixedRadix.python_fast_round_route_encoding_is_1211,
    "python-lean-conformance", ["ASP-RFC-10.05.64-PYTHON-LEAN-CONFORMANCE"]⟩,
  ⟨``ASPProof.SearchRouteScipyMixedRadix.python_fast_round_route_beats_cheaper_token_route,
    "python-lean-conformance", ["ASP-RFC-10.05.64-PYTHON-LEAN-CONFORMANCE"]⟩,
  ⟨``ASPProof.SearchRouteScipyMixedRadix.python_search_hit_route_encoding_is_19,
    "python-lean-conformance", ["ASP-RFC-10.05.64-PYTHON-LEAN-CONFORMANCE"]⟩,
  ⟨``ASPProof.SearchRouteScipyMixedRadix.python_search_hit_route_beats_search_miss_route,
    "python-lean-conformance", ["ASP-RFC-10.05.64-PYTHON-LEAN-CONFORMANCE"]⟩
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
      ("rfcClauseIds", toJson spec.clauses),
      ("type", toJson typeText),
      ("axioms", toJson (axioms.toList.map Name.toString)),
      ("hasSorryAx", toJson (axioms.contains ``sorryAx))
    ]
  let receipt := Json.mkObj [
    ("schemaId", toJson "asp.lean-proof-audit.v1"),
    ("schemaVersion", toJson "1"),
    ("proofPackage", toJson "ASPProof"),
    ("module", toJson "ASPProof.SearchRouteScipyMixedRadix"),
    ("sourcePath", toJson "ASPProof/SearchRouteScipyMixedRadix.lean"),
    ("leanVersion", toJson versionString),
    ("declarationCount", toJson specs.length),
    ("axiomFreeDeclarationCount", toJson axiomFreeCount),
    ("axiomDependentDeclarationCount", toJson (specs.length - axiomFreeCount)),
    ("axiomInventory", toJson (inventory.toList.map Name.toString)),
    ("hasSorryAx", toJson (inventory.contains ``sorryAx)),
    ("declarations", Json.arr declarations)
  ]
  liftIO <| IO.println receipt.pretty
