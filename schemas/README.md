<!--
SPDX-FileCopyrightText: 2026 tao3k team and Contributors
SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later
-->

# Semantic Search Schemas

Policy boundary: ASP EvidenceGraph, DynamicTopology, and GraphRoute own least-search target localization and decide where to edit through stable selectors, owners, tests, dependency edges, snapshots, benchmarks, and topology context. Language policy receives that target context and tells the agent how to edit and validate safely; it does not own broad search or primary file selection.
`semantic-where-frame.v1.schema.json`, `semantic-dynamic-topology.v1.schema.json`, and `semantic-how-frame.v1.schema.json` are the prompt-facing frame contracts for the where/how loop. WhereFrame localizes the edit boundary and exposes the next smallest search/query action. DynamicTopology projects the current architecture slice, including owner edges, pressure, trust boundary, stale facts, and branch legality. HowFrame consumes the localized boundary plus topology and policy/scenario facts to tell the agent how to edit, which branches are illegal, and which parser/schema/scenario/snapshot/benchmark/test evidence should validate the edit. These frames should carry ids and compact facts, not pasted source or line-number identity.
`semantic-policy-fact.v1.schema.json` and `semantic-policy-recipe.v1.schema.json` are the runtime policy layer between the software criterion canon and agent reasoning. Policy facts say which criterion/profile/ecosystem claims are active for a workspace, package, owner, selector, entity, dependency, test, artifact, scenario, profile, or session. Policy recipes say how an agent should improve, repair, test, document, or suppress a route when those facts are visible. They are evidence-backed and selector/entity/generation/overlay scoped; path/line data is display-only and must not become executable identity.
Scenario benchmark evidence and behavior snapshot evidence are first-class policy evidence kinds: benchmarks project route quality, budget, dependency usage under the current architecture, resource pressure, failure mode, and expected resolution facts, while snapshots project observable behavior, output contract, and compact-render stability facts. Dependency-oriented scenarios are not dependency tutorials; they are benchmark-backed architecture facts that connect documentation, upstream source, research notes, benchmark artifacts, topology context, and verification receipts to the way this project should use the dependency.
Normal agent rendering should expose these facts through a progressive `QualityFrame`: a small read-model for one intent and candidate edit boundary, with architecture pressure, decision rules, change shape, guardrails, and validation shown first and exact fact/recipe/snapshot/benchmark references expanded only on demand.
Gerbil-style language policy evidence extends the same layer with runtime-source, compiler-evidence, language-rule, standard-library, and macro-pattern evidence kinds. These let a provider prove macro boundaries, active runtime constraints, medium-weight compiler evidence, module import rules, and native style policy without turning provider-specific details into root schema fields.
Rust search-quality policy and Gerbil language-policy evidence are the first two mature practice cases for a single shared obligation: ASP EvidenceGraph, DynamicTopology, and GraphRoute decide where to edit through least-search target localization, while every ASP language provider policy receives that target context and tells the agent how to edit and validate safely. Shared policy facts can now represent edit strategy, cross-language capability, topology context, graph-route evidence as target context, topology updates, provider capabilities, prior failures, and plan-selection or architecture-context use. Providers should map their strongest native facts into these shared families so the agent receives a compact QualityFrame while coding, not a full policy catalog.
`semantic-task-frame-archive.v1.schema.json` owns the machine-readable archive packet for task/feature-scoped reasoning state. It records the completed, suspended, superseded, or handed-off TaskFrame plus the WhereFrame/HowFrame references, scope, edit groups, observations, promoted facts, and ephemeral evidence that must not cross task boundaries. The matching Org archive is the human-auditable and `asp query playbook --documents org` surface; the JSON packet is the replayable machine state for validation, project evidence promotion, and downstream Python/Julia analysis. Together they form the artifact lattice for long-running agent work: `taskFrameId`, `intentId`, `featureId`, `evidenceGeneration`, parser selectors, scenario ids, snapshot ids, benchmark ids, receipt ids, and artifact ids connect EvidenceGraph, topology Org, policy/scenario facts, observations, and archives without forcing those payloads into the prompt. Search/query commands must not create these archives as hidden side effects; an explicit archive action closes or hands off a TaskFrame.

`semantic-proof-obligation.v1.schema.json`, `semantic-proof-recipe.v1.schema.json`,
`semantic-proof-receipt.v1.schema.json`, and
`semantic-formal-verification-report.v1.schema.json` own the formal-verification
artifact loop for proof-backed branch legality. Proof obligations are
EvidenceGraph/DynamicTopology-owned questions about stable invariants; proof
recipes are Policy/Scenarios-owned instructions for how to verify them; proof
receipts are compact checker results that can update branch legality; formal
verification reports expand receipts for audit and replay. Lean/AXLE is one
executor for this loop, not the owner of the protocol. The agent-facing surface
should normally read the receipt or HowFrame summary first and expand to Lean
proof artifacts only when debugging the proof lane.

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

`semantic-agent-search-dispatch-receipt.v1.schema.json` owns the language-neutral
resident dispatch lifecycle. It joins one exact command digest to the verified
root, child, and message target; records monotonic dispatch state; marks resident
execution terminal and non-redispatchable; and requires completed receipts to
carry a materialized owner selector plus a placeholder-free executable command.

`search-topology-settlement.v1.schema.json` owns the current single public
Search Playbook result while the V1 contract is being stabilized. Runtime
joins the Agent-authored acquisition evidence to the exact attached Project
Topology generation, admits the binding, proof, coverage, fixed-point, and
selector-ownership invariants, and returns that typed settlement. The Client
invokes its canonical renderer, producing exactly one Org-owned GQL source
block. Flat evidence rows, duplicate materialization sets, planner state, Query
grammar, recommended commands, and parallel renderers are not public Search
results.
`project-topology-library.v1.schema.json` owns the reusable topology generation
that supplies those settlements. It includes
source-owned expected relations, relation-scoped coverage certificates, and an
exact frontier for every unresolved expectation; a positive factual or derived
edge and a frontier for the same relation endpoints are mutually exclusive.
`project-topology-inference-receipt.v1.schema.json` owns the corresponding
relation-preserving MRR/Ascent receipt, including typed rule identities and
premise witnesses. These contracts remain V1 during the current stabilization and
refactoring phase; no duplicate V2 family is published.
`--syntax` carries a registered producer and its provider-native query argv;
`--native-syntax` is a separate exact-selector axis and cannot carry a
Tree-sitter query.

`runtime-search-execution-budget.v1.schema.json` owns the generation-bound
cardinality receipt used by Search Playbook execution. Runtime derives its
limits from admitted owner/corpus/graph cardinalities and the public V1 graph
envelope. It is not an Agent planner input and contains no Tokio task-count or
concurrency control.

`runtime-resident-request-plane-receipt.v1.schema.json` owns the strict
sub-millisecond Search/Query request-plane receipt. It binds cold/warm lookup
state and generation identity to one measured elapsed time and requires one
resident lookup with zero waits, builds, filesystem/database reads, provider or
parser work, secondary Runtime RPCs, socket discovery, and terminal waits.

`project-resolution.v1.schema.json` owns provider-resolved package-manager
workspace membership and source scope. Search and graph consumers use the
ASP-projected source index from that receipt.
Document language providers such as `org` and `md` use a document-specific
Query packet. `semantic-document-query-packet.v1.schema.json` owns metadata
and filtered element projections, with explicit `queryKind`, `querySurface`,
and `contentBlocks` fields. Document discovery enters the same public
`search playbook` operation as source discovery; exact document
materialization consumes a returned selector through `query --selector`.
Providers must not report document facts through source-language
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
For tree-sitter-backed exact query rendering, `--projection source` returns
parser-authoritative source and `--projection callable-skeleton` returns the
bounded callable structure. Query
render profiles such as the `compact-graph-frontier` profile and
`corpus-locator` profile project an ASP-compiled tree-sitter query plan over
provider-native projection; they do not introduce a new packet surface.
RFC 009 adds optional `reasoningProfiles` as typed internal planning facts.
They are projected only through the single public `search playbook` receipt;
they are not independently addressable command routes.

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
`semantic-structural-index.v1.schema.json` is the local client cache export
shape for parser-owned owner, symbol, file-hash, and dependency-usage rows. It
exists to make warm search replay index-first without storing source code:
providers emit names, locators, dependency identities, query keys, and freshness
hashes; the Rust client owns Turso storage, invalidation, and replay
eligibility. Lightweight provider interfaces may leave workspace-wide
`symbols`, `dependencyUsages`, and `syntaxFacts` empty on the hot path while
still emitting parser-owned totals such as `symbolTotal`,
`dependencyUsageTotal`, and `nativeSyntaxFactTotal`. ASP Rust then fans out to
owner fact commands or explicit artifacts for full row materialization,
caching, and graph construction.
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

`agent-semantic-client-config.v1.schema.json`,
`agent-semantic-client-cache-manifest.v1.schema.json`, and
`agent-semantic-client-receipt.v1.schema.json` own the agent semantic client/backend
envelope. They describe route mode, provider set, privacy policy, cache
generation provenance, Turso client DB status, execution route, provider
command counts, and native provider provenance. They do not duplicate
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
DB Engine imports from a validated provider-owned manifest. Manifest re-imports
must preserve unrelated normalized row families rather than replacing the parent
generation in a way that cascades syntax rows away. `cache-invalidate`
receipts describe
local DB Engine generation-row invalidation and do not imply manifest or artifact
deletion. `cache flush syntax-rows` deletes only normalized syntax query row
families and preserves manifest generations plus artifact provenance. In
local-native receipts, `warm-provider`
means a matching DB Engine generation was found but provider execution still
supplied the output; only `hit` means the client served output from cache. The
replay surface covers identity-bound provider `prompt-output/*.txt` artifacts
and `query/*.json` semantic-query-packet artifacts for exact Query replay under
the protocol artifact root.
`semantic-tree-sitter-query/*.json` artifacts and normalized syntax rows replay
only through AST/ABI fingerprints plus freshness hashes. Syntax-query receipts
surface the AST/ABI fingerprint, grammar id, grammar profile version, and
selector when present; artifact ids remain provenance rather than cache facts.
Providers may supply
`/cache/fileHashes`; when they do not, the client may hash validated syntax
locator paths from the packet, storing only path+sha256 and no raw source.
The client may cache only a typed, identity-bound `search playbook` receipt for
an identical request. Legacy prompt stdout and provider-command replay are not
search authorities. Exact query projections remain separately bound to their
canonical selector and generation.
`semantic-search-stage-receipt.v1.schema.json` owns search/router stage receipts
such as `search-candidate-merge`. It records route sources, candidate counts,
line-identity filtering, fallback reason, and optional latency/proof-gain fields
for Python graph-route ranking and Julia replay without embedding source text.
Search-owned query-wrapper trace projections carry the same fields into compact
agent-facing source traces; command renderers only forward those projections.
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
schema-owned way to opt hidden directories into the ASP repository candidate
snapshot. The ASP facade applies activation-root config first and
invocation-root config second; list assignments are normalized replacements,
not prompt-time merges. Retired `[search] ignoreDirs/includeHiddenDirs` input
is unsupported. ASP applies normalized discovery policy once before
dispatching candidate-bounded project-resolution requests; source-language
providers MUST NOT maintain or reapply private default ignore lists to those
candidates. Built-in
document providers are enabled by default and require no activation entry, but
they still honor `providers.org.enabled=false` and
`providers.md.enabled=false`. Hook activation also consumes
`providers.<language>.enabled` to disable external providers and
`providers.<language>.binary` to pin external provider executables.
Provider-specific policy config may stay in language-owned files, but source
discovery and provider selection must not silently diverge from the nearest
`asp.toml`.

`runtime-dev-config.v1.schema.json` owns only the State Home
`$ASP_STATE_HOME/asp.toml` `[dev]` table. Its canonical shape is
`enabled = true` plus one absolute `root`. It is not part of project-local
`.agents/asp.toml`, does not define a scope enum, and cannot authorize release
locks or `PATH` artifacts while enabled.
The same config owns extension activation under `extensions.*`. The CodeQL
extension is default off and default experimental:
`extensions.codeql.enabled=false`, `extensions.codeql.experimental=true`, and
`extensions.codeql.mode="disabled"`. Enabling it changes only explicit
extension/evidence paths unless a cache-only artifact is already available; the
schema keeps `extensions.codeql.allowHotPath=false` so ordinary search, query,
hook recovery, and dependency-owned policy evaluation cannot be configured to create CodeQL
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
on retired lifecycle waiver/task objects.

`semantic-evidence-graph.v1.schema.json` is the shared portable graph artifact
over reviewer evidence. P6.1 uses it to link review packets, invariant
candidates, receipts, behavior snapshots, determinism readiness summaries,
proof pilots, waivers, and review actions as explicit nodes and edges. It is an
artifact contract, not a database or long-lived storage layer. Language
providers emit only syntax/parser facts. The immutable Runtime generation uses
the shared MRR Ascent/GQL program to derive this graph, reviewers can inspect
it, and later assurance-case renderers can consume it without inventing a new
evidence vocabulary. `semantic-evidence-graph-derivation-receipt.v1.schema.json`
binds the graph digest to that source generation, the canonical fact digest,
and the exact GQL plan digest for every admitted rule.

`semantic-assurance-case.v1.schema.json` is the shared reviewer-first assurance
artifact derived from an evidence graph. P6.2 uses it to turn graph nodes and
edges into claims, supporting evidence references, review actions, stale waiver
references, and open gaps. It deliberately keeps references by graph node id
instead of embedding another full graph, so assurance rendering stays portable
without becoming a storage or visualization layer.

`semantic-query-packet.v1.schema.json` is the provider-to-Runtime data-plane
contract for parser-owned facts. It is not a public provider CLI. Public
discovery uses only `asp search playbook '<scheme-expression>'`: its
`producers`, `rg`, `tantivy`, `syntax`, and optional `graph` forms establish
the shared file and structural context. Exact
materialization uses only `asp query playbook` with the matching producer-axis
declaration, canonical `--selector`, and `--projection
<source|callable-skeleton>`. Root hooks must use those two public operations
and must not maintain a parallel read/query engine.
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
still rendered through frontier-first packets and exact
`--projection source|callable-skeleton` follow-up selectors.
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
an agent asks for an exact source projection.

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

Agent-facing syntax search stdout has a separate render contract from exact
query projection: search output is locator/frontier evidence, while exact
query accepts only the typed `source` and `callable-skeleton` projections.
Rust currently renders a graph-rendered
locator-frontier profile, and TypeScript/Python render the `corpus-locator`
profile. These are render profiles over frontier facts, not "compact frontier"
protocols. Both profiles are valid only when backed by the same ASP-compiled
tree-sitter query plan and provider-native projection, and neither profile may
expose cache ids, DB paths, receipts, full node lists, or raw source windows
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
docs, comments, document headings, properties, drawers, tables, blocks, links,
code fences, includes, fields, bindings, constants, arguments, and macros; provider specific
syntax remains in `languageKind` and `fields`. Rust, TypeScript, Python, Julia,
Org, Markdown, and future providers own their concrete fact builders and
provider-local schema refinements. Search and query packets may embed these
facts as optional `nativeSyntaxFacts`.

`semantic-finder-tools.v1.schema.json` is the shared contract for internal
provider-approved acquisition stages used by `search playbook`. It describes
tool catalogs and pipelines such as `rg+lexical` without exposing stage argv as
public Agent commands. The language provider owns path normalization, owner
resolution, nearest-item resolution, test frontier selection, deduplication,
caps, and packet rendering.

`semantic-sandtable-scenario.v1.schema.json` is the shared scenario descriptor
for replaying bounded search flows against real harness binaries. It owns the
portable drill shape: workdir selection, argv commands, stdin pipe commands,
regex capture handoff, line-protocol expectations, and warning budgets for
token-size and latency findings. Scenario descriptors can also carry compact
real-trigger `evidence` metadata for recorded agent exploration loops, including
the launch intent, edit-stop boundary, receipt path, recorded metrics,
repeated-search findings, and query-set merge opportunities. Hook replay steps
may use `expect.guideQuality` to assert that a denial includes the reason kind,
language route, safe Search Playbook command shape, and no leaked source text;
guide-quality output assertions can require or forbid exact output and route
command text. JSON stdout expectations can assert exact paths, substring containment,
schema conformance, and array membership with scalar values or object subsets.
Large-library calibration scenarios use typed `evidence.targetLibrary`,
`evidence.fixtureTier`, and `evidence.intentCases` metadata so every provider
can publish the same feature/API/principle search matrix without asking the
harness to parse natural-language intent. Coverage audits render this as
`|intent-matrix` and `|intent-library` lines, and `--fail-on-missing` treats
missing large-library rows or missing intent cases as coverage failures.
`intentCases[].queryTerms` records which query-set terms exercise each intent
when several same-view probes are compressed into one scenario step.
Agent SDK replay expectations can use `expect.commandFlow` read-loop budgets to
bound direct-code reads, duplicate selectors, adjacent range windows, and
same-owner scans; `forbiddenStages` can also reject the aggregate
`read-loop-risk` stage.
Failure-frontier replay gates use
`evidence.failureFrontierComparison` to compare a baseline receipt/trace with a
candidate receipt/trace. The candidate must prove command reduction, bounded
exact source-projection count, zero duplicate selectors, zero same-file
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

`semantic-agent-hook-provider-manifest.v1.schema.json` and
`hook-activation.v2.schema.json` are immutable archive contracts for the
retired manifest/activation routing architecture. Current Hook compilation and
classification do not consume either document. Provider policy identity is
compiled from the managed Hook profile projection; provider execution and
generation readiness belong to Runtime. These versioned schemas remain only so
historical artifacts can be identified and validated without mutating an
already published contract. They must not be used to reconstruct a
compatibility route.

`activation-admission-receipt.v1.schema.json` is likewise an immutable archive
contract. Runtime artifact-slot admission and generation-bound provider
execution receipts replace it in the current architecture.

`semantic-agent-runtime-profiles.v1.schema.json` is the archive shape for the
retired derived runtime-profile file. Current execution consumes the active
provider set and artifact execution closure published under Runtime-owned
generation slots. Hook skill rendering and Hook classification do not read a
runtime profile.

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
belong in manifests, DB Engine rows, receipts, and artifacts under the shared git
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
decisions may also include `fields.toolSurface`, `fields.operationIntent`,
`fields.aspCommandIntent`, and `fields.aspCommandRoute` so
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

`semantic-read-packet.v1.schema.json` is the provider-owned historical packet
for bounded source windows. It is not a root hook command surface and does not
define the current exact-query CLI. The current exact-query contract uses a
canonical structural selector plus `--projection source|callable-skeleton`.
The packet records parser-owned selection evidence:
project-relative selectors or source locators, owner paths, optional item facts,
bounded source-preserved line windows, truncation state, and notes. Exact
exact source projections must not be reconstructed from lossy search
projection rows; projection may select or repair a frontier, but `sourceWindows`
text is source/formatter-preserved for the bounded selector. When a selector is
broad or low-signal, providers should emit `readPlan` with `code=false`,
`mode=range-frontier`, executable `frontier` entries, bounded `windows`, and
`avoid` actions instead of `sourceWindows`; broad discovery still stays in
Search Playbook discovery or exact query repair.

ASP separates search projection from exact query projection. Search is
source-free locator/frontier evidence. Exact query accepts only
`--projection source|callable-skeleton`; JSON is an explicit diagnostic or
machine-consumer representation, never an implicit replacement for source
projection. The removed `code` flag and direct-read recovery surface are not
valid aliases.

`query-playbook-materialization-request.v1.schema.json` binds one caller-ordered,
unique selector sequence to one immutable Runtime execution identity without a
Search-settlement or Project Topology handle. Query never sorts that sequence.
`query-playbook-materialization-receipt.v1.schema.json` is its all-or-nothing
terminal: Ready returns every requested selector exactly once and in request
order under the requested projection, while Failed returns no materialized
bytes. Search owns topology and GQL relationships; the Query receipt contains
neither those fields nor planner, recommendation, or explanation fields.

`runtime-execution-binding.v2.schema.json` is the current Runtime identity
product used by Search and Query. It binds the complete parser-admitted
`projectWorkspace`, the host-local `worktreeInstanceId`, one publication nonce,
the content binding, Runtime artifact, evaluator policy, active publication
receipt, and evaluator ABI. The former V1 `projectId`/`workspaceId` pair is not
a workspace authority and must not be accepted or reconstructed by current
clients.

When a direct read must distinguish worktree, staged index, and committed
contents, the same packet carries `sourceVersion=worktree|index|head`.
Providers should set `repositoryRoot` when the Git repository root differs from
`projectRoot`, such as a nested ASP language provider repo. `gitBlobOid` identifies
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

`asp-client-workspace-search-playbook-request.v1.schema.json` remains the
normalized northbound request. The MRR repository owns the
`defsearch-playbook` macro, POO Flow lowering, and importable AOT SCM alongside
the native Rust Ascent/GQL crates. `agent-semantic-search` owns the short configuration and
native Scheme/query-table admission. Neither Scheme source nor a private POO
IR is added to Client Protocol; the former flag-shaped CLI is removed rather
than retained as a compatibility path.

`enhanced-tree-sitter-query-operator-table.v1.schema.json` defines the
language-neutral MRR Gerbil AOT operator declaration for the namespaced
`#asp-*` predicates and result directive.
`enhanced-tree-sitter-query-capability-table.v1.schema.json` defines each
provider's parser/grammar-bound publication and equivalence rows.
`resident-syntax-query-plan.v1.schema.json` is the complete normalized plan
executed over one active generation. Search and direct Syntax requests carry
that plan rather than Tree-sitter Query source or provider argv. These are
stable V1 contracts completed as one hard cut; no V2 or compatibility route is
created.
`asp-client-workspace-syntax-plan-context-request.v1.schema.json` and
`asp-client-workspace-syntax-plan-context-response.v1.schema.json` own the
constant-time resident context read. The response supplies only the exact
generation digest and provider capability; the Client performs source parsing
and plan compilation locally before execution.

`search-playbook-pretool-calibration.v1.schema.json` remains the unchanged
denial contract. The Scheme parser supplies typed expression issues at the
outer argv token index. Arrays requiring flag/argv-leaf boundaries are empty
for Scheme source, rather than populated with fabricated inner indices. No V2
receipt or alternate public flag grammar is introduced.

The public Search result is `workspace-search-playbook-result.v1`; Runtime joins it to `search-topology-settlement.v1` before the Client renders the single Org/GQL projection. Provider-native exact materialization remains `semantic-query-packet.v1`.

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

The Runtime may combine repeated same-axis terms inside one internal query-set,
but query-set packets are not a public command batch surface. The public input
remains one `search playbook` request whose native rg, structured Tantivy,
producer, parser, and optional Graph blocks are explicit. Provider acquisition,
lexical recall, parser projection, and graph ranking remain ordered internal
stages.

Provider results must preserve the meaning and provenance of each matched term.
A fixture string that resembles a source path is classified as fixture evidence,
not promoted to a real owner. Graph-derived planning facts may rank exact owner
and selector evidence, but they cannot emit another search command. The
agent-facing receipt returns bounded evidence and canonical selectors; the only
follow-up that materializes source is `query --selector ... --projection ...`.

`semantic-graph-resident-evaluation-request.v1.schema.json` is the intent-only
northbound graph request for `asp.graph.evaluate`. It contains no graph,
source-snapshot, workspace-generation, provider, cache, or algorithm identity.
The Runtime resolves those identities from the admitted Ready session.
`semantic-graph-resident-evaluation-result.v1.schema.json` binds the returned
ranked nodes and traversed edges to that exact resident generation and proves
query-time provider RPC, durable reads, and generation mutation are zero.

`semantic-graph-turbo-request.v1.schema.json` is the schema-owned algorithm
input packet for the cold/offline `asp-python-graphs` service project. It never
serves the Ready `search`, `query`, or `asp.graph.evaluate` routes. It carries
the requested reasoning profile, algorithm id, seed node ids, ranking budget,
optional per-kind budgets, optional window-merge controls, and typed graph
facts under `graph.nodes[]` and `graph.edges[]`. Internal playbook request nodes may
carry parser-owned `syntaxQuery` locators for candidate symbols, hot range
nodes for direct code follow-ups, and dependency package nodes connected by
`owner -> dependency` import edges for query-deps routing. Internal playbook
packets may also carry `actionFrontier[]`: typed action facts with action id, kind, capability id,
target, target role, and fields such as selector, owner path, query, query
clauses, scope, recipe, or names. These action facts are materializer input for
display-only `nextCommand` text and intentionally reject materialized `command`
or `argv` fields. This packet is never an Agent-facing CLI spelling.
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
the ASP Server `asp.graphs.sandtable-summary` method consumes a calibration-ready benchmark
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
stream between ASP's Rust DB Engine cache and graph-turbo timeline analysis. It
records compact artifact events for command, prompt-output, Search Playbook,
Query Playbook, and tree-sitter artifacts without storing provider stdout or
source windows. `semantic-graph-turbo-artifact-timeline.v1.schema.json` is the
matching internal report contract; it does not create another Search command.

Large-library packets should keep source and runtime limits explicit instead
of forcing the agent to discover them through repeated commands.
`sourceCoverage` reports whether the selected package root or config made the
expected source owners parser-visible. `testResolution` reports whether a
tests search linked, missed, or noisily found tests for an owner. `runtimeCost`
reports coarse cache and parser reuse facts such as `cacheStatus`, `elapsedMs`,
`sourceFilesParsed`, and `parserFactsReused`. These fields are evidence for
follow-up search planning; provider-specific compiler details still belong in
`fields`.

`search playbook` parses an explicit option grammar. A query term that begins
with `-` must be supplied with `--query`; legacy view, seed, query-set, owner,
and pipe options are rejected instead of being reinterpreted.

This repository's `schemas/` directory is the protocol source of truth.
It contains common protocol schemas only. Provider packages that run CI from
independent checkouts should carry package-local copies of those common schemas
at the same relative paths, for example
`schemas/search-topology-settlement.v1.schema.json`,
`schemas/semantic-source-location.v1.schema.json`, and
`schemas/semantic-tree-sitter-provenance.v1.schema.json`. Language-specific schemas stay
inside the ASP language provider repository, for example the TypeScript provider's
`schemas/typescript-semantic-capabilities.v1.schema.json`. The protocol
repository may keep language-specific templates, such as
`schemas/typescript-semantic-capabilities-template.v1.schema.json` and
`schemas/python-semantic-capabilities-template.v1.schema.json`, to document the
expected active schema shape without making the common registry schema own a
global capability enum. The TypeScript harness unit suite reads its
package-local common schema copies, validates every implemented
`asp-typescript search ... --json` view against the shared envelope, checks
`asp-typescript agent doctor --json` against the common registry contract, checks
TypeScript descriptor capabilities against the TypeScript-local schema, compares
common package-local copies with this repository's source schemas when the
package is checked out as a submodule, and compares the TypeScript-local
capability vocabulary with the protocol repository template when that template
is available.
The Python harness follows the same ownership split: `asp-python agent doctor
--json` advertises the common registry and search packet schemas plus the
Python-local `schemas/python-semantic-capabilities.v1.schema.json`, while this
repository only keeps the template vocabulary.
The Rust harness exposes the same registry contract through
`asp-rust agent doctor --json`.

Schema evolution is versioned by file name and `schemaVersion`.
Optional fields, enum members, and method descriptors can be additive v1
changes. Renaming a field, changing field meaning, making an optional field
required, or removing an enum member is breaking and requires a new schema file
such as a new versioned semantic search packet schema. Provider packages must update
their package-local copies and sync tests in the same change that advertises a
new schema version.

The current TypeScript public discovery and materialization surfaces are:

```shell
asp search playbook '(search (workspace "main") (producers (language typescript)) (intersect (rg "-n" "-F" "OrderStatus" ".") (tantivy "title:OrderStatus^2 OR body:OrderStatus")))'
asp search playbook '(search (workspace "main") (producers (language typescript)) (intersect (rg "-n" "-F" "OrderStatus" "src/index.ts") (tantivy "title:OrderStatus^2 OR body:OrderStatus")))'
asp query playbook --language typescript --selector <exact-selector> --projection source --workspace main
```

Provider-internal parser and lexical stages may resolve reasoning owners,
parser-visible modules, and existing project paths. Parser-visible modules
outside the reasoning owner graph are represented with
`fields.source=parser-visible-module`, `fields.parserOwner=false`, role/layer
metadata, line counts, validity, and diagnostic counts. Existing paths outside
the parser module set are still represented as path-only owners with
`fields.source=path-only`, `fields.parserOwner=false`, and
typed unavailable diagnostics. These facts are all projected through the one
playbook receipt. Dependency-version documentation remains a separate admitted
source and cannot be synthesized from current-workspace parser facts.

The Rust slice emits the same envelope from `asp-rust search ... --json`,
including Cargo, owner, dependency, symbol, callsite, import, cfg, pattern,
docs, api, public-external-types, tests, and ingest views.

The current Python public discovery and materialization surfaces are:

```shell
asp search playbook '(search (workspace "main") (producers (language python)) (intersect (rg "-n" "-F" "AspPythonReport" ".") (tantivy "title:AspPythonReport^2 OR body:AspPythonReport")))'
asp search playbook '(search (workspace "main") (producers (language python)) (intersect (rg "-n" "-F" "AspPythonReport" "src/asp_python/_cli.py") (tantivy "title:AspPythonReport^2 OR body:AspPythonReport")))'
asp query playbook --language python --selector <exact-selector> --projection source --workspace main
```

`runtime-selector-overlay-receipt.v1.schema.json` records a selector-only
publication against one admitted workspace generation. Exact repair binds the
owner digest and byte range without advancing or rewriting the canonical source
generation.
