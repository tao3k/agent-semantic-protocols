#set document(
  title: "SearchLoop Clause-first Mathematical Model",
  author: "Agent Semantic Protocols",
)
#set page(margin: 22mm)
#set text(font: "New Computer Modern", size: 10.5pt)
#set heading(numbering: "1.")
#show raw: set text(font: "DejaVu Sans Mono", size: 8.5pt)

= Scope and artifact identity

This note is the mathematical explanation corresponding to
`packages/proofs/ASPProof/SearchLoopClauseFirst.lean`.
It explains the first executable model of:

- `ASP-RFC-10.05-CFR-GOD-DECISION`,
- `ASP-RFC-10.05-BECA-STATES`,
- `ASP-RFC-10.05-CFR-GOD-GLOBAL-BUDGET`,
- `ASP-RFC-10.05-RPGE-CLOSED-WORLD`,
- `ASP-RFC-10.05-CFR-GOD-GRAPH-AUTHORITY`.

The Lean source is authoritative for checked derivations. This Typst document
is the human-facing mathematical projection. Neither artifact is an
implementation-readiness receipt.

= Validation domain

Every reusable search fact is interpreted inside a validation domain

$ D = (s, p, sigma, pi), $

where $s$ is the source snapshot, $p$ the provider identity, $sigma$ the schema
identity, and $pi$ the policy identity. Two bindings or cache entries are
compatible only when their required domain components agree.

This makes freshness and provider authority part of semantic identity rather
than optional cache metadata.

= Physical postings and logical facts

A physical posting is modeled as

$ P = (f, r, e, D), $

where $f$ is a logical fact identity, $r$ an argument role, and $e$ an entity.
Two postings may mention the same entity without belonging to the same fact:

$ e(P_1) = e(P_2) quad "does not imply" quad f(P_1) = f(P_2). $

The Lean theorem `shared_endpoint_does_not_imply_same_fact` constructs this
counterexample. Therefore relation postings must reconstruct by admitted fact
identity and validation domain, not by endpoint co-occurrence.

= Clause-state classification

Let the Boolean predicates be:

$ W(c) $: a valid sufficiency witness exists,

$ P(c) $: an admitted relation-posting action exists,

$ Q(c) $: a bounded discovery/canonicalization action exists.

Treating these as independent states is unsound because

$ W(c) and P(c) $

can both hold. Lean checks this witness in
`naive_clause_predicates_can_overlap`.

The repaired classification is an ordered function:

$ op("classify")(c) =
cases(
  "resolved" & "if " W(c),
  "searchable" & "else if " P(c),
  "fuzzy" & "else if " Q(c),
  "stalled" & "otherwise."
) $

The ordering makes the state total and deterministic. The Lean theorems
`sufficiency_has_classification_precedence`,
`posting_action_precedes_discovery`,
`discovery_without_posting_is_fuzzy`, and
`no_admitted_action_is_stalled` check the four branches.

= Mandatory fairness and insufficient budget

Let $m$ be the number of mandatory searchable obligations and $b$ the available
unit-floor budget. Unconditional positive allocation is impossible when

$ b < m. $

Lean checks the minimal witness $b = 0, m = 1$ in
`zero_budget_cannot_cover_one_mandatory`.

The RFC repair is not to invent fractional success. The disposition is:

$ op("disposition")(b, m) =
cases(
  "schedulable" & "if " m <= b,
  "needsBudget" & "if " b < m.
) $

When $m <= b$, every mandatory index $i < m$ receives the floor

$ A(b, m, i) = 1. $

This is checked by `sufficient_budget_gives_positive_floor`.

= Search-once validity

A searched action is not identified only by clause text. Its key contains

$ K = (c, s, p, sigma, pi, beta, q), $

where $beta$ is the binding digest and $q$ the query-pack digest.

If the snapshot changes from $s$ to $s'$ with $s != s'$, then

$ K(c, s, dots) != K(c, s', dots). $

Lean proves this as `snapshot_drift_changes_action_identity`. Thus “search only
once” is valid only within a stable dependency identity.

= No-hit versus absence

A lookup result is

$ R = (h, C, F), $

where $h$ is the hit count, $C$ states complete admitted scope, and $F$ states
freshness. Absence is justified only by

$ h = 0 and C and F. $

Lean checks:

- no hit without complete scope does not prove absence;
- stale complete no-hit does not prove absence;
- fresh complete no-hit does prove absence.

These correspond to
`no_hit_without_scope_does_not_prove_absence`,
`stale_no_hit_does_not_prove_absence`, and
`fresh_complete_no_hit_proves_absence`.

= Closure authority

Closure evidence is the conjunction

$ op("Closed")(E) =
  op("discharged")(E)
  and op("contradictionFree")(E)
  and op("fresh")(E)
  and op("exactSatisfied")(E). $

Rank does not occur in this proposition. Consequently no rank value can repair
missing discharge or exact materialization. Lean checks the finite witness in
`high_rank_cannot_replace_closure_evidence`.

This formalizes the RFC authority split:

- postings retrieve candidate facts,
- EvidenceGraph represents admitted graph-dependent evidence,
- the closure validator decides the terminal proposition.

= Selector preservation

For an exact materialization request

$ M = (s_b, s_e), $

$s_b$ is the selector bound by the clause/graph handoff and $s_e$ is the
selector emitted to exact query. A valid handoff requires

$ s_b = s_e. $

`selector_drift_blocks_materialization` proves that unequal selector identities
cannot satisfy the request.

= Mandatory graph escalation

The first model makes escalation mandatory for:

- enumeration,
- absence,
- cross-namespace joins,
- contradiction,
- unresolved-variable count above the local limit,
- hop depth above the local limit.

The predicate deliberately excludes rank. Lean therefore proves
`rank_cannot_disable_mandatory_escalation`: changing rank cannot change a
proof-mandatory escalation result.

= Cache identity

A clause cache key contains the clause digest, validation domain, binding
digest, and query-pack digest. Provider drift changes the key:

$ p != p' quad "implies" quad K_c(p) != K_c(p'). $

Lean checks this in `provider_drift_changes_cache_identity`. This blocks reuse
of evidence produced under a different provider authority.

= RFC defects discovered by the model

#table(
  columns: (auto, 1fr, 1fr),
  inset: 5pt,
  [*Defect*], [*Counterexample*], [*RFC repair*],
  [Overlapping clause states],
  [`resolved(c)` and `searchable(c)` both true],
  [Define ordered classification],
  [Impossible fairness],
  [$b = 0, m = 1$],
  [Return typed `needsBudget`],
  [Snapshot-independent search-once],
  [$s != s'$ but clause text unchanged],
  [Include validation domain in action identity],
  [No-hit treated as absence],
  [$h = 0$ with incomplete or stale scope],
  [Require complete fresh scope],
  [Rank treated as closure],
  [High rank with missing discharge/materialization],
  [Keep rank outside closure proposition],
)

= Proof status and next layer

The first Lean file checks finite counterexamples and repaired safety lemmas for
representation identity, state classification, budget disposition, freshness,
absence, closure, selector handoff, escalation, and cache identity.

The next proof layer should add:

1. qualified n-ary fact reconstruction,
2. AND/OR obligation sufficiency witnesses,
3. clause-step preservation,
4. cycle and bounded-progress measures,
5. graph projection refinement,
6. deterministic concurrent projection,
7. advisory improvement-policy refinement.

Any failed theorem or new finite witness updates the RFC clause/risk ledger
before its statement is weakened.
