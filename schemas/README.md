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
the equivalent compact command. Providers should emit JSON in a compact
machine-oriented form, leaving readability to validators and artifact viewers
rather than spending terminal tokens on pretty-print whitespace.

`semantic-graph.v1.schema.json` is the shared embeddable graph vocabulary behind
search packets. It owns parser-proved graph nodes, graph edges, bounded
synthesis algorithms, frontier owners, finding owners, and graph-derived next
actions. Agent workflows should consume graph evidence through normal
`search ...` packets: `nodes`, `edges`, and `searchSynthesis` carry the graph
slice that lets the LLM choose the next focused search. The graph schema exists
to keep that embedded vocabulary aligned across providers, not to introduce a
separate `search graph` or top-level graph exploration workflow.

`semantic-type-surface.v1.schema.json` is the shared vocabulary for
language-neutral public type surface facts. It owns the facts that agents need
to compare across Rust, TypeScript, Python, Julia, and future providers: type
name, kind, role, owner path, visibility, member shape, external origin, and
version scope. It does not model a complete language type system. Compiler,
AST, checker, lifetime, variance, overload, or typing-module details stay in
provider-owned `fields` maps or provider-local schemas. Search packets may
embed these facts through optional `typeSurfaces` when views such as
`search/api`, `search/public-external-types`, or provider-native query output
need a contract-visible type surface.

`semantic-invariant-candidate.v1.schema.json` is the shared vocabulary for
machine-facing invariant candidates raised from parser-owned findings before
test, receipt, proof, or review evaluation. Findings remain the human-facing
diagnostic surface; invariant candidates carry stable ids, source rule ids,
candidate kind, concrete location, evidence, and required receipt hints. P0
providers should emit candidates additively, without deleting or parsing
finding summaries. P1 receipt schemas, P2 behavior snapshots, P4 proof pilots,
and P5 review packets consume this shared candidate shape.

`semantic-verification-receipt.v1.schema.json` is the shared executable
evidence receipt emitted by tool adapters. It records the producer, tool
adapter, command argv, status, exit code, duration, compact observations,
candidate ids, task fingerprints, and artifact references. It is distinct from
the Rust harness verification lifecycle receipt: lifecycle receipts answer
"does this configured task clear"; verification receipts answer "what tool ran
and what evidence did it produce". P1 covers receipt command shaping for
`cargo-check`, `cargo-test`, `clippy`, `expect-test`, `proptest`,
`cargo-fuzz`, `kani`, `creusot`, and `verus`; P4 decides which formal proof
harnesses should be trusted as project rules.

`semantic-behavior-snapshot.v1.schema.json` is the shared observable-behavior
snapshot contract for expect-test outputs, golden public API shapes, CLI
observations, and review-visible behavior diffs. It records the producer,
subject, status, compact observations, optional expected/actual/diff values,
and links back to verification receipt ids or invariant candidates. P2 uses it
to let agents see behavior, not only type shape.

`semantic-determinism-readiness.v1.schema.json` is the shared readiness
contract for direct nondeterminism sources. It records parser-owned
observations for clock, random, filesystem, network, environment, and
global-state access, plus review-visible suggestions such as trait injection or
explicit parameter boundaries. P3 uses it to make determinism blockers concrete
before any larger simulation or mocking strategy is considered.

`semantic-formal-proof-pilot.v1.schema.json` is the shared proof-pilot
contract for bounded or formal evidence that a harness rule judgment is
reliable. It records the target rule surface, proof method, claims, concrete
checks, model counts, and optional verification receipt links. P4 uses it for
small focused pilots such as dependency graph acyclicity before widening to
Kani, Creusot, or Verus-backed receipts.

`semantic-review-packet.v1.schema.json` is the shared reviewer-first artifact
that consumes the new evidence APIs: invariant candidates, verification
receipts, behavior snapshots, determinism readiness packets, proof pilots, and
explicit review-packet waiver evidence. P5 uses it to summarize changed
invariants, changed behavior, missing receipts, stale waivers, determinism
observations, proof claims, and prioritized reviewer actions without depending
on legacy lifecycle waiver/task objects.

`semantic-evidence-graph.v1.schema.json` is the shared portable graph artifact
over reviewer evidence. P6.1 uses it to link review packets, invariant
candidates, receipts, behavior snapshots, determinism readiness summaries,
proof pilots, waivers, and review actions as explicit nodes and edges. It is an
artifact contract, not a database or long-lived storage layer: providers can
emit it from current evidence packets, reviewers can inspect it, and later
assurance-case renderers can consume it without inventing a new evidence
vocabulary.

`semantic-assurance-case.v1.schema.json` is the shared reviewer-first assurance
artifact derived from an evidence graph. P6.2 uses it to turn graph nodes and
edges into claims, supporting evidence references, review actions, stale waiver
references, and open gaps. It deliberately keeps references by graph node id
instead of embedding another full graph, so assurance rendering stays portable
without becoming a storage or visualization layer.

`semantic-query-packet.v1.schema.json` is the shared JSON contract for
provider-native parser queries that return compact code by default. Query is a
language-provider capability, not a root hook capability: Rust, TypeScript,
Python, and future providers own AST/parser lookup, exact item matching,
multi-term expressions such as `fun1|fun2|fun3`, and compact code extraction.
Root hooks should route source access back to provider `search owner <path>
items [--query SYMBOL]`; they should not maintain a parallel read/query engine.
The query packet also supports owner-local discovery without source windows:
`outputMode=names` or `outline` may omit match `code`, while `queryCoverage`
and bounded `candidateItems` explain missed terms and parser-owned repair
candidates.

`semantic-handle.v1.schema.json` is the shared contract for stable semantic
facts that agents need to query but that are not necessarily parser items. It
covers policy rule ids, schema fixtures, test cases, config keys, command
surfaces, dependency APIs, provider capabilities, and similar handles across
Rust, TypeScript, Python, Julia, and future providers. Search and query packets
may embed these facts as optional `semanticHandles`; language-specific details
stay in provider-owned `fields`.

`semantic-native-syntax-fact-index.v1.schema.json` is the shared contract for
parser-owned syntax facts. It exists so code-shaped queries such as `pub use
rules`, `fn format_field`, `struct PacketCollections`, `import {Foo}`, or
`def run` are routed through native parser facts before semantic text search.
The root schema owns only the portable fact envelope: fact id, kind, source,
owner path, location, visibility, query keys, relations, and extension fields.
Rust, TypeScript, Python, Julia, and future providers own their concrete fact
builders and provider-local schema refinements. Search and query packets may
embed these facts as optional `nativeSyntaxFacts`.

`semantic-finder-tools.v1.schema.json` is the shared contract for
provider-approved finder pipelines behind `search fzf`, compatibility
`search fzf`, `search ingest`, and `search pattern`. It describes tool catalogs
and pipelines such as `rg+fzf` without exposing raw shell argv to agents. `rg`
owns lexical candidate generation, `fzf` owns headless fuzzy filtering/ranking,
and the language provider owns path normalization, owner resolution,
nearest-item resolution, test frontier selection, deduplication, caps, and
packet rendering. `ast-grep` is modeled as a structural recipe/search tool, not
as a fuzzy text backend or a replacement for native provider `query`.

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
text. JSON stdout expectations can assert exact paths, substring containment,
schema conformance, and array membership with scalar values or object subsets.
Large-library calibration scenarios use typed `evidence.targetLibrary`,
`evidence.fixtureTier`, and `evidence.intentCases` metadata so every provider
can publish the same feature/API/principle search matrix without asking the
harness to parse natural-language intent. Coverage audits render this as
`|intent-matrix` and `|intent-library` lines, and `--fail-on-missing` treats
missing large-library rows or missing intent cases as coverage failures.
`intentCases[].queryTerms` records which query-set terms exercise each intent
when several same-view probes are compressed into one scenario step.

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
policy toggles without making the hook classifier language-specific. Profiles
may also advertise a `commands.guide` template; deny messages should point to
that provider-owned `agent guide` command instead of reconstructing searchflow
instructions from route argv.

`semantic-agent-hook-decision.v1.schema.json` is the shared decision packet for
the root hook classifier before it renders a platform-specific Codex or Claude
hook response. It standardizes normalized event names, deny/context decisions,
language/provider routes, and state updates while provider repositories own only
their language profile descriptors and semantic search/check commands.

`semantic-read-packet.v1.schema.json` is an optional provider-owned packet for
bounded exact source windows selected by the language query layer. It is not a
root hook command surface and does not reintroduce `semantic-agent-hook read`.
Providers may emit it from `query/*` methods, for example an exact
`query --from-hook direct-source-read --selector <path>` recovery with
`outputMode=read-packet`. The packet records parser-owned selection evidence:
project-relative selectors, owner paths, optional item facts, bounded line
windows, truncation state, and notes. Broad discovery still stays in provider
search, prime, ingest, or normal query repair.

`semantic-search-packet.v1.schema.json` owns the search-synthesis frontier that
precedes read packets. `searchSynthesis.editFrontier` names source owners,
`searchSynthesis.testFrontier` names coupled tests, and
`searchSynthesis.windowSet` names typed `{kind,target}` owner/test/read windows
that an agent may inspect with bounded read transport after the provider has
selected the semantic axis.

The TypeScript provider registers as:

```json
{
  "languageId": "typescript",
  "providerId": "ts-harness",
  "binary": "ts-harness",
  "namespace": "agent.semantic-protocols.languages.typescript.ts-harness",
  "methods": ["search/workspace", "search/prime", "check/full", "agent/doctor", "agent/guide"],
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
Query descriptors use `query/*` methods, advertise packet schemas such as
`agent.semantic-protocols.semantic-query-packet` and optional
`agent.semantic-protocols.semantic-read-packet`, and describe owner-local inputs
such as `input="owner-path"`, required options such as `--term`, and supported
`outputModes` including compact, JSON, code, names, outline, and read-packet.
They must not reuse a search `view`; query is the parser-owned item lookup
surface that lets an agent repair stale symbol probes without escalating to
source reads.
When a search packet embeds shared sub-schema content, descriptors list both
the packet schema and the embedded sub-schema. For example,
`search/public-external-types` advertises
`agent.semantic-protocols.semantic-search-packet` plus
`agent.semantic-protocols.semantic-type-surface` because its JSON packet may
populate `typeSurfaces`.
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
`search/fzf` accepting `owner` and `tests`.

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
  `dependency`, `deps`, `symbol`, `callsite`, `import`, `query`, `cfg`,
  `patterns`, `pattern`, `docs`, `api`, `public-external-types`, `policy`,
  `tests`, `fzf`, `text`, or `ingest`
- `header`, `packages`, `nodes`, `edges`, `owners`, `items`, `hits`,
  `findings`, `nextActions`, and `notes`
- optional `typeSurfaces` for shared public API and dependency type surface
  facts
- optional `invariantCandidates` for shared test/proof/review candidate facts
  raised from provider-owned findings
- optional `semanticHandles` for stable non-code semantic facts such as policy
  rules, schema fixtures, test cases, config keys, and provider capabilities
- optional `nativeSyntaxFacts` for parser-owned syntax facts from
  `semantic-native-syntax-fact-index.v1`, used by code-shaped query routing
  before broad text search
- optional `querySet` and `queryComposition` for homogeneous same-view
  query-set packets
- optional `queryCoverage`, `ownerResolution`, `searchSynthesis`, and
  `avoidNextActions` when a provider must explain term-level coverage, fixture
  paths, false owner candidates, or synthesized follow-up seeds
- optional `sourceCoverage`, `testResolution`, and `runtimeCost` when a large
  project search must explain parser-visible source coverage, owner-to-test
  reachability, or cold/warm index cost
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
`search/fzf` query-set packet with `scope.ownerPath`, not several separate
text packets or a comma-joined literal query.
Project-scoped TypeScript text query-sets are also valid when the owner has not
been selected yet and the repeated probes are still the same text axis.

Query-set packets must not only merge terms; they must preserve the meaning of
each term. When a text hit is a test fixture string such as
`"src/cli/agent-hooks.ts"` inside `tests/unit/cli.test.ts`, the packet should
classify the hit as `surface="test-fixture-string"`, set `realOwner=false`,
record `fixturePath` and `fixtureOwner`, add `queryCoverage` for every term,
and add `ownerResolution` so the agent knows not to run
`search owner src/cli/agent-hooks.ts`. If the provider can infer a real
implementation axis from the fixture context, it should emit
`searchSynthesis.seeds` such as `text:runProtocolCli` or
`owner:src/cli/protocol.ts`, and put the false follow-up in `avoidNextActions`.

Providers may also use `searchSynthesis` for bounded graph-derived planning
facts. The shared schema owns the graph algorithm name, scope, high-impact
owners, frontier owners, and finding owners as explicit `searchSynthesis`
properties; derived follow-up routes belong in `searchSynthesis.seeds`. These
facts rank and explain parser-owned owner/dependency/test edges but do not
introduce a second source of truth.

Large-library packets should keep source and runtime limits explicit instead
of forcing the agent to discover them through repeated commands.
`sourceCoverage` reports whether the selected package root or config made the
expected source owners parser-visible. `testResolution` reports whether a
tests search linked, missed, or noisily found tests for an owner. `runtimeCost`
reports coarse cache and parser reuse facts such as `cacheStatus`, `elapsedMs`,
`sourceFilesParsed`, and `parserFactsReused`. These fields are evidence for
follow-up search planning; provider-specific compiler details still belong in
`fields`.

For `search fzf`, a flag-like first query positional remains literal. For
example, `ts-harness search fzf --json --view seeds .` searches for the token
`--json`; request JSON output by placing `--json` after the query.

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
ts-harness search fzf OrderStatus --json .
rg -n "OrderStatus" src tests | ts-harness search ingest --json .
```

Those JSON examples are contract checks, not an agent exploration recipe. A
prompt-facing agent should use compact line protocol, for example
`ts-harness search fzf OrderStatus --view seeds .`, and reserve `--json` for
tests, receipts, validators, IDE/Flowhub, or other machine consumers.

For TypeScript, `search owner` resolves reasoning owners first, then
parser-visible modules, then existing project paths. Parser-visible modules
outside the reasoning owner graph are represented with
`fields.source=parser-visible-module`, `fields.parserOwner=false`, role/layer
metadata, line counts, validity, and diagnostic counts. Existing paths outside
the parser module set are still represented as path-only owners with
`fields.source=path-only`, `fields.parserOwner=false`, and
`nextActions=[{kind:"ingest", target:<path>}]`. `search fzf` indexes
parser-visible source text, owner paths, and exports; docs, schema files, and
other non-parser text should be expanded with `rg` or `fd` and normalized
through `search ingest`. The TypeScript registry advertises this directly:
`search/owner` carries TypeScript-scoped
`parser-visible-module-owner-search`, `test-owner-search`, and
`ingestRequiredFor=[{languageId:"typescript",namespace:"typescript",name:"non-parser-path"}]`;
`search/fzf` carries TypeScript-scoped
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
py-harness search fzf PythonHarnessReport --json .
rg -n "PythonHarnessReport" src tests | py-harness search ingest --json .
```
