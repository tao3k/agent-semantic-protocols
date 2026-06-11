# Semantic Search Schemas

`semantic-language-registry.v1.schema.json` is the language-server-style
provider registry. It records the semantic language protocol, language ids,
provider ids, executable binaries, callable methods, structured method
descriptors, and packet schemas.

`software-criterion-catalog.v1.schema.json` is the shared software criterion
canon: criterion ids, criterion-domain vocabulary, evidence kinds, repair
recipe shape, and severity promotion policy.
`software-criterion-extension-report.v1.schema.json` is the optional
ecosystem/profile criterion envelope for provider-owned packs such as
`typescript.extension.effect`, `rust.extension.tokio`,
`julia.extension.moshi`, and `julia.profile.sciml`. The core canon supplies
navigation vocabulary; extension reports carry ecosystem-specific activation
evidence, parser/compiler facts, source doctrine, and repair recipes without
turning Effect, Tokio, Moshi, or SciML into cross-language mapping rules.

`semantic-search-packet.v1.schema.json` is the shared JSON contract for search
output across semantic language providers. Compact text stays the default
prompt surface; JSON is the validation, cache, and artifact shape. Agent-facing
interactive exploration should not request `search ... --json`; hooks should
deny that output-mode error with `reasonKind=agent-search-json` and guide to
the equivalent compact command. Providers should emit JSON in a compact
machine-oriented form, leaving readability to validators and artifact viewers
rather than spending terminal tokens on pretty-print whitespace.
Document language providers such as `org` and `md` use document-specific packet
shapes. `semantic-document-search-packet.v1.schema.json` owns metadata search
facts for headings, TOC outlines, properties, tables, blocks, links, and
selectors.
`semantic-document-query-packet.v1.schema.json` owns document query metadata
and filtered `--content` element projections, with explicit `queryKind`,
`querySurface`, and `contentBlocks` fields. Document hook recovery must use
document `query` routes, not source `search owner` or owner/items routes. These
providers must not report document facts through source-language
`nativeSyntaxFacts`.
`semantic-org-elements-query-packet.v1.schema.json` owns the host-facing
org-elements index query input used by `orgize elements-query --packet ...`.
It keeps `CONTRACT_ORG` and other consumers on parser-owned org element
predicates, such as category, kind, affiliated names, context, outline,
property, summary, relations, and boolean composition, instead of treating
contracts as a skeleton DSL or search stdout.
`semantic-content-compaction.v1.schema.json` owns content-level compaction
metadata for source code, documentation, logs, test output, schema JSON, review
judgments, and proof/evidence text. It records `contentKind`, `criticality`,
`lossiness`, `trustLevel`, `validFor`, `notValidFor`, `preserved`, `omitted`,
and exact-source requirements. This is a content payload transform, not a graph,
frontier, rank, action, or command-materialization protocol.
For tree-sitter-backed source query rendering, non-`--code` output is
locator/frontier evidence only, while `--code` prints pure source code. Query
render profiles such as the `compact-graph-frontier` profile and
`corpus-locator` profile project an ASP-compiled tree-sitter query plan over
provider-native projection; they do not introduce a new packet surface.
RFC 009 adds optional `reasoningProfiles` to this packet as a typed return-entry
surface for `search prime` and `search reasoning <profile>`. Those entries
describe profile names, selector slots, returns, and frontier actions; they
deliberately reject natural-language `goal` or `intent` fields so planning stays
in the agent.

`semantic-graph.v1.schema.json` is the shared embeddable graph vocabulary behind
search packets. It owns parser-proved graph nodes, graph edges, bounded
synthesis algorithms, frontier owners, finding owners, and graph-derived next
actions. Agent workflows should consume graph evidence through normal
`search ...` packets: `nodes`, `edges`, and `searchSynthesis` carry the graph
slice that lets the LLM choose the next focused search. The graph schema exists
to keep that embedded vocabulary aligned across providers, not to introduce a
separate `search graph` or top-level graph exploration workflow.

`semantic-fact-ontology.v1.schema.json` is the shared field/type/collection
fact ontology introduced by RFC 013. It defines parser-owned `field`, `type`,
and `collection` nodes plus relation vocabulary such as `has_type` and
`collection_of` so Rust, TypeScript, Python, and Julia providers can describe
equivalent sequence/map facts without provider-private names.
`semantic-fact-graph.v1.schema.json` is the runtime packet emitted by
`search semantic-facts --json`; provider descriptors advertise it for provider
fact graph output rather than `semantic-graph.v1`. The companion
`semantic-fact-ontology.fixtures.v1.json` catalog keeps the P0 cross-language
matrix executable: `Vec`/`HashMap`, `Array`/`Map`, `list`/`dict`, and
`Vector`/`Dict` must validate through the same schema before graph-turbo ranking
or sparse relation banks consume those facts.
`semantic-fact-frontier-receipt.v1.schema.json` records whether a task-session
frontier was returned, followed, read, tested, or edited. Its top-level fields
stay present with `null` or empty-array values when a receipt kind does not use
that field, so benchmark conversion can distinguish not-applicable from
not-recorded.
`semantic-fact-frontier-receipt.fixtures.v1.json` keeps the real-project
receipt catalog for this shape, covering task-session, graph-turbo runtime,
ASP runtime frontier-only, and ASP runtime followed/read/test captures.
`semantic-fact-frontier-benchmark-report.v1.schema.json` owns the offline
comparison packet that joins receipt metrics, graph-turbo benchmark metrics,
gold context, and derived ContextBench-style scoring metrics before live
sandtable scoring. Its fixture catalog,
`semantic-fact-frontier-benchmark-report.fixtures.v1.json`, currently fixes
six scenarios across those receipt kinds, including three calibration-ready ASP
runtime followed/read/test rows and a relation-weight hot-companion case with
`goldFrontierBestRank` plus `goldSelectorActionRank`, so calibration decisions
can distinguish frontier emission, actual frontier use, selector action order,
and missed or recovered gold context. Its report summary may also carry
`weightCalibrationDecision`; the current fixture defers relation-weight changes
until a new calibration-ready runtime receipt shows a frontier miss or missing
gold selector action.

`semantic-compact-graph-render.v1.schema.json` is the legacy shared stdout
render template for compact graph search output. It describes the view-native
header contract, micro-legend grammar, role-typed alias line grammar, dense
alias separator, combined `rank=... frontier=...` line, legend-declared search
root, renderer ownership, and source-kind to node/target-role/action/relation
vocabulary used by Rust, TypeScript, Python, and future providers. It is not
the trusted machine protocol for graph/frontier/rank/action evidence; providers
derive facts from `semantic-search-packet.v1.schema.json`,
`semantic-graph-turbo-result.v1.schema.json`, or an explicit JSON projection.
Language providers under `languages/` may call `asp graph render` for legacy
stdout compatibility instead of adding renderer library dependencies. Input
packets use one canonical field vocabulary: query-set count comes from
root-level `querySet`, and graph frontier source locators use
`searchSynthesis.seeds[].read`; provider-specific field aliases are schema
violations rather than renderer compatibility cases.
Owner-local item search must retain `owner=`, `selector=items`, term count,
and `view=seeds` in the header, declare every packet-local alias id in the
legend, split search-match and owner-containment edges, rank matched symbols
before the already-expanded owner, and emit `omit` / `avoid` facts that steer
agents away from repeat owner searches, raw reads, and full JSON.
Its `!code` symbol aliases must also carry parser-owned read locators:
same-owner aliases use `@start:end`, while cross-owner aliases use
`@path:start:end`.
When a source packet carries `reasoningProfiles`, the shared compact graph
renderer may
emit `entries=<profile>(<ID>,...=><return>+...)` after the
`rank=... frontier=...` line. Every selector in that line is a rendered
packet-local alias id whose node kind matches the typed profile selector, so the
line is a return-entry catalog for the current graph packet rather than an alias
hint or a second action protocol.
Profile names are not free-form compatibility aliases. The shared
`reasoningProfileName` catalog currently accepts `owner-query`, `query-deps`,
`owner-tests`, `finding-frontier`, and `feature-cfg`; adding a new prompt-facing
entry name requires a schema update so Rust, TypeScript, Python, and future
providers can compare the same returned entries.
`semantic-compact-graph-render.v1.schema.json` exposes the prompt-facing
`reasoningProfileContracts` catalog, including selector order, optional
selectors, and return entries. `semantic-search-packet.v1.schema.json` validates
implemented packet profiles against the same contract, so compatibility aliases
or extra selectors are schema errors rather than model-inferred hints.
Provider `guide` and `search guide` output should print the same catalog
as a compact `reasoningProfiles=... entries=... routes=...` line. Runnable
reasoning rows belong in `entries`; repair/direct-read flows belong in `routes`
or command lines and must not be presented as reasoning-profile entries.
Provider-specific `entries` can be a subset of the shared catalog. If a provider
implements a profile through an existing selector slot such as `--query`, it
should document that command directly instead of adding compatibility aliases
that would create new drift points.
Implemented selector profiles must also expose the selected value as a typed
packet action, for example `nextActions.kind="feature"` or
`nextActions.kind="finding"`, so compact graph `entries=` rows are matched from
schema-visible facts instead of prompt inference.

`agent-semantic-client-config.v1.schema.json`,
`agent-semantic-client-cache-manifest.v1.schema.json`, and
`agent-semantic-client-receipt.v1.schema.json` own the agent semantic client/backend
envelope. They describe route mode, provider set, privacy policy, cache
generation provenance, SQLite client DB status, execution route, provider
command counts, and native provider provenance. They do not duplicate `semantic-search-packet` or
`semantic-query-packet`, and they do not rename the lower layers:
`agent-semantic-protocol` still owns shared protocol rendering and
`agent-semantic-hook` still owns hook classification. agent semantic client is the
client/backend brand. Arrow and Flight remain server/cloud capabilities rather
than default client-cache dependencies. `cache-status` receipts are read-only
inspections; the prompt line reports manifest/DB health as `missing`,
`unimported`, `available`, `invalid`, or `unavailable`, while the receipt keeps
machine routing state in `cacheStatus` plus `cacheManifestStatus` and
`clientDbStatus`. Local DB receipts also expose normalized syntax row
generation/match/capture counts so cache hits can distinguish artifact replay,
row replay, and warm-provider gaps. Runtime DB diagnostics expose observed
`clientDbJournalMode`, `clientDbSynchronous`, `clientDbBusyTimeoutMs`, and
`clientDbForeignKeys` so WAL/busy-timeout drift is machine-visible in cache
status and replay receipts. `cache-import` receipts describe explicit
SQLite imports from a validated provider-owned manifest. Manifest re-imports
must preserve unrelated normalized row families rather than replacing the parent
generation in a way that cascades syntax rows away. `cache-invalidate`
receipts describe
local SQLite generation-row invalidation and do not imply manifest or artifact
deletion. `cache flush syntax-rows` deletes only normalized syntax query row
families and preserves manifest generations plus artifact provenance. In
local-native receipts, `warm-provider`
means a matching SQLite generation was found but provider execution still
supplied the output; only `hit` means the client served output from cache. The
initial replay surface covers provider-owned `prompt-output/*.txt` artifacts,
`search/*.json` semantic-search-packet artifacts rendered through shared compact
graph output, and `query/*.json` semantic-query-packet artifacts for
`query/owner-items` compact query replay under the protocol artifact root.
`semantic-tree-sitter-query/*.json` artifacts and normalized syntax rows replay
only through AST/ABI fingerprints plus freshness hashes. Syntax-query receipts
surface the AST/ABI fingerprint, grammar id, grammar profile version, and
selector when present; artifact ids remain provenance rather than cache facts.
Providers may supply
`/cache/fileHashes`; when they do not, the client may hash validated syntax
locator paths from the packet, storing only path+sha256 and no raw source.
The client may also capture successful replay-safe `search --view seeds`
provider stdout as `prompt-output/*.txt` write-back artifacts for the next
identical request. `prompt-output/*.command.json` stores the matching
provider-command provenance when stdout replay has no packet-level command
field. This path deliberately excludes query/code windows.
Structured relation and flow evidence uses schema-owned JSON artifact families
instead of prompt stdout: `relation-plan/*.json`, `flow-lite/*.json`, and
`codeql-evidence/*.json`. When these structured evidence artifacts are present,
the client keeps them as evidence/provenance and does not fall back to
`prompt-output/*.txt` direct replay for that generation.
The CodeQL evidence family belongs to the experimental ASP CodeQL extension,
not to a default provider hot path. It supports both metadata-only normalized
row artifacts and cold-path unavailable artifacts with `rowCount: 0` plus a
`backend-unavailable` omission. This lets providers and clients return a stable
extension receipt without requiring CodeQL to be installed or advertising
`executionBackends: ["codeql"]` from provider registry descriptors.
When CodeQL is installed, `packages/python/src/tools/codeql_evidence.py`
normalizes `codeql version --format=json` and
`codeql resolve languages --format=json` into the same metadata-only evidence
family, so CLI metadata can be tested without storing raw CodeQL output in
prompt-facing artifacts.
`packages/python/src/tools/codeql_bounded_evidence.py` covers the next cold path:
it copies a tiny Rust fixture to a temporary source root, creates a CodeQL
database, runs a bounded raw-dbscheme `files` query, decodes BQRS, and emits
`codeql-evidence/bounded/*.json`. This proves CodeQL database/query execution
without treating the installed extractor as a full Rust QL semantic executor.
Because Rust CodeQL database creation dominates runtime, the tool stores a
repo-local warm cache under `.cache/agent-semantic-protocol/codeql-fixtures` and
records `databaseCacheStatus` in evidence fields.
Ordinary `search` and `query` commands must not create CodeQL databases or run
CodeQL queries. They may reference previously produced `codeql-evidence/*.json`
artifacts only after native provider facts have selected a bounded evidence
question and the `extensions.codeql` ASP project config or an explicit
extension command allows that evidence path.

`agent-semantic-project-config.v1.schema.json` owns the shared `asp.toml`
project configuration surface. `discovery.ignoredDirNames` is the canonical
directory-skip list, and `discovery.includeHiddenDirNames` is the only
schema-owned way to opt hidden directories into provider project walks. The
ASP facade applies activation-root config first and invocation-root config
second; list assignments are normalized replacements, not prompt-time merges.
The legacy `[search] ignoreDirs/includeHiddenDirs` names remain runtime input
compatibility only. Source-language providers, embedded document providers
(`org`/`md`), fd/rg prefilters, and hook activation should consume the same
normalized config before selecting provider facts or binaries. Built-in
document providers are enabled by default and require no activation entry, but
they still honor `providers.org.enabled=false` and
`providers.md.enabled=false`. Hook activation also consumes
`providers.<language>.enabled` to disable external providers and
`providers.<language>.binary` to pin external provider executables.
Provider-specific policy config may stay in language-owned files, but source
discovery and provider selection must not silently diverge from the nearest
`asp.toml`.
The same config owns extension activation under `extensions.*`. The CodeQL
extension is default off and default experimental:
`extensions.codeql.enabled=false`, `extensions.codeql.experimental=true`, and
`extensions.codeql.mode="disabled"`. Enabling it changes only explicit
extension/evidence paths unless a cache-only artifact is already available; the
schema keeps `extensions.codeql.allowHotPath=false` so ordinary search, query,
hook recovery, and `check --changed` cannot be configured to create CodeQL
databases or run CodeQL queries.

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

`semantic-dev-command-log.v1.schema.json` is the development-only JSONL event
contract emitted by providers when `SEMANTIC_PROTOCOL_DEV_MODE=1` is enabled.
It records command argv, project identity, normalized method/view/query facts,
session ordering, start/end timestamps, exit code, elapsed time, and
stdout/stderr byte counts under `$PRJ_CACHE_HOME/semantic_protocol` or
`SEMANTIC_PROTOCOL_TRACE_DIR`. It does not record full stdout or stderr
content, so normal agent exploration remains compact and source-safe.

`semantic-dev-active-context.v1.schema.json` is the short-lived marker contract
written by hook runtimes under `semantic_protocol/dev-context/`. Providers read
it as a best-effort development trace aid so direct `*-harness` commands can be
attached to the latest hook/session context without requiring every command to
receive hook environment variables.

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
Document providers use `semantic-document-query-packet.v1.schema.json` instead
of this source-language packet: `asp org query --term <term> --view metadata`
and `asp md query --term <term> --view metadata` return bounded document fact
frontiers, while `asp org query --term <term> --content` and
`asp md query --selector <path:start-end> --content` keep stdout as pure
matched element content. Source-preserved document reads stay on
`query --from-hook direct-source-read --selector <path-or-range>` and must not
be combined with `--content`.
Root hooks should route source access back to provider `search owner <path>
items [--query SYMBOL]`; they should not maintain a parallel read/query engine.
The query packet also supports owner-local discovery without source windows:
`outputMode=names` or `outline` may omit match `code`, while `queryCoverage`
and bounded `candidateItems` explain missed terms and parser-owned repair
candidates. Julia remains workspace-managed for performance reasons, but its
`query <owner-path> --term <symbol> --json` output uses this same packet shape
so the Rust client can cache and reuse provider facts without inventing a
Julia-private search payload.
Compact AST projections use `projection.nodes[].id` as their shared reference
keys. `renderedNodeIds` records which nodes own primary compact rows, while
`omitted[].nodeId` and `expandActions[].target` should refer back to nodes or
exact read locators instead of duplicating hidden code. JSON Schema covers the
field shape and direct uniqueness such as `renderedNodeIds`; cross-field
projection identity invariants are enforced by
`test_semantic_query_packet_projection_uniqueness.py`. The protocol semantics
for these fields are owned by
`docs/10-19-rfcs/10.10-semantic-query-projection-protocol.org`.

`semantic-tree-sitter-query.v1.schema.json` is the shared portable ABI for
tree-sitter-compatible syntax query results exposed through the existing
provider `query` method. It does not create `ts-query`, `syntax-query`, or a
second public command family. ASP owns catalog ids, canonical `.scm` catalog
metadata, schema validation, artifact/cache references, replay receipts, and
prompt render hints; language providers remain the authority for native
parser/compiler facts, catalog source delivery, grammar-profile delivery, and
project captures into this packet. `.scm` is the only repository and registry
catalog filename extension for this ABI; Scheme-like S-expression query text is
an input form, not a `.scheme` filename compatibility surface. Search, query,
read, and native syntax fact packets can refer back to this ABI through
`syntaxQueryRef`, `syntaxMatchRefs`, `syntaxCaptureRefs`, and an optional short
`syntaxAnchor` when those references improve a decision path without adding a
new render protocol.
Predicate facts under `query.fields.predicates` use structured operands
`{op,capture,values:[{kind,value}]}` so `string` and `capture` operands remain
explicit across ASP cache keys and native provider projection. The `op` label is
preserved as ABI data, including tree-sitter `any-eq` and `any-match` variants.
The current v1 implementation boundary is tree-sitter-compatible native projection:
packets may carry `sourceAuthority=native-parser-adapter`,
`adapterMode=native-projection`, and `compatibilityLevel=native-only`,
`approximate`, or `exact` without requiring a grammar-specific tree-sitter
runtime. Complete tree-sitter-runtime matching, ASP-compiled pattern graphs,
multi-capture match rows, and field structural selectors are next-layer
capabilities that must remain explicit in RFC text and provider descriptors
before consumers treat them as generally supported.
The optional execution backend field keeps engine choice separate from packet
shape: `executionBackend=codeql` and `adapterMode=codeql-query` are allowed for
CodeQL-projected semantic frontier packets. CodeQL is an optional semantic
backend rather than a new command family or a requirement for basic syntax
query. The first CodeQL-aligned target is `flow-lite` local source/sink/path frontier,
still rendered through frontier-first packets and exact `--code` follow-up
selectors.
Compact contract: CodeQL is an optional semantic backend for `flow-lite` local source/sink/path frontier.
`docs/10-19-rfcs/10.12-asp-native-relation-flow-codeql.org` owns the native relation catalog,
flow-lite packet plan, artifact policy, and CodeQL promotion gates that sit
above this schema field.
Provider registry method descriptors may advertise supported engines with
`executionBackends`; current native providers should declare `native-parser`
and must not list `codeql` until a real backend can produce the advertised
frontier packet.
`python -m tools validate provider-registry-contracts` is the focused
real-provider gate for this surface: it runs `asp <language> agent doctor
--json`, validates the returned registry against this schema, and checks that
query descriptors advertising `semantic-tree-sitter-query.v1` carry the shared
tree-sitter query provenance fields. Keep this separate from fast schema unit
tests because it starts real provider binaries.
Tree-sitter-compatible query capability is advertised on the same `query`
method descriptor with `packetSchemas`, `queryInputForms`, `queryCatalogs`,
`grammarId`, `grammarProfileVersion`, `adapterModes`, `sourceAuthorities`,
`renderProfiles`, `cacheReplay`, and `unsupportedPatternBehavior`. Catalog
descriptors use `sourceDelivery=provider-binary-embedded` plus an optional
`fingerprint`, so downstream users do not need provider package source to
resolve the canonical `.scm` ABI.

`semantic-relation-plan.v1.schema.json` is the shared relation-evidence packet
introduced by RFC 012. It records provider-owned directed relation rows between
semantic handles, the evidence authority that proved those rows, optional
artifact references, omitted relation reasons, and exact next actions. Relation
rows are protocol facts; consumers must not infer prompt-visible edges from raw
text or model guesses.

`semantic-flow-lite.v1.schema.json` is the shared bounded flow packet introduced
by RFC 012. It intentionally starts with local source/sink/path shapes such as
`local-source-sink`, `guarded-effect`, `mutation-flow`, and
`test-coverage-path`. This is not a global dataflow contract. The packet keeps
source/sink handles, ordered path steps, guard/effect points, evidence
artifacts, and `confidence=proved|bounded|partial|unavailable` explicit before
an agent asks for exact source with `--code`.

`semantic-codeql-evidence.v1.schema.json` is the metadata-only artifact contract
for optional CodeQL evidence. It records database/query fingerprints, source
snapshot identity, input handles, normalized row count, project-root policy, and
the relation plan or flow-lite id it supports. Raw CodeQL tables, logs, and
database paths stay out of prompt-facing packets by default.

`semantic-source-location.v1.schema.json` owns the shared project-relative
path, line range, and source locator vocabulary used by query, search, read,
tree-sitter query/profile/provenance, and native syntax fact schemas. Packet
schemas should reference that base instead of carrying their own path/range
regex copies.

`semantic-tree-sitter-provenance.v1.schema.json` owns the shared tree-sitter provenance base.
The packet envelopes stay separate because query, search, and read packets have
different required fields and consumer semantics, but tree-sitter provenance
must not be redefined separately in each envelope. Additive changes to syntax
provenance fields go through this shared schema first, then package-local schema
copies and provider registry descriptors. The provenance schema itself depends
on `semantic-source-location.v1.schema.json` for its `syntaxAnchor.location`.

Provider-maintained catalogs must use the upstream tree-sitter-style
`tree-sitter/<grammar-id>/queries/*.scm` layout. Selected upstream query
snapshots and corpus profiles are development/CI alignment assets.
Editor-oriented assets such as `highlights.scm` are not included unless they
are given an explicit syntax ABI calibration role. Downstream clients consume
provider-emitted packets or binary-embedded catalog sources, not provider
package source files.

Provider-local `query-corpus/*.txt` fixtures pin syntax ABI capture precision.
Providers store these fixtures beside `queries/*.scm`, but the main ASP
workspace owns validation, query compilation, cache keys, and replay semantics.
`semantic-tree-sitter-grammar-profile.v1.schema.json` owns the shared
`grammar-profile.json` shape so Rust, TypeScript, Python, Julia, and future
providers can expose the same catalog/profile/corpus contract. The profile pins
the ASP workspace git revision only as validation provenance; it is not a
current-HEAD equality gate. The compatibility gate is
`aspWorkspace.contractFingerprint`, computed from the ASP tree-sitter query
ABI/schema/validator files. It also
declares `nativeFactProjection` entries that map provider-owned native
parser/compiler facts onto catalog captures, keeping native authority visible
through the canonical `.scm` ABI rather than provider-private fields. The
fixtures may cite upstream `test/corpus` files for grammar provenance, but
should test only provider/ASP capture granularity rather than duplicate
upstream parser grammar coverage.

Agent-facing syntax query stdout has a separate render contract from the JSON
packet: non-`--code` output is locator/frontier evidence only, while `--code`
prints pure source code. Rust currently renders a graph-rendered
locator-frontier profile, and TypeScript/Python render the `corpus-locator`
profile. These are render profiles over frontier facts, not "compact frontier"
protocols. Both profiles are valid only when backed by the same ASP-compiled
tree-sitter query plan and provider-native projection, and neither profile may
expose cache ids, SQLite paths, receipts, full node lists, or raw source windows
in default non-JSON output.

`parser-compact-case.v1.schema.json` and
`parser-compact-token-cost.v1.schema.json` are the root fixture contracts for
parser compact snapshots. A case manifest names the language, fixture project,
raw source path, provider commands, and expected artifacts. The token-cost
report records raw source, compact line output, and query packet size for a
specific tokenizer. These schemas keep parser compact changes comparable across
language providers before search-flow optimization claims are accepted.

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
Portable fact kinds cover owners, modules, public APIs, imports, calls, tests,
docs, document headings, properties, drawers, tables, blocks, links, code fences,
includes, fields, bindings, constants, arguments, and macros; provider specific
syntax remains in `languageKind` and `fields`. Rust, TypeScript, Python, Julia,
Org, Markdown, and future providers own their concrete fact builders and
provider-local schema refinements. Search and query packets may embed these
facts as optional `nativeSyntaxFacts`.

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
text; guide-quality output assertions can require returned compact graph
`entries=...` facts through `guideQuality.primeOutput.entries`, require
optimized-prime status fields through `guideQuality.primeOutput.requiresStructureStatus`,
and reject stale profile names, unknown profile names, or compatibility text. JSON stdout expectations can assert exact paths, substring containment,
schema conformance, and array membership with scalar values or object subsets.
Large-library calibration scenarios use typed `evidence.targetLibrary`,
`evidence.fixtureTier`, and `evidence.intentCases` metadata so every provider
can publish the same feature/API/principle search matrix without asking the
harness to parse natural-language intent. Coverage audits render this as
`|intent-matrix` and `|intent-library` lines, and `--fail-on-missing` treats
missing large-library rows or missing intent cases as coverage failures.
`intentCases[].queryTerms` records which query-set terms exercise each intent
when several same-view probes are compressed into one scenario step.
Agent SDK replay expectations can use `expect.pipeFlow` read-loop budgets to
bound direct-code reads, duplicate selectors, adjacent range windows, and
same-owner scans; `forbiddenStages` can also reject the aggregate
`read-loop-risk` stage.
Failure-frontier replay gates use
`evidence.failureFrontierComparison` to compare a baseline receipt/trace with a
candidate receipt/trace. The candidate must prove command reduction, bounded
`direct-source-read --code` count, zero duplicate selectors, zero same-file
window fanout, and full coverage of explicit `expectedHotBlocks`. Receipt-path
comparisons validate stable checked-in replay evidence; trace-path comparisons
first normalize JSONL command traces into the same receipt contract, then run
the identical gate. Trace receipts preserve structured `failureFrontier`
entries from compact provider stdout, including rule/severity/path, the
single-line message/summary/repair fields, `hotBlockSelector`, and the copyable
`next` selector/root. When a comparison does not pass explicit
`expectedHotBlocks`, those structured frontier selectors become the declared
hot-block set. Fewer commands alone is not sufficient evidence.

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
`python -m tools sandtable` can validate these receipts directly with
`--receipt <path>` from `packages/python`, and scenarios can link a receipt through
`evidence.receiptPath`.

`semantic-agent-hook-provider-manifest.v1.schema.json` is the static provider
manifest contract consumed by `agent-semantic-hook` after a workspace activation
selects that provider. It standardizes language-owned source defaults, policy
defaults, and route argv templates without making the hook classifier
language-specific. It does not store independent command display text; command
text is rendered from argv when needed.

`semantic-agent-hook-activation.v1.schema.json` is the generated workspace
activation contract. It records which provider manifests are active in the
current project, their resolved command prefixes, manifest digests, and
coverage roots. It does not repeat provider routes or policies, so a stale
activation cannot drift into an alternate command registry.

`semantic-agent-runtime-profiles.v1.schema.json` is the derived runtime
execution profile shape for activated providers. It is not a separate
workspace source of truth and must not resurrect the retired runtime profile
state file. Activation
answers which providers and coverage are active; runtime profile facts are
derived from that activation plus current project binary resolution for
install receipts, skill rendering, and `asp hook doctor` health reporting.
`asp` facades and local native client execution must prefer activation-derived
provider argv over ad hoc shell `PATH` lookup, and doctor reports profile
health so PATH, direnv, and stale binary drift are visible instead of hidden
behind symlink behavior.

`semantic-agent-healthcheck.v1.schema.json` is the read-only report emitted by
`asp healthcheck --json`. It treats git toplevel as the first project fact,
then reports the canonical `PRJ_CACHE_HOME` or git `.cache` runtime layout,
the `PRJ_CACHE_HOME` value if present, `.agents` skill
paths, hook activation, runtime profiles, current `asp` executable, `asp` on
`PATH`, provider profile health, and compact issue codes.

ASP state storage is rooted at `${PRJ_CACHE_HOME}/agent-semantic-protocol` when
the explicit override is set, otherwise at
`<git-toplevel>/.cache/agent-semantic-protocol`. In monorepos, package roots and
subdirectories do not create separate ASP `.cache` homes; package root facts
belong in manifests, SQLite rows, receipts, and artifacts under the shared git
toplevel state root.

`semantic-agent-hook-client-config.v1.schema.json` is the optional client-side
configuration contract loaded by `asp hook` on each hook
invocation. Codex installs seed `.codex/agent-semantic-protocol/hooks/config.toml` with
schema metadata and commented examples while preserving any existing valid
project config. `.codex/agent-semantic-protocol/hooks` is durable project
policy; generated activation, profile registries, and hook event logs are cache
artifacts under `${PRJ_CACHE_HOME}/agent-semantic-protocol/hooks` or the git
toplevel `.cache/agent-semantic-protocol/hooks`. It
standardizes typed rule matchers, priorities, decisions, and routes without
introducing a client watch loop or server runtime. Rule
`languageIds` are matching filters resolved through activated provider coverage,
not just labels copied into the emitted decision. Config-derived decisions set
`fields.configRuleId`, so runtime loading rejects duplicate rule ids before
classification and mirrors schema-shape checks for identifiers, min-length
strings, events, platforms, language id uniqueness, route argv, and route
binary names. `asp hook doctor --client <codex|claude>` reports the same path
through `clientConfig` and `clientConfigStatus`; missing config is reported as
`missing`, valid config as `ok`, and invalid config is a doctor failure.

`semantic-agent-hook-decision.v1.schema.json` is the shared decision packet for
the root hook classifier before it renders a platform-specific Codex or Claude
hook response. It standardizes normalized event names, deny/context decisions,
language/provider routes, and state updates while provider repositories own only
their provider manifests. Config-derived decisions use `fields.configRuleId` to
identify the matching typed rule without parsing the message. Action-derived
decisions may also include `fields.toolSurface` and `fields.operationIntent` so
black-box tests can distinguish the client surface from the provider route.

`semantic-source-access-decision.v1.schema.json` is the Codex-internal
source-access decision packet for the no-daemon lane. It is separate from hook
decisions and records the Codex boundary, normalized operation, enforcement
mode, whether source bytes were returned locally, and whether any source bytes
became model-visible. In v1 it covers Codex-owned FS API, tool-action,
shell-preflight, shell-egress, and subprocess-open status reporting. MCP
surfaces are intentionally out of scope. Hard FS API denials require
`sourceBytesReturned=false` and `modelVisibleBytesReturned=false`; shell egress
suppression may report `sourceBytesReturned=true` while keeping
`modelVisibleBytesReturned=false`. The internal probe command
`asp source-access read-file|shell-egress --activation <activation.json> ...`
emits this packet for Codex integration tests; it is not an agent exploration
surface.

`semantic-read-packet.v1.schema.json` is the active provider-owned packet for
bounded exact source windows or actionable read-plan frontiers selected by the
language query layer. Its `schemaVersion` remains the current fixed contract
value while the read-plan frontier shape is refined. It is not a root hook
command surface and does not reintroduce a root read command. Providers
may emit it from `query/*` methods, for example an exact
`query --from-hook direct-source-read --selector <path[:range]>` recovery with
`outputMode=read-packet`. The packet records parser-owned selection evidence:
project-relative selectors or source locators, owner paths, optional item facts,
bounded source-preserved line windows, truncation state, and notes. Exact
`direct-source-read --code` windows must not be reconstructed from lossy compact
projection rows; projection may select or repair a frontier, but `sourceWindows`
text is source/formatter-preserved for the bounded selector. When a selector is
broad or low-signal, providers should emit `readPlan` with `code=false`,
`mode=range-frontier`, executable `frontier` entries, bounded `windows`, and
`avoid` actions instead of `sourceWindows`; broad discovery still stays in
provider search, prime, ingest, or normal query repair.

ASP owns the shared output mode names for provider stdout and packet requests:
`frontier`, `code`, `read-packet`, and `json`. `frontier` is the default compact
search/query mode and is source-free: providers must return metadata, omit/avoid
facts, and executable read locators rather than `|code` rows or inline `text=`
source fields. `code` is selected only by `--code` and may carry pure
source/compact code text. `read-packet` is selected by `--json --view
read-packet` and is the structured mode that may carry `sourceWindows` or
`readPlan`. Other JSON packet requests use `json`. The ASP client validates the
default frontier mode before prompt-output cache write-back and replay so
language packages cannot drift into incompatible compact renderers.

When a direct read must distinguish worktree, staged index, and committed
contents, the same packet carries `sourceVersion=worktree|index|head`.
Providers should set `repositoryRoot` when the Git repository root differs from
`projectRoot`, such as a nested language harness repo. `gitBlobOid` identifies
the Git object read for `index` or `head`; `worktreeHash` identifies bounded
worktree text. This keeps Git object reads inside the provider-owned
`direct-source-read` route instead of relying on raw `git show :path`, raw
diffs, or untracked shell dumps.

`semantic-ast-patch.v1.schema.json` and
`semantic-ast-patch-receipt.v1.schema.json` define the compact AST patch
verification boundary for `asp ast-patch`. The request owns
the language, provider, parser locator, `read` locator, and operation intent
using compact `path:start:end` and `lineRange` strings, not
`startLine`/`endLine` fields. The receipt records whether the packet is well
formed and, for Codex adapters, explicitly keeps `mutationAvailable=false` so
Codex still applies edits through its native `apply_patch` tool. Agents should
build requests with `asp ast-patch template`, run provider
`asp <language> ast-patch dry-run --packet semantic-ast-patch.json .`, then use
the exact-read preimage as patch context. Receipt `next` is intentionally
command-shaped so a hook denial does not force another schema search.

`rust-ast-patch-real-project-evidence.v1.schema.json` owns metadata-only Rust
provider evidence gathered from representative external crates. It records the
external repository commit, provider query target, selected `ast-patch-safe`
match, save-token rustfmt compact metrics, parser-owned responsibilities, and
provider dry-run/temp-apply receipt events. It deliberately rejects source text
fields and requires `sourceStored=false`, so real-project evidence can live in
fixtures without vendoring external project code.

`semantic-search-packet.v1.schema.json` owns the search-synthesis frontier that
precedes read packets. `searchSynthesis.editFrontier` names source owners,
`searchSynthesis.testFrontier` names coupled tests, and
`searchSynthesis.windowSet` names typed `{kind,target}` owner/test/read windows
that an agent may inspect with bounded read transport after the provider has
selected the semantic axis. Julia remains workspace-managed for startup-cost
reasons, but `search ... --json` still emits this shared packet so agent semantic clients
can cache search frontiers without parsing Julia-specific line text. Julia's
hook wildcard `query --from-hook direct-source-read --selector <glob>
--term <term> --surface owners,tests --view seeds --json` form uses the same packet with `querySet`,
`queryCoverage`, `sourceCoverage`, hits, frontier owners, and native syntax
facts; exact source-window JSON remains a provider `query/*` read/query packet,
not a search packet.

The TypeScript provider registers as:

```json
{
  "languageId": "typescript",
  "providerId": "ts-harness",
  "binary": "ts-harness",
  "namespace": "agent.semantic-protocols.languages.typescript.ts-harness",
  "methods": ["search/workspace", "search/prime", "check/full", "agent/doctor", "guide"],
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
`agent.semantic-protocols.hook.decision`, so providers can render
platform-specific hook payloads without changing the shared decision contract.
Query descriptors use `query/*` methods, advertise packet schemas such as
`agent.semantic-protocols.semantic-query-packet` and optional
`agent.semantic-protocols.semantic-read-packet`, and describe owner-local inputs
such as `input="owner-path"`, required options such as `--term`, and supported
`outputModes` including frontier, json, code, names, outline, and read-packet.
Providers may keep a user-facing `compact` label in guide text only as an alias
for ASP `frontier`; registry descriptors should use the ASP-owned mode names.
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
introduce a second source of truth. In agent-facing `--view seeds` output,
providers render derived follow-up routes through the RFC 006 compact graph
projection: the view-native `[search-<view>]` header, the micro-legend,
`aliases=...`, role-typed aliases, `G>{...}` edges, `rank=`, and
`frontier=`.
Providers should not render seed or synthesis as a second independent prompt
protocol.

`semantic-graph-turbo-request.v1.schema.json` is the schema-owned algorithm
input packet for the `asp-graph-turbo` Python workspace package. It carries
the requested reasoning profile, algorithm id, seed node ids, ranking budget,
optional per-kind budgets, optional window-merge controls, and typed graph
facts under `graph.nodes[]` and `graph.edges[]`. Fast-search request nodes may
carry parser-owned `syntaxQuery` locators for candidate symbols, hot range
nodes for direct code follow-ups, and dependency package nodes connected by
`owner -> dependency` import edges for query-deps routing. Search-pipe request
packets may also carry `actionFrontier[]`: typed action facts with action id,
kind, capability id, target, target role, and fields such as selector, owner
path, query, scope, recipe, or names. These action facts are materializer input
for display-only `nextCommand` text and intentionally reject materialized
`command` or `argv` fields.
`semantic-graph-turbo-result.v1.schema.json` is the matching schema-owned
response packet. It records the effective profile, algorithm, seed nodes,
budget, per-kind budgets, ranked node ids, frontier actions, relation edges,
scores, merged windows, profile compatibility, source/sink frontier, typed
paths, flow-lite path ranking, packet fingerprint, graph cache metadata,
algorithm trace, rank explanations, supported profiles, and prompt-visible
`omit`/`avoid` facts from the turbo ranking engine. It is ranking evidence,
not a prompt-facing render template or compact text protocol. Graph/frontier
token reduction should use schema-owned JSON projections, not an inline DSL
that another component parses back into actions.
`semantic-graph-turbo-summary.v1.schema.json` is the agent-facing JSON
projection of that result packet. It keeps ranked node locators, frontier
actions, selected edges, typed paths, algorithm trace, algorithm metrics, and
the active profile matrix while explicitly omitting the full score vector, full
node fields, non-active profile matrices, and source code.
Python MVP 11 uses a SciPy sparse CSR backend for `typed-ppr-diverse` so the
request/response pair can represent real matrix-backed ranking, path, cache,
trace, and sandtable metric evidence instead of a renderer-local graph format.
`semantic-graph-turbo-sandtable-summary.v1.schema.json` can also carry
report-derived `context` metrics and `benchmarkReport` provenance when
`asp-graph-turbo sandtable-summary` consumes a calibration-ready benchmark
report scenario.
Python MVP 12 requires relation-owned default edge weights and profile-owned
typed transition masks before PageRank/path ranking. The result packet exposes
edge weights plus each profile's allowed transitions and node-kind bonuses in
`profileCompatibility` so policy drift is visible outside the Python package.
The current result metrics also expose read-loop guard counters for direct code
actions, duplicate selectors, adjacent range windows, and same-owner scans.
Graph-turbo artifact timeline reports mirror that operational signal as
`readLoopRisk` JSON and `[graph-turbo-read-loop]` text evidence over cached
agent command artifacts.
This roadmap does not require a new schema version yet: future graph-turbo
packet changes should add explicit node/edge quality facts such as
`provenance`, `confidence`, and `freshness`, plus additional profile-specific
transition evidence, only when an implementation needs those facts to leave the
Python ranking boundary. PyG/HeteroData export remains an optional lab surface
and must not become part of the default request/result contract or package
dependency set.

`semantic-dependency-topology.v1.schema.json` owns the manifest-first dependency
cache packet used as graph-turbo evidence. It records language id, package
manager, manifest and lockfile hashes, source files that contributed import-site
locators, and typed graph nodes for packages, dependencies, dependency
versions, import sites, and API symbols. This packet caches topology and
locators rather than source text: manifests and lockfiles are the dependency
truth, while source import scans only add usage evidence through relations such
as `depends_on`, `version_locked`, `imports`, `uses_api`, `documented_by`,
`example_of`, and `tested_by`.

`semantic-graph-turbo-artifact-events.v1.schema.json` is the schema-owned event
stream between ASP's Rust SQLite cache and graph-turbo timeline audit. It
records compact artifact events for command, prompt-output, search, query,
search-output, and tree-sitter-query artifacts without storing provider stdout
or source windows. `semantic-graph-turbo-artifact-timeline.v1.schema.json` is
the matching audit report contract. It owns session, microburst, repeat,
fanout, action-summary, and efficiency-estimate fields so `asp search history
audit` can use SQLite-indexed events for speed while graph-turbo remains the
ranking and timeline algorithm owner.

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
`schemas/semantic-search-packet.v1.schema.json`,
`schemas/semantic-source-location.v1.schema.json`, and
`schemas/semantic-tree-sitter-provenance.v1.schema.json`. Language-specific schemas stay
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
ts-harness search prime --json .
ts-harness search prime packages/core --json .
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
current provider path context. Dependency-prefixed or external-version API
queries require a separate docs/API source and must not present current project
parser facts as dependency-version documentation.
`search/public-external-types` projects TypeScript parser-owned public type
surfaces that expose a dependency package. Direct import-type text is confirmed;
owner-level external import plus unbound type text is marked possible until the
provider exposes named import binding attribution.

The Rust slice emits the same envelope from `rs-harness search ... --json`,
including Cargo, owner, dependency, symbol, callsite, import, cfg, pattern,
docs, api, public-external-types, tests, and ingest views.

The current Python slice emits conforming packets from:

```shell
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
