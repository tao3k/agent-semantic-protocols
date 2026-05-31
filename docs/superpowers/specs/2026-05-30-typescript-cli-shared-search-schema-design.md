# TypeScript CLI And Shared Semantic Search Schema Design

## Scope

This slice aligns the TypeScript harness CLI with RFC 005 while establishing a
shared semantic-search JSON Schema for future Rust, Julia, Python, and other language harnesses.
Only the TypeScript implementation is changed in this slice. Other languages
get a stable schema contract they can map to later without changing their
current outputs.

## CLI Contract

`ts-harness` is the TypeScript provider binary name. The primary public
protocol becomes:

```shell
ts-harness search <view> ... [--json] [--package PATH] [PROJECT_ROOT]
ts-harness check [--changed | --full] [--json] [PROJECT_ROOT]
ts-harness agent doctor [--json] [PROJECT_ROOT]
```

The protocol identity is the semantic language registry, not the binary:
`languageId=typescript`, `providerId=ts-harness`, `binary=ts-harness`, and
`namespace=agent.semantic-protocols.languages.typescript.ts-harness`.
Agent-facing commands use the generic `agent` command namespace rather than
naming one agent platform.

The first implemented search views are deterministic primitives, not intent
parsers. Compact text remains the default agent-facing output. `--json` emits a
structured packet conforming to the shared semantic-search schema.

## Shared Schema

Add `schemas/semantic-language-registry.v1.schema.json` as the
language-server-style provider registry. It standardizes `languageId`,
`providerId`, `binary`, provider `namespace`, method names such as
`search/dependency`, `search/deps`, structured `methodDescriptors`, and the
packet schemas implemented by each provider. The registry's `methods` list is
the callable truth; the shared packet schema's cross-language view enum is not
a capability advertisement. Search descriptors also carry CLI input semantics
such as `requiresQuery`, `acceptsStdin`, and `supportsPackageScope`.
The TypeScript provider treats this registry as the single source of CLI search
view metadata: `methodDescriptors` and `methods` must be the same set, search
descriptors use `view` equal to the `search/<view>` suffix, and the public
semantic-language identity is exactly the advertised provider registration.

Add `schemas/semantic-search-packet.v1.schema.json` as a repo-level contract for
semantic search packets. The schema models:

- packet identity: schema id/version, protocol id/version, language id,
  provider id, binary, namespace, method, project root, view, render mode
- bounded search facts: packages, owners, items, hits, graph nodes, graph edges
- finding groups, notes, and next actions
- optional ingest detection metadata for stdin-derived searches

The schema uses language-neutral field names and permits language-specific
structured `fields` maps. That keeps the stable packet shape shared while
allowing each harness to preserve parser-native facts.

## TypeScript Mapping

The TypeScript CLI builds packets from the existing `TypeScriptHarnessReport`
and `TypeScriptReasoningTree`.

- `search workspace` renders a workspace package/router index without raw hits.
- `search prime` renders a bounded package-level owner/dependency/finding map;
  `--package <path>` selects a workspace package after `search workspace`.
- `search owner <path-or-owner>` renders one owner plus adjacent edges.
- `search dependency <query>` renders parser-owned `package.json` dependency
  facts plus TypeScript import-resolution usage for an external package.
- `search deps <dep[/subpath][@version][::api]>` renders version-aware
  dependency API usage: manifest range, current `currentWorkspaceVersion` when
  available, local import usage, and explicit `versionScope`.
  If the requested version is not the current workspace resolution, the packet
  is an external-version query and local import usage is not attributed to it.
- `search symbol <query>` renders exported symbol definitions.
- `search callsite <query>` renders owner-level import/reexport sites for
  matching exported symbol owners.
- `search import <query>` renders import/reexport owner edges grouped by owner.
- `search tests <owner-or-path>` renders test owners that import a source owner.
- `search text <query>` renders owner-grouped path/export matches.
- `search ingest` detects stdin shape (`rg -n`, vimgrep, path list,
  diff paths, JSONL, or unknown) and returns owner-grouped hits.
- `check` delegates to the existing compact harness renderer or JSON report.
- `agent doctor` reports local CLI/schema readiness only; it does not install
  hooks in this slice.

## Validation

TypeScript unit tests cover the new CLI grammar, compact line prefixes, JSON
packet shape, direct output flags, and ingest detection. The closeout gate is
split into implementation and policy lanes: `npm run check:implementation`,
`npm run check:policy`, `npm run test:implementation`, `npm run test:policy`,
and `git diff --check` for the touched repository.
