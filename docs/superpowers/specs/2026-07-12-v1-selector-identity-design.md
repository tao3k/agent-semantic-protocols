# V1 Selector Identity and Resource-Stability Design

## Status

Approved design. The public selector contract remains v1 indefinitely.

## Problem

Providers can emit a structured selector that their own query resolver cannot
resolve. The Gerbil `export/...` failure is one observed instance. This is an
identity-contract failure: display-oriented selector segments and index-owned
item identities have diverged.

An unresolved selector is not itself a memory leak. It becomes a leak
amplifier when a no-hit repeatedly enters parse, index-refresh, fallback, or
retry paths that retain snapshots, syntax trees, ports, subprocess handles, or
unbounded failed-selector state.

## Goals

- Preserve all existing v1 selector inputs and response envelopes.
- Make every emitted v1 selector resolvable in the same index snapshot, or
  return a deterministic v1 identity-drift receipt.
- Give Rust, TypeScript, Python, Julia, Gerbil Scheme, Org, and Markdown the
  same selector-resolution and resource-lifecycle semantics.
- Make repeated invalid selector requests bounded in retained memory and work.
- Do not perform implicit source scans or index rebuilds after a selector
  no-hit.

## Non-goals

- A public v2 selector syntax or a forced caller migration.
- Provider-specific compatibility parsing as the durable solution.
- Hiding a no-hit by guessing a nearby source item.

## Public V1 Contract

The existing `selector` string remains the request key and retains its current
v1 schema identity. Responses may add these optional fields without changing
the v1 envelope:

- `resolution`: `exact`, `alias`, `identity-drift`, or `not-found`.
- `canonicalIdentity`: an opaque provider-owned stable item identifier.
- `indexGeneration`: the immutable snapshot generation used for resolution.
- `selectorAliases`: compact, bounded aliases only when useful for repair.

`identity-drift` means the selector shape is recognized but it cannot map to
an item in the selected generation. `not_found` means the selector is valid
but no corresponding item exists. Both are terminal query results: neither may
trigger reindexing or source fallback.

## Internal Identity Model

Each provider creates an index generation containing:

1. A canonical identity table keyed by opaque `canonicalIdentity` values.
2. A v1 selector table mapping emitted selector strings to canonical identity.
3. A bounded alias table mapping accepted legacy/display/export forms to the
   same canonical identity.

The query resolver first chooses the requested immutable generation, then
normalizes the v1 selector, resolves an exact key or bounded alias, and only
then loads the compact item projection. It never derives identity from a
display line range or reparses the workspace to repair a miss.

Export selectors are aliases for a canonical exported binding, not standalone
index identities. A provider must emit `export/...` only when its alias table
can resolve that form in the current generation.

## Resource and Failure Semantics

- Selector normalization and map lookup happen before expensive parsing.
- Any transient parse tree, source snapshot, port, FFI handle, child process,
  or callback registration is released on success, no-hit, cancellation, and
  error through one provider-owned cleanup boundary.
- Negative cache entries use `(normalizedV1Selector, indexGeneration)` and are
  capacity-bounded; generation retirement drops all of its entries.
- Retry and diagnostic paths receive the terminal receipt rather than retaining
  failed query contexts.
- A provider may refresh an index only through an explicit refresh/ingest
  action, never as a side effect of `identity_drift` or `not_found`.

## Provider Conformance

Every provider descriptor declares selector support and emits the same v1
resolution receipt. Provider adapters implement only two operations:

1. `emit_selector_aliases(canonical_item, generation)` during indexing.
2. `resolve_v1_selector(selector, generation)` during querying.

Shared schema validation owns the envelope, resolution values, generation
provenance, and bounded diagnostic fields. Provider implementations own AST
and language-binding extraction.

## Verification Gates

For each supported language and index generation:

1. **Round trip:** every emitted v1 selector resolves to the originating
   canonical identity in that generation.
2. **Alias conformance:** accepted export/display aliases resolve to the same
   canonical identity, never a nearby item.
3. **Terminal no-hit:** malformed, stale, and unknown selectors return
   deterministic v1 receipts and do not change index generation.
4. **Resource stability:** repeated invalid selector workloads show bounded
   retained memory, syntax-tree roots, open ports/handles, child processes,
   callbacks, and negative-cache entries.
5. **Cross-provider parity:** Rust, TypeScript, Python, Julia, Gerbil Scheme,
   Org, and Markdown produce schema-valid v1 receipts for the same fixture
   categories.

## Rollout

1. Add the additive v1 receipt fields and shared validator.
2. Implement canonical identity and selector tables in each provider, starting
   with the Gerbil export gap as the reference fixture.
3. Enable round-trip and terminal-no-hit tests for every provider.
4. Add memory/resource convergence fixtures to the provider harnesses.
5. Remove any no-hit-triggered rebuild, source fallback, or unbounded failed
   selector cache discovered during migration.

No caller-visible selector format changes occur during rollout.
