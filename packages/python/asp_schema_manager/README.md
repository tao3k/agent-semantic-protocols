<!--
SPDX-FileCopyrightText: Contributors to Agent Semantic Protocols
SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only
-->

# ASP Schema Audit

The Python `asp-schema-manager` package is a read-only audit consumer of the
repository JSON Schema registry and Rust Schema Manager receipts.  The Rust
`agent-semantic-schema-manager` crate and `asp schema materialize|verify` are the
only schema bundle lifecycle authority.  Python never rewrites, copies, or
deletes language-package schemas.  It reports reusable validation fragments, direct and transitive
Rust use, unresolved references, and lifecycle recommendations.

Schema families are contractual filename namespaces declared once in the Rust
Schema Manager registry at `schemas/language-schema-profiles.json`. Filename prefixes discover membership, while the
stable `familyId`, precedence, owner, and parent relationship remain explicit
registry policy. Python consumes that registry read-only and owns no parallel
family policy. Cross-family reference candidates are never assigned an
implicit definition owner.
Accepted cross-family extraction or intentional-duplication decisions live in
the same canonical registry; Python does not own a package-local decision file.

```sh
uv run --project packages/python asp-schema-manager audit --workspace-root .
uv run --project packages/python asp-schema-manager check --workspace-root .
# Opt-in 0-drift gates for references and family classification.
uv run --project packages/python asp-schema-manager check --workspace-root . --fail-on-family-local-refs --fail-on-unclassified-schemas --fail-on-mixed-family-refs --fail-on-reference-decision-drift
uv run --project packages/python asp-schema-manager families --workspace-root .
# Select one or more registered schemas for a deterministic proof plan.
uv run --project packages/python asp-schema-manager proof-plan --workspace-root . --schema schemas/semantic-proof-obligation.v1.schema.json --json
# Omit --schema to project the full registered schema catalog.
uv run --project packages/python asp-schema-manager proof-plan --workspace-root . --json
```

The opt-in gates emit an error and exit nonzero for family-local reference
opportunities, unclassified schemas, or mixed-family reference opportunities.
Cross-family candidates are governed by the canonical registry's
`referenceDecisions`; the final gate fails when a candidate
lacks an accepted decision or its fingerprint, occurrence-set digest, or family
set drifts.

The lifecycle manifest is explicit policy. An absent Rust string is evidence,
not deletion authority; unreferenced schemas become reviewable removal
candidates and still require a manifest decision before deletion.

## Logical proof plans

The `proof-plan` command is deterministic and read-only. For each selected
registered schema it emits the canonical source-content digest, transitive
resolved-reference closure and closure digest, family and lifecycle state, and
supported versus unsupported JSON Schema keyword obligations. Supplying
`--expected-plan-digest` turns a digest mismatch into a typed `stale` result and
a nonzero exit status.

Schema Manager is the projection producer, not a proof authority. The plan
reuses `semantic-proof-definitions.v1.schema.json#/$defs/schemaProjection` and
emits open obligations conforming to `semantic-proof-obligation.v1.schema.json`.
It binds the existing `semantic-proof-recipe.v1.schema.json`,
`lean-org-typst-proof-bundle-index.v1.schema.json`, and
`semantic-proof-receipt.v1.schema.json` contracts for later Axle/Lean
materialization and verification. It does not create private
`asp-schema-proof-*` contracts or modify the management report v1 shape.
