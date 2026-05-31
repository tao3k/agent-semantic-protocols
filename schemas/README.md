# Semantic Search Schemas

`semantic-language-registry.v1.schema.json` is the language-server-style
provider registry. It records the semantic language protocol, language ids,
provider ids, executable binaries, callable methods, structured method
descriptors, and packet schemas.

`semantic-search-packet.v1.schema.json` is the shared JSON contract for search
output across semantic language providers. Compact text stays the default
prompt surface; JSON is the validation, cache, and artifact shape. Agent-facing
interactive exploration should not request `search ... --json`; hooks should
deny that output-mode error with `reasonKind=agent-search-json` and guide to
the equivalent compact command.

`semantic-sandtable-scenario.v1.schema.json` is the shared scenario descriptor
for replaying bounded search flows against real harness binaries. It owns the
portable drill shape: workdir selection, argv commands, stdin pipe commands,
regex capture handoff, line-protocol expectations, and warning budgets for
token-size and latency findings. Scenario descriptors can also carry compact
real-trigger `evidence` metadata for recorded agent exploration loops, including
the launch intent, edit-stop boundary, receipt path, recorded metrics,
repeated-search findings, and query-set merge opportunities. Hook replay steps
may use `expect.guideQuality` to assert that a denial includes the reason kind,
language route, safe command shape, ingest-pipe guidance, and no leaked source
text.

`semantic-sandtable-receipt.v1.schema.json` is the compact evidence contract
for a real-trigger agent exploration before it is converted into replayable
scenario steps. It records the project, intent, edit boundary, accepted search
commands, hook-deny guide routes, subagent or ingest shapes, per-command
metrics, per-command token cost, output mode, repeated-search findings,
JSON-search misuse counts, summary token cost, and query-set merge
opportunities without embedding source excerpts or full terminal transcripts.
Each `commands[].metrics.tokenCost` records the token cost for that command id;
`summary.tokenCost` is the checked sum across command costs. Both levels must
identify whether the value is an estimate or a measured count and include a
basis string so sandtable evidence is not confused with model billing.
`semantic-sandtable` can validate these receipts directly with
`--receipt <path>`, and scenarios can link a receipt through
`evidence.receiptPath`.

`semantic-agent-hook-profile.v1.schema.json` is the profile registry consumed by
the root `semantic-agent-hook` runtime. It standardizes language-owned source
extensions, config files, ignored path prefixes, route command templates, and
policy toggles without making the hook classifier language-specific.

`semantic-agent-hook-decision.v1.schema.json` is the shared decision packet for
the root hook classifier before it renders a platform-specific Codex or Claude
hook response. It standardizes normalized event names, deny/context decisions,
language/provider routes, and state updates while provider repositories own only
their language profile descriptors and semantic search/check commands.

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
only call methods present in the provider registry. `methodDescriptors` is the
machine-readable command grammar for each method.
Search descriptors must include a `view` and emitted `outputSchemaIds`; check
descriptors intentionally do not advertise a search view; agent descriptors can
point at registry output schemas such as
`agent.semantic-protocols.semantic-language-registry`. Agent hook descriptors
that emit structured decisions must instead advertise
`agent.semantic-protocols.agent-hook-decision`, so providers can render
platform-specific hook payloads without changing the shared decision contract.
For search methods, `requiresQuery`, `acceptsStdin`, and `supportsPackageScope`
define the v1 public input shape: one optional/required query positional, stdin
participation, and `--package <package-id>`. Additional public controls must be
added to the registry schema before agents depend on them. Provider-private
debug flags are not semantic-language protocol methods until they are
registry-described.

Search descriptors may also carry `capabilities` and `ingestRequiredFor`.
The common registry schema only standardizes their shape:
`{languageId, namespace, name}`. It does not maintain TypeScript, Rust, Python,
Julia, or JavaScript capability vocabularies. Language-specific harness
repositories own those schemas under their local `schemas/` directories and may
advertise them through the provider `schemas` list.
`capabilities` is the machine-readable answer to "what can this method search
directly"; `ingestRequiredFor` is the machine-readable answer to "what must be
expanded through `rg`/`fd` or another external source and normalized through
`search ingest`." Agents should consult these fields before interpreting packet
notes or falling back to raw shell output.
Search descriptors can also carry `acceptedPipes`, a provider-advertised list of
final-only pipe names accepted by that method, such as TypeScript's
`search/text` accepting `owner` and `tests`.

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
- optional `querySet` and `queryComposition` for homogeneous same-view
  query-set packets
- optional `inputDetection` for stdin-derived searches

Language harnesses should preserve compiler-native facts in `fields` maps
instead of changing the envelope. For example, Rust can place Cargo feature
facts in `fields`, TypeScript can place owner import summaries in `fields`, and
Julia can place JuliaSyntax-native module facts in `fields`.
Shared `nodes` may also name common search-axis kinds such as `tsconfig`,
`extension`, `build_tool`, and `test_surface` when a language provider exposes
those axes from native project facts.

Structured path fields use the shared `projectPath` definition. A project path
is a canonical project-root-relative path, not a display locator. It must not
include rank prefixes such as `0:`, URI schemes, absolute paths, `..` escapes,
line ranges, or command prefixes such as `owner:`. Put line/column data in
`location`, graph identity in typed node ids such as `O:src/lib.rs`, and ranking
metadata in separate fields.

Dependency API searches should distinguish the current workspace resolution
from an explicitly requested external version. Providers can use fields such as
`requestedVersion`, `versionScope`, `currentWorkspaceVersion`, and `apiQuery`;
local usage should only be attributed when `versionScope` is `current`. When
`versionScope` is `external`, owner evidence belongs to the workspace version
and must not be presented as evidence for the requested external version.

Query-set packets are for repeated same-axis searches, such as multiple
dependencies or multiple owner paths in one package/scope context. Providers
should set `queryComposition.mode` to `query-set`, list normalized terms in
`querySet`, include `queryComposition.scope` when the query-set is owner- or
package-scoped, and advertise support through registry method descriptors.
Descriptor `querySetScopes` uses `project`, `package`, and `owner` to show which
scope forms are accepted. Query sets are not a general command batch surface;
distinct axes should remain separate search packets.

Owner-scoped TypeScript text searches are the motivating case: once
`search owner src/cli/semantic-search/render.ts .` has selected the owner,
repeated text probes such as `location.path`, `location.column`,
`location.line`, and `renderLocation` should become one
`search/text` query-set packet with `scope.ownerPath`, not several separate
text packets or a comma-joined literal query.

This repository's `schemas/` directory is the protocol source of truth.
It contains common protocol schemas only. Provider packages that run CI from
independent checkouts should carry package-local copies of those common schemas
at the same relative paths, for example
`schemas/semantic-search-packet.v1.schema.json`. Language-specific schemas stay
inside the language harness repository, for example the TypeScript provider's
`schemas/typescript-semantic-capabilities.v1.schema.json`. The protocol
repository may keep language-specific templates, such as
`schemas/typescript-semantic-capabilities-template.v1.schema.json` and
`schemas/python-semantic-capabilities-template.v1.schema.json`, to document the
expected active schema shape without making the common registry schema own a
global capability enum. The TypeScript harness unit suite reads its
package-local common schema copies, validates every implemented
`ts-harness search ... --json` view against the shared envelope, checks
`ts-harness agent doctor --json` against the common registry contract, checks
TypeScript descriptor capabilities against the TypeScript-local schema, compares
common package-local copies with this repository's source schemas when the
package is checked out as a submodule, and compares the TypeScript-local
capability vocabulary with the protocol repository template when that template
is available.
The Python harness follows the same ownership split: `py-harness agent doctor
--json` advertises the common registry and search packet schemas plus the
Python-local `schemas/python-semantic-capabilities.v1.schema.json`, while this
repository only keeps the template vocabulary.
The Rust harness exposes the same registry contract through
`rs-harness agent doctor --json`.

Schema evolution is versioned by file name and `schemaVersion`.
Optional fields, enum members, and method descriptors can be additive v1
changes. Renaming a field, changing field meaning, making an optional field
required, or removing an enum member is breaking and requires a new schema file
such as `semantic-search-packet.v2.schema.json`. Provider packages must update
their package-local copies and sync tests in the same change that advertises a
new schema version.

The current TypeScript slice emits conforming packets from:

```shell
ts-harness search workspace --json .
ts-harness search prime --package packages/core --json .
ts-harness search prime --json .
ts-harness search owner src/index.ts --json .
ts-harness search dependency react --json .
ts-harness search deps react/jsx-runtime@19.0.0::jsx --json .
ts-harness search api OrderStatus --json .
ts-harness search public-external-types react --json .
ts-harness search symbol OrderStatus --json .
ts-harness search callsite OrderStatus --json .
ts-harness search import ./order --json .
ts-harness search tests src/domain/order.ts --json .
ts-harness search text OrderStatus --json .
rg -n "OrderStatus" src tests | ts-harness search ingest --json .
```

Those JSON examples are contract checks, not an agent exploration recipe. A
prompt-facing agent should use compact line protocol, for example
`ts-harness search text OrderStatus --view seeds .`, and reserve `--json` for
tests, receipts, validators, IDE/Flowhub, or other machine consumers.

For TypeScript, `search owner` resolves reasoning owners first, then
parser-visible modules, then existing project paths. Parser-visible modules
outside the reasoning owner graph are represented with
`fields.source=parser-visible-module`, `fields.parserOwner=false`, role/layer
metadata, line counts, validity, and diagnostic counts. Existing paths outside
the parser module set are still represented as path-only owners with
`fields.source=path-only`, `fields.parserOwner=false`, and
`nextActions=[{kind:"ingest", target:<path>}]`. `search text` indexes
parser-visible source text, owner paths, and exports; docs, schema files, and
other non-parser text should be expanded with `rg` or `fd` and normalized
through `search ingest`. The TypeScript registry advertises this directly:
`search/owner` carries TypeScript-scoped
`parser-visible-module-owner-search`, `test-owner-search`, and
`ingestRequiredFor=[{languageId:"typescript",namespace:"typescript",name:"non-parser-path"}]`;
`search/text` carries TypeScript-scoped
`parser-visible-source-text-search` and TypeScript-scoped ingest surfaces for
non-parser text, docs text, schema JSON, and generated artifacts.
`search/api` projects TypeScript parser-owned exported/public API facts from the
current workspace. Dependency-prefixed or external-version API queries require a
separate docs/API source and must not present current workspace parser facts as
dependency-version documentation.
`search/public-external-types` projects TypeScript parser-owned public type
surfaces that expose a dependency package. Direct import-type text is confirmed;
owner-level external import plus unbound type text is marked possible until the
provider exposes named import binding attribution.

The Rust slice emits the same envelope from `rs-harness search ... --json`,
including Cargo, owner, dependency, symbol, callsite, import, cfg, pattern,
docs, api, public-external-types, tests, and ingest views.

The current Python slice emits conforming packets from:

```shell
py-harness search workspace --json .
py-harness search prime --json .
py-harness search owner src/python_lang_project_harness/_cli.py --json .
py-harness search dependency pytest --json .
py-harness search deps pytest::fixture --json .
py-harness search api PythonHarnessReport --json .
py-harness search public-external-types pytest --json .
py-harness search symbol PythonHarnessReport --json .
py-harness search callsite PythonHarnessReport --json .
py-harness search import python_lang_project_harness --json .
py-harness search tests src/python_lang_project_harness/_cli.py --json .
py-harness search text PythonHarnessReport --json .
rg -n "PythonHarnessReport" src tests | py-harness search ingest --json .
```
