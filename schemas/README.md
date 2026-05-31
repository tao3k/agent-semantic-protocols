# Semantic Search Schemas

`semantic-language-registry.v1.schema.json` is the language-server-style
provider registry. It records the semantic language protocol, language ids,
provider ids, executable binaries, callable methods, structured method
descriptors, and packet schemas.

`semantic-search-packet.v1.schema.json` is the shared JSON contract for search
output across semantic language providers. Compact text stays the default
prompt surface; JSON is the validation, cache, and artifact shape.

The TypeScript provider registers as:

```json
{
  "languageId": "typescript",
  "providerId": "ts-harness",
  "binary": "ts-harness",
  "namespace": "agent.semantic-protocols.languages.typescript.ts-harness",
  "methods": ["search/workspace", "search/prime", "check/full", "agent/doctor"],
  "methodDescriptors": [
    {
      "method": "search/workspace",
      "command": "search",
      "view": "workspace",
      "outputSchemaIds": ["agent.semantic-protocols.semantic-search-packet"],
      "requiresQuery": false,
      "acceptsStdin": false,
      "supportsPackageScope": true,
      "supportsJson": true,
      "supportsCompact": true
    }
  ]
}
```

`ts-harness` is the binary/provider name. The protocol namespace is
`agent.semantic-protocols.semantic-language`; the registry is
`agent.semantic-protocols.semantic-language-registry`. The provider namespace
is the stable method space for a concrete implementation.

`methods` is the authoritative callable set for a provider. The shared search
packet schema may list additional cross-language views, but an agent should
only call methods present in the provider registry. `methodDescriptors` adds
machine-readable command/view/schema metadata for each method.
Search descriptors must include a `view` and emitted `outputSchemaIds`; check
descriptors intentionally do not advertise a search view; agent descriptors can
point at registry output schemas such as
`agent.semantic-protocols.semantic-language-registry`.
For search methods, `requiresQuery`, `acceptsStdin`, and `supportsPackageScope`
are provider-owned CLI semantics and should be consumed by command parsers
instead of being duplicated in separate hard-coded view lists.

Registry invariants mirror Language Server Protocol naming discipline without
copying LSP transport. `languageId` identifies the source language,
`providerId` identifies the implementation, `binary` is the executable an
agent should invoke, and `namespace` is always
`agent.semantic-protocols.languages.<languageId>.<providerId>`. Compatibility
binary aliases are not registry identities. A provider must publish exactly one
descriptor for every method in `methods`, no extra descriptors, and no duplicate
descriptor methods.

The stable envelope is language-neutral:

- `schemaId`: `agent.semantic-protocols.semantic-search-packet`
- `schemaVersion`: `1`
- `protocolId`: `agent.semantic-protocols.semantic-language`
- `protocolVersion`: `1`
- `languageId`: source language id, such as `typescript`, `rust`, `julia`, or
  `python`
- `providerId`: provider id, such as `ts-harness`, `rs-harness`, or
  `jl-harness`
- `binary`: executable entrypoint advertised by the provider
- `namespace`: dot-qualified provider namespace, such as
  `agent.semantic-protocols.languages.typescript.ts-harness`
- `method`: namespaced method, such as `search/prime`, `search/dependency`,
  or `search/deps`
- `view`: one semantic-search view, such as `workspace`, `prime`, `owner`,
  `dependency`, `deps`, `symbol`, `callsite`, `import`, `cfg`,
  `patterns`, `pattern`, `docs`, `api`, `public-external-types`, `tests`,
  `text`, or `ingest`
- `header`, `packages`, `nodes`, `edges`, `owners`, `items`, `hits`,
  `findings`, `nextActions`, and `notes`
- optional `inputDetection` for stdin-derived searches

Language harnesses should preserve compiler-native facts in `fields` maps
instead of changing the envelope. For example, Rust can place Cargo feature
facts in `fields`, TypeScript can place owner import summaries in `fields`, and
Julia can place JuliaSyntax-native module facts in `fields`.

Dependency API searches should distinguish the current workspace resolution
from an explicitly requested external version. Providers can use fields such as
`requestedVersion`, `versionScope`, `currentWorkspaceVersion`, and `apiQuery`;
local usage should only be attributed when `versionScope` is `current`. When
`versionScope` is `external`, owner evidence belongs to the workspace version
and must not be presented as evidence for the requested external version.

This repository's `schemas/` directory is the protocol source of truth.
Provider packages that run CI from independent checkouts should carry
package-local copies at the same relative paths, for example
`schemas/semantic-search-packet.v1.schema.json`. The TypeScript harness unit
suite reads its package-local copies, validates every implemented
`ts-harness search ... --json` view against the shared envelope, checks
`ts-harness agent doctor --json` against the registry contract, and compares
the package-local copies with this repository's source schemas when the package
is checked out as a submodule.
The Rust harness exposes the same registry contract through
`rs-harness agent doctor --json`.

The current TypeScript slice emits conforming packets from:

```shell
ts-harness search workspace --json .
ts-harness search prime --package packages/core --json .
ts-harness search prime --json .
ts-harness search owner src/index.ts --json .
ts-harness search dependency react --json .
ts-harness search deps react/jsx-runtime@19.0.0::jsx --json .
ts-harness search symbol OrderStatus --json .
ts-harness search callsite OrderStatus --json .
ts-harness search import ./order --json .
ts-harness search tests src/domain/order.ts --json .
ts-harness search text OrderStatus --json .
rg -n "OrderStatus" src tests | ts-harness search ingest --json .
```

The Rust slice emits the same envelope from `rs-harness search ... --json`,
including Cargo, owner, dependency, symbol, callsite, import, cfg, pattern,
docs, api, public-external-types, tests, and ingest views.
