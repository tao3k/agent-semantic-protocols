# Gerbil POO Extension Harness Design

## Status

This document defines the design for turning `.data/gerbil-poo` into the
reference corpus for a higher-quality Gerbil Scheme project harness extension
system. It is a design artifact only. Implementation must not begin until this
spec is reviewed and the next implementation plan is approved.

## Context

The repository already has a Gerbil Scheme provider with extension, parser,
search, and schema surfaces. The current POO support is useful but still mostly
provider-local:

- `languages/gerbil-scheme-language-project-harness/src/extensions/poo.ss`
  owns POO extension activation, direct pattern evidence, and several large
  hand-written pattern specs.
- `languages/gerbil-scheme-language-project-harness/src/parser/poo.ss`
  extracts parser-owned facts for `defclass`, `.defclass`, `defmethod`,
  `.defmethod`, `defgeneric`, `.defgeneric`, `defprotocol`, `.defprotocol`,
  and `.def`.
- `languages/gerbil-scheme-language-project-harness/src/extensions/model.ss`
  defines the current package extension fact model.
- `schemas/semantic-extension-pattern-mapping.v1.schema.json` and
  `schemas/semantic-search-packet.v1.schema.json` already provide shared
  contract surfaces, with matching package-local copies under the Gerbil
  harness.

The `.data/gerbil-poo` corpus provides the missing real-project reference. Its
core shape is:

- `proto.ss`: small function-level prototype combinators:
  `instantiate-proto`, `identity-proto`, `compose-proto`,
  `compose-proto*`, and `instantiate-protos`.
- `object.ss`: the object kernel: slots, defaults, C3 precedence lists, lazy
  slot computation, `.ref`, `.has?`, `.all-slots`, and slot spec normalization.
- `mop.ss`: meta-object layer: `.defgeneric`, generic dispatch through slots,
  `define-type`, runtime type descriptors, JSON/sexp conversion hooks, and
  validation.
- `type.ss`: richer type and serialization descriptors built on the MOP.
- `fq.ss`: a representative consumer that uses `define-type`, POO objects, and
  type-level operations for finite-field code.

## Problem

The harness currently knows POO mostly as pre-authored pattern evidence. That
creates several quality problems:

1. Pattern specs repeat the same selector, minimal-form, failure-case, witness,
   and steering shapes.
2. The parser has only shallow POO form facts and does not yet expose the
   richer prototype, slot, type, and method contract facts that agents need.
3. POO output is not yet strongly tied to a real reference corpus, so the
   harness can describe best practices without proving that they map to a real
   project.
4. Schema and search surfaces can drift if provider-local fields grow before
   shared contracts are explicit.

The target is not a Gerbil POO runtime clone. The target is a project harness
that emits compact, schema-visible, parser-owned evidence that helps agents
write better Gerbil POO code and avoid generic Scheme or Racket object-system
guesses.

## Goals

1. Make this repository the best-practice home for agent-facing Gerbil POO
   harness evidence.
2. Treat `.data/gerbil-poo` as a reference corpus and test fixture, not as a
   vendored runtime dependency.
3. Reduce duplicated POO pattern code by modeling pattern families as base
   specs plus composable overlays.
4. Upgrade parser facts so POO evidence is produced from syntax and reference
   selectors, not just hand-written prose.
5. Keep shared schema changes contract-first and synchronized between root
   `schemas/` and the Gerbil harness local schema copies.
6. Preserve compact search output and avoid raw source dumps in agent-facing
   workflows.

## Non-Goals

- Do not import or execute `.data/gerbil-poo` as provider runtime code.
- Do not implement a full POO interpreter, type checker, or macro expander.
- Do not make POO fields required for non-Gerbil providers.
- Do not replace the existing search packet envelope.
- Do not introduce hidden provider-private string heuristics that cannot be
  represented in schema, parser facts, or tests.

## Recommended Approach

Use a staged "reference corpus plus overlays" design.

The POO extension keeps one activation boundary, but pattern evidence is split
into reusable spec components. A base `object-system` spec owns common source
selectors, minimal forms, failure cases, quality signals, and agent steering.
Specialized families such as `prototype-composition`, `slot-cache`,
`type-validation`, `io-json-fallback`, `c3-mro`, `lens`, and
`dependency-protocol-adapter` become overlays that add or replace the smallest
necessary fields.

This mirrors the POO prototype idea without embedding POO runtime semantics in
the harness. In POO, a prototype is composed into an instance. In the harness,
a pattern overlay is composed into a stable extension pattern mapping packet.

## Architecture

### Reference Corpus Layer

`.data/gerbil-poo` remains outside the provider runtime. The harness should use
it for:

- source selector fixtures, such as `gerbil-poo://proto.ss#compose-proto`;
- parser fixture coverage for `define-type`, `.defgeneric`, `defmethod`,
  `.o`, `.def/ctx`, slot specs, and `@method`;
- real-project scenario evidence from `fq.ss`;
- regression snapshots that prove the provider can guide an agent from search
  terms to correct Gerbil POO forms.

The corpus can be refreshed independently from the harness implementation, but
tests must pin expected selectors and evidence packets so changes are reviewed.

### Shared Contract Layer

Before provider behavior changes, update the search RFC and shared schema
contract. The expected contract remains additive in v1 unless a breaking shape
change is required.

The contract should make these concepts explicit:

- extension activation proof;
- pattern mapping identity;
- source reference and source selector authority;
- minimal form mapping;
- parser-derived form facts;
- failure cases;
- quality signals;
- witness and missing evidence;
- next action guidance.

Root schemas remain the source of truth. Gerbil package-local schema copies are
kept byte-for-byte aligned with root schemas, and schema tests should catch
drift.

### Provider Model Layer

Introduce a small provider-owned model for POO pattern specs. The model should
separate:

- spec identity: id, extension, focus, origin;
- activation and source reference;
- source owners and selectors;
- minimal forms;
- failure cases;
- quality signals;
- witness, missing, and next guidance;
- overlay composition rules.

The model does not need a general object system. It needs deterministic
composition over hashes/lists with explicit conflict rules:

- scalar fields use overlay replacement;
- list fields append and dedupe by stable keys;
- required fields must be present after composition;
- unknown fields fail in tests unless the schema explicitly permits them.

### Parser Fact Layer

Extend `src/parser/poo.ss` in small slices. The first parser expansion should
cover the forms that appear in `.data/gerbil-poo` and current pattern specs:

- `define-type`;
- `.defgeneric`;
- `defgeneric`;
- `defmethod`;
- `.defmethod`;
- `@method` receiver shape;
- `.o` and `.def/ctx` object forms;
- slot specs such as constants, inherited computations, computed slots,
  default slots, and mixin overrides;
- prototype composition calls: `compose-proto`, `compose-proto*`,
  `instantiate-proto`, and `instantiate-protos`.

Each parser fact should preserve role, symbol, owner path, source location, and
provider-owned fields. Parser facts should drive evidence where possible; static
pattern specs remain the fallback for conceptual guidance.

### Search And Rendering Layer

Search output should continue to be compact by default. For POO-related
queries, the provider should prefer this flow:

1. detect extension activation from `gerbil.pkg`;
2. classify the query into a pattern family;
3. compose the base pattern spec with the selected overlay;
4. enrich the packet with parser facts when source evidence exists;
5. emit schema-valid pattern mapping evidence;
6. include explicit missing evidence when the corpus or parser cannot prove a
   claim.

Agent-facing text must emphasize selectors, minimal forms, failure cases, and
next actions. It should not dump source.

## Data Flow

```text
gerbil.pkg dependency
  -> extension activation fact
  -> POO query classification
  -> base object-system spec
  -> selected pattern overlay
  -> parser facts from project and reference corpus
  -> semantic-extension-pattern-mapping packet
  -> compact search lines and JSON packet
```

## Implementation Slices

### Slice 1: Contract And RFC

Update the search RFC before provider implementation. The RFC should define how
extension pattern mappings enter search output, how parser facts enrich them,
and how missing evidence is reported.

Then update the shared schema contract if needed and sync the Gerbil local copy.
The first schema pass should be additive and validate existing packets.

### Slice 2: POO Spec Composition Model

Split the monolithic pattern spec construction in `src/extensions/poo.ss` into
a small model plus data definitions. The first refactor should preserve output
shape exactly, proving that the overlay model reduces duplication without
changing behavior.

Suggested boundaries:

- `src/extensions/poo/model.ss`: pattern spec and overlay composition helpers.
- `src/extensions/poo/specs.ss`: base spec plus overlay data.
- `src/extensions/poo.ss`: activation, dispatch, and public facade.

If the local module layout makes this split awkward, keep the same logical
boundaries inside fewer files for the first pass.

### Slice 3: Parser Facts

Add parser-owned facts for one family at a time. Start with
`prototype-composition` because `.data/gerbil-poo/proto.ss` is compact and the
expected selectors are clear. Then add `define-type` and slot spec facts using
`mop.ss`, `type.ss`, and `object.ss`.

Each parser slice should include snapshot or unit coverage before another form
family is added.

### Slice 4: Reference Corpus Evidence

Use `.data/gerbil-poo/fq.ss` as the real-project scenario. The evidence should
prove that the agent can discover `define-type`, type descriptors, prototype
composition, and object-system selectors without raw source reading.

### Slice 5: Quality Gates

Add tests for:

- root/local schema drift;
- schema-valid pattern mapping packets;
- compact search output for POO extension and pattern queries;
- parser facts for each supported POO form family;
- regression snapshots for `prototype-composition`, `slot-cache`,
  `type-validation`, and `object-system`;
- failure cases that prevent Racket class syntax, inactive extension usage,
  method-without-generic mistakes, and unchecked MRO assumptions.

## Error Handling

The provider should fail closed for malformed internal pattern specs during
tests. At runtime, user-facing search should prefer partial evidence packets
with explicit `missing` entries over silent omission.

Examples:

- inactive POO dependency: emit extension missing evidence and next action to
  inspect `gerbil.pkg`;
- unknown pattern family: fall back to `object-system` only when the query
  still clearly targets POO;
- selector unavailable in the reference corpus: preserve the pattern mapping
  but mark the selector witness missing;
- parser fact unsupported: emit static guidance and record the parser gap.

## Validation Strategy

Fast validation should stay provider-scoped:

```sh
just check-gerbil
just test-gerbil
```

If these aliases are unavailable or too broad, use the Gerbil harness test
entrypoints directly. Schema validation must include both root schemas and the
Gerbil package-local schema copies.

Search validation should include compact agent-facing commands:

```sh
asp gerbil-scheme search extension poo --workspace . --view seeds
asp gerbil-scheme search pattern poo prototype --workspace . --view seeds
asp gerbil-scheme search pipe 'gerbil poo prototype composition' --workspace . --view seeds
```

The success condition is not just passing JSON schema. The packet must include
usable selectors, minimal forms, failure cases, witness or missing evidence, and
clear next actions.

## Migration Rules

1. RFC before schema behavior changes.
2. Shared schema before provider-specific output changes.
3. Root schema before package-local schema copy.
4. Parser facts before search output depends on syntax-specific claims.
5. Reference corpus selectors before declaring best-practice evidence complete.
6. Snapshot tests before behavior-preserving refactors are accepted.

## Implementation Defaults

1. Keep the first behavior-preserving refactor inside `src/extensions/poo.ss`
   unless the patch becomes harder to review than a file split. Split into
   `src/extensions/poo/model.ss` and `src/extensions/poo/specs.ss` only when
   tests can prove output stability before and after the split.
2. Keep `semantic-extension-pattern-mapping.v1` for the first pass and use only
   additive optional fields. Create a v2 schema only if parser-enriched pattern
   evidence requires a breaking required-field, enum, or nesting change.
3. Treat `.data/gerbil-poo` as a pinned fixture copy for this design. Any
   refresh should be a separate reviewed change with a short update receipt,
   selector drift report, and snapshot updates.

## First Experiment After Approval

The first experiment should be behavior-preserving:

1. Extract the current POO pattern specs into a base spec plus overlays.
2. Keep emitted compact output and JSON packet shape unchanged.
3. Add regression tests proving no packet drift.
4. Measure code reduction and readability before adding any new parser facts.

This gives a low-risk proof that the P/prototype composition idea improves the
harness engineering model before the provider starts emitting new facts.
