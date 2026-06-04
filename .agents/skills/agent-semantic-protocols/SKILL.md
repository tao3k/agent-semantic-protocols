---
name: agent-semantic-protocols
description: Use when working with the language provider binaries maintained by agent-semantic-protocols, including rs-harness, ts-harness, py-harness, asp hook installs, compact semantic search flow, and non-JSON agent command guidance.
---

# Agent Semantic Protocols

## Rules

- Use the protocol language facade for agent exploration: Rust uses `asp rust`, TypeScript uses `asp typescript`, Python uses `asp python`.
- Use `asp` for the agent semantic client/backend surface: `asp guide`, `asp doctor`, `asp providers`, `asp cache status`, `asp cache import`, and `asp cache invalidate`. The full client/backend name is agent-semantic-client; `agent-semantic-protocol` and `agent-semantic-hook` keep their protocol/runtime names.
- Treat `rs-harness`, `ts-harness`, and `py-harness` as provider implementation/debug binaries, not the default agent command surface.
- Julia is intentionally skipped for bin parity and cross-language search alignment for now. Do not require, install, document, or validate a standalone `julia-project-harness` binary.
- If Julia evidence is explicitly requested, use the workspace-managed command `julia --project=languages/JuliaLangProjectHarness.jl languages/JuliaLangProjectHarness.jl/bin/julia-project-harness.jl`; otherwise skip Julia in provider parity audits.
- Start with `asp <language> agent guide .` when unsure; it prints the provider-owned command menu through the shared facade.
- Do not add `--json` during agent exploration. `--json` is only for schema tests, validators, receipts, or IDE integrations.
- `asp hook install --client codex .` installs or verifies the shared `asp` binary on PATH, then installs hooks, provider activation, and this skill at `.agents/skills/agent-semantic-protocols/SKILL.md`.
- `search --view seeds` graph rendering is shared protocol output. Rust, TypeScript, and Python providers should build canonical packets and shell out to `asp graph render`; if graph rendering fails, fix PATH or `SEMANTIC_AGENT_PROTOCOL_BIN` instead of adding provider-local renderer fallback.
- agent semantic client-backend phase 1 probes the local SQLite client DB before local native provider execution. It must not invent semantic facts or hide cache state: `asp cache status` reports manifest/DB health as `missing`, `unimported`, `available`, `invalid`, or `unavailable`; request receipts use `cacheStatus=miss|warm-provider|hit|stale`.
- Successful replay-safe `asp <language> search ... --view seeds` requests can write back schema-valid `search/*.json` artifacts when the provider can export the matching search packet, with prompt-output writeback as fallback; schema-valid `query/owner-items` query packets can write back and replay query artifacts when the provider can export the matching packet.

## Command Shapes

- Map the project: `asp <language> search prime --view seeds .`
- Resolve an owner: `asp <language> search owner <owner-path> --view seeds .`
- Search local fuzzy text with tests: `asp <language> search fzf <term> owner tests --view seeds .`
- Search external API/deps: `asp <language> search deps <dep[/subpath][@version][::api]> .`
- Query parser items with compact code: `asp <language> search owner <path> items --query '<symbol-or-a|b|c>' .`
- Discover owner-local item names before code: `asp <language> query <path> --term <candidate> --names-only .`
- Follow a hook exact direct-read route: `asp <language> query --from-hook direct-source-read --selector <path[:line-range]> [--code] .`
- Follow a hook wildcard direct-read route: `asp <language> query --from-hook direct-source-read --selector <glob-or-path> --term <term> --surface owners,tests --view seeds .`
- Pipe candidate lines: `rg -n '<term>' src tests | asp <language> search ingest --view seeds .`
- Check changed work: `asp <language> check --changed .`
- Verify a parser-owned mechanical edit intent: `asp <language> ast-patch dry-run --packet <semantic-ast-patch.json> .`
- Build Rust evidence graphs: `asp rust evidence graph --review-packet-json <path> --json .`
- Inspect the agent semantic client: `asp guide`, `asp doctor`, `asp providers`, `asp cache status`, `asp cache import`, `asp cache invalidate`.
- Route through agent semantic client local-native/provider-cache execution with the language facade: `asp <language> search <provider-search-args> [PROJECT_ROOT]`, `asp <language> query <provider-query-args> [PROJECT_ROOT]`, and `asp <language> check <provider-check-args> [PROJECT_ROOT]`.
- Do not use or document top-level `asp search|query|check --language ...`; it is not a public command surface.

Hook rule of thumb: source-suffix reads and content dumps are denied; exact
source paths should follow the protocol facade `query --from-hook direct-source-read
--selector <path[:line-range]> [--code] .` route; raw
`rg`/`grep`/`fd`/`find` with a concrete source term should follow the hook
`query --from-hook direct-source-read` route; source file listings without terms
should pipe candidates to `search ingest`; non-source docs/README/markdown
searches should be allowed.
Exact direct-read may return either `read-owner` source windows or a `read-plan`
with `code=false`. When it returns `read-plan`, follow the provider-selected
frontier/window read locators or search repair action instead of forcing a raw
source dump or manually scanning overlapping line windows.

Use `asp <language> ast-patch dry-run` before large
mechanical edits when the change is structural and repetitive: deleting many
matching statements/items, replacing a bounded family of statements/items, or
inserting or removing imports across a known owner set. The packet must come
from parser-owned query/search evidence, use a `path:start:end` target read
locator, and set `operation.allowLargeMechanicalEdit=true`,
`operation.maxEdits`, `operation.mechanicalKind`, and
`operation.expectedSnippet`. Treat a verified receipt as permission to proceed
with the planned Codex `apply_patch` or a provider-native AST dry-run; it is not
permission to mutate files directly in the Codex adapter. The top-level
`asp ast-patch dry-run` command is only the protocol-level
receipt validator when no provider parser is needed.

Do not use `ast-patch` for free-form text rewriting, formatting churn, prose
edits, generated file refreshes, or uncertain targets. If the receipt reports
`status=failed`, revise the query/locator or read the exact source window; do
not fall back to broad raw source reads or regex replacement.

When the provider guide advertises handle-aware search, use it before code for
stable non-code facts such as policy rule ids, schema fixtures, test cases,
config keys, command surfaces, dependency APIs, or capabilities:
`asp <language> search policy <rule-id-or-alias> owner tests --view seeds .` or
`asp <language> search owner <owner-path> handles --query <term> .`

Owner item query output includes a `|query` line with `status=hit|miss` and
`match=exact|fallback-contains|none`. Treat `status=miss` as a wrong or stale
symbol query to revise, not as permission to keep raw-searching the file. If a
miss line includes `candidates=...`, follow the parser-owned candidate instead
of guessing another symbol. Use `--names-only` for broad owner-local prefixes
such as `parse_` so the provider returns item names and read locators without
dumping code windows.

When `searchSynthesis.windowSet` appears, treat it as the provider-selected
bounded read plan. Read those exact owner/test targets with
the protocol facade `query --from-hook direct-source-read --selector <path[:line-range]> [--code] .` route
only when source owner context is needed; do not restart broad discovery from
the same terms.

Rust owner and ingest accept extra scopes:

```sh
asp rust search owner src/lib.rs items --view seeds .
rg -n 'HookDecision' src tests | asp rust search ingest items tests --view seeds .
```

Julia is skipped in bin parity audits. Only use the workspace-managed provider
command when a Julia-specific task explicitly asks for Julia evidence:

```sh
julia --project=languages/JuliaLangProjectHarness.jl languages/JuliaLangProjectHarness.jl/bin/julia-project-harness.jl search owner src/cli.jl --view seeds languages/JuliaLangProjectHarness.jl
printf 'src/cli.jl:1\n' | julia --project=languages/JuliaLangProjectHarness.jl languages/JuliaLangProjectHarness.jl/bin/julia-project-harness.jl search ingest owner tests --view seeds languages/JuliaLangProjectHarness.jl
```

## Flow Examples

1. Implement a TypeScript feature around a known symbol:

```sh
asp typescript agent guide .
asp typescript search prime --view seeds .
asp typescript search fzf runCodexAgentHook owner tests --view seeds .
asp typescript search owner src/cli/agent-hooks.ts --view seeds .
asp typescript check --changed .
```

Use the `owner` output to choose the edit file. Use the `tests` seeds before editing.

2. Find Rust API usage before changing behavior:

```sh
asp rust agent guide .
asp rust search deps tokio::spawn public-api --view seeds .
asp rust search fzf tokio::spawn tests --view seeds .
asp rust search owner src/runtime.rs items --query RuntimeConfig .
```

Use `deps` for external API facts. Use `owner` only after a real owner path appears.
Use provider-native `search owner <path> items --query <symbol>` for compact code extraction.
Do not use raw `cat`, `sed`, `rtk read`, or editor reads for source files.

3. Understand a Python implementation path:

```sh
asp python agent guide .
asp python search prime --view seeds .
rg -n 'Session' src tests | asp python search ingest --view seeds .
asp python search owner src/client.py --view seeds .
asp python search owner src/client.py items --query 'Session|request' .
asp python check --changed .
```

Use `rg` or `fd` to collect candidates, then let `py-harness` rank owners/tests.

Julia is out of scope for bin-parity flows. Do not use a bare
`julia-project-harness` command in docs, tests, hook activations, or provider
parity audits. Julia remains workspace-managed until a separate Julia-specific
lane is reopened.

## Combination Query Examples

1. Same TypeScript question, multiple names:

```sh
asp typescript search fzf --query-set runCodexAgentHook --query-set permissionDecision owner tests --view seeds .
```

Use this when both terms describe the same hook decision path.
For a hook-blocked wildcard read such as `Read *.ts`, use the hook query form
when the selector is broad and the agent has concrete terms:

```sh
asp <language> query --from-hook direct-source-read --selector '**/*.{ts,tsx,js}' --term parseSearchArgs --term querySets --surface owners,tests --view seeds .
```

This emits normal search seeds and synthesis; it is not a raw source-read
fallback.

2. Same Rust concept, type plus field:

```sh
asp rust search fzf --query-set HookDecision --query-set permissionDecision owner tests --view seeds .
```

Use this when one answer should cover both aliases. Follow with `asp rust search owner <path> items --query '<symbol|otherSymbol>' .` when a concrete owner is selected.

3. Same Python API, import name plus method name:

```sh
asp python search fzf --query-set requests.Session --query-set Session.request owner tests --view seeds .
```

Use this when API naming varies across imports/callsites.

Do not use query-set for independent axes. Run these separately and synthesize after reading compact outputs:

```sh
asp typescript search deps playwright::APIRequestContext .
asp typescript search owner src/http/client.ts --view seeds .
asp typescript search tests src/http/client.ts --view seeds .
```
