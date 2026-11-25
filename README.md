<!--
SPDX-FileCopyrightText: 2026 tao3k team and Contributors
SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later
-->

# Agent Semantic Protocols

Shared protocol contracts, resident semantic runtime, language provider
supervision, and replay sandtables for semantic ASP language providers integrated
under the Marlin (`marlin-agent-core`) host system.

This repository keeps the semantic search, exact query, AST patch verification,
and resident workspace generation surfaces stable across Rust, TypeScript,
Python, Julia, Gerbil Scheme, Org, and Markdown providers. ASP language providers
own parser facts and provider-specific analysis; Marlin owns hooks, session
control, and multi-agent orchestration; this repository owns the shared schemas,
RFCs, typed ClientFrame/gRPC runtime server, and scenario evidence.

## What Lives Here

- `docs/10-19-rfcs/`: protocol intent and change process. Start here before
  changing search, query, schema, or Marlin host integration behavior.
- `schemas/`: shared JSON contracts for search/query packets, language
  registries, provider manifests, Marlin host integration bindings, sandtable
  scenarios, and receipts.
- `crates/agent-semantic-client*/` and `crates/agent-semantic-runtime*`: Rust
  client CLI (`asp`), typed ClientFrame/gRPC protocol crates, Turso/Tantivy
  workspace generation store, and resident Runtime Server.
- `packages/python/tools/src/tools/semantic_sandtable/`: sandtable runner for
  replaying real provider commands and validating scenario evidence.
- `sandtables/`: cross-language replay scenarios for search flows, performance
  budgets, and real-trigger regressions.
- `languages/`: language provider worktrees used by the protocol repo for
  alignment and integration tests.

## Workflow

For semantic search protocol changes, use the repo workflow in `AGENTS.md`:

```text
RFC -> shared schema -> language alignment -> Rust implementation/tests ->
sandtable alignment -> real-project evidence -> optimization loop
```

Do not jump from a real-project pain point directly to a provider-private fix
when the issue changes the shared contract. Keep the durable behavior visible
in the RFC and schema first, then align providers and sandtable evidence.

## Documentation Map

- `docs/10-19-rfcs/10.34-marlin-host-integration-protocol.org` owns the Marlin
  host integration boundary, typed host context/binding contracts, and the
  explicit ownership split between Marlin (hooks, sessions, multi-agent graph)
  and ASP (semantic search/query playbooks, resident generations, providers).
- `docs/10-19-rfcs/10.11-semantic-tree-sitter-query-protocol.org` owns the
  portable tree-sitter-compatible syntax ABI, catalog/profile/corpus layout,
  native projection boundary, and pattern-graph roadmap.
- `docs/10-19-rfcs/10.05-interactive-graph-first-progressive-searchloop.org`
  owns the server-first agent search architecture; `asp <language> ...` is a
  typed ASP Server client surface, search/query/read-plan stdout contracts, and
  syntax locate/code flows.
- `schemas/README.md` owns the schema catalog and explains how query/search/read
  packets share tree-sitter provenance without merging packet envelopes.
- `docs/30-39-research/31.18-tree-sitter-query-rfc-roadmap.org` records the
  current tree-sitter native-projection audit and the closure checklist for the
  RFC/docs pass.

## Common Commands

Run repository commands through the captured devenv profile. This reuses the
already-activated workspace without evaluating `direnv` for every command:

```sh
.devenv/devenv-profile-exec <command>
```

Install the core ASP runtime binary and all language providers from this
checkout into `$ASP_STATE_HOME` (`~/.agent-semantic-protocols`) and expose the
canonical `~/.local/bin/asp` entrypoint:

```sh
just install
```

Install individual tool boundaries when only one component changed:

```sh
just agent-tools-install-client
just agent-tools-install-rust
just agent-tools-install-typescript
just agent-tools-install-python
just agent-tools-install-julia
just agent-tools-install-gerbil
```

Install released language provider artifacts into the current project or State Home:

```sh
asp install language rust --rev v0.1.0 --project .
asp install language julia --rev v0.1.0 --target aarch64-apple-darwin --project .
```

Use the Runtime-owned Search Playbook and Query Playbook for code and document
discovery and exact structural reads:

```sh
asp guide
asp doctor
asp providers
asp cache status
asp search playbook '(search (producers (language rust)) (intersect (rg "-n" "-e" "<term>") (tantivy "title:<term>^2 OR body:<term>")))'
asp query playbook '(query (producers (language rust)) (select (selectors "rust://src/lib.rs#item/function/run") (projection source)))'
asp query playbook '(query (producers (documents org)) (select (selectors "org://docs/spec.org#item/heading/Contract") (projection source)))'
```

`asp search playbook` and `asp query playbook` are the sole public Search and
Query surfaces. Use `(language rust typescript python julia gerbil-scheme)` for
code producers and `(documents org md)` for document producers; a cross-domain
request may use both. Search accepts one Scheme composition with top-level
Workspace scope; an `rg` leaf never carries a path. Query accepts only exact
canonical selectors returned on owning GQL nodes.

Verify a compact AST patch intent without enabling client-side mutation:

```sh
asp ast-patch verify --packet ast-patch.json .
asp ast-patch dry-run --packet ast-patch.json .
```

Run RFC/docs, schema, and policy checks:

```sh
just check-rfc-docs
just check-schema-profiles
just check-schema-manager
just check-rust-workspace-policy
just check-license-contract
```

## Notes For Agents

- Prefer compact provider search output over raw source reads.
- Treat `asp` as the public shared CLI and resident Runtime Server integrated
  with Marlin; hook interception, session control, and multi-agent routing belong
  exclusively to Marlin (`marlin-agent-core`).
- Use sandtable receipts and real-trigger scenarios to prove that a workflow
  saves commands, bytes, latency, or repeated searches.
- Keep README short. Put design detail in `docs/10-19-rfcs/`,
  `schemas/README.md`, or `docs/`.
