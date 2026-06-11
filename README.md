# Agent Semantic Protocols

Shared protocol contracts, hook runtime, and replay sandtables for semantic
language harnesses.

This repository keeps the agent-facing surface stable across Rust, TypeScript,
Python, Julia, and future providers. Language harnesses own parser facts and
provider-specific commands; this repository owns the shared schemas, RFCs,
root hook classifier, and scenario evidence used to keep those providers
aligned.

## What Lives Here

- `rfcs/`: protocol intent and change process. Start here before changing
  search behavior or hook behavior.
- `schemas/`: shared JSON contracts for search packets, language registries,
  hook decisions, hook activations, provider manifests, sandtable scenarios,
  and receipts.
- `crates/agent-semantic-protocol/`: shared Rust CLI entrypoint for protocol
  commands such as `hook` and `ast-patch`.
- `crates/agent-semantic-hook/`: Rust root hook runtime for Codex, Claude,
  and other agent hook classification and provider routing, used by `asp hook`.
- `crates/agent-semantic-client*/`: agent semantic client/backend crates. `asp` is the
  product-facing local/cloud client surface; hook and protocol crates keep
  their `semantic-agent-*` names.
- `packages/python/src/tools/semantic_sandtable/`: sandtable runner for
  replaying real provider commands and validating scenario evidence.
- `sandtables/`: cross-language replay scenarios for search flows, hook denial
  guides, performance budgets, and real-trigger regressions.
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

- `rfcs/semantic-tree-sitter-query-protocol.org` owns the portable
  tree-sitter-compatible syntax ABI, catalog/profile/corpus layout, native
  projection boundary, and pattern-graph roadmap.
- `rfcs/cli-first-harness-ux.org` owns the agent-facing `asp <language> guide`,
  search/query/read-plan stdout contracts, and syntax locate/code flows.
- `rfcs/agent-hook-interception-protocol.org` owns hook decision packets,
  Markdown recovery prompts, `Detected Binaries`, and ast-patch config
  branching.
- `schemas/README.md` owns the schema catalog and explains how query/search/read
  packets share tree-sitter provenance without merging packet envelopes.
- `docs/30-39-research/31.18-tree-sitter-query-rfc-roadmap.org` records the
  current tree-sitter native-projection audit and the closure checklist for the
  RFC/docs pass.

## Common Commands

Enter the project shell first when available:

```sh
direnv exec . <command>
```

Install agent-facing tools and the Codex hook config through the current
convenience target:

```sh
just agent-hooks-install
just agent-hooks-doctor
```

This installs the core ASP runtime surface: `asp`, `asp-graph-turbo`,
`rs-harness`, `ts-harness`, and `py-harness`. `asp-graph-turbo` is the only
supported graph turbo executable and a required local ranking dependency for
the graph-turbo search/history path, not an optional debugging tool.

Install the same tools into a user-owned bin directory. The directory must be
on the PATH that your agent client uses to run hooks:

```sh
mkdir -p "$HOME/.local/bin"
export PATH="$HOME/.local/bin:$PATH"
just agent-hooks-install "$HOME/.local/bin"
just agent-hooks-doctor "$HOME/.local/bin"
```

Install individual agent tools when only one boundary changed:

```sh
just agent-tools-install-protocol "$HOME/.local/bin"
just agent-tools-install-asp "$HOME/.local/bin"
just agent-tools-install-asp-graph-turbo "$HOME/.local/bin"
just agent-tools-install-hook "$HOME/.local/bin"
just agent-tools-install-rust "$HOME/.local/bin"
just agent-tools-install-typescript "$HOME/.local/bin"
just agent-tools-install-python "$HOME/.local/bin"
```

Run `asp-graph-turbo` through the native ASP wrapper when an agent step needs the
ranking engine without depending on Python workspace internals:

```sh
asp wrap asp-graph-turbo -- help
asp tools wrap asp-graph-turbo -- help
```

Graph-turbo request packets use the ranking engine through schema-owned JSON:
`semantic-graph-turbo-request.v1` enters `asp-graph-turbo`, and
`semantic-graph-turbo-result.v1` or its JSON projection leaves that boundary.
The legacy compact graph renderer is a prompt/debug projection only; it is not a
trusted graph, frontier, rank, or action protocol.

Agent-facing fast search uses graph-turbo ranking by default.
`asp rust search fzf <term> owner tests .` and the explicit seeds form
`asp rust search fzf <term> owner tests --view seeds .` avoid printing the
request packet, but the trusted structure remains the schema packet and any
schema-owned JSON projection. Rank, profile, paths, scores, cache, trace,
explanations, metrics, and frontier actions must be packet-visible before any
text renderer serializes them.
Default fast-search request packets include candidate hot range nodes and
`item -> hot` typed edges when locators are available, plus owner-scoped
dependency nodes and `owner -> dependency` import edges for query-deps routing,
so graph-turbo can rank direct code and package follow-ups. Warm graph-turbo
backend cache entries are stored under `$PRJ_CACHE_HOME` when set, otherwise
under the git toplevel `.cache`, with the graph fingerprint guarding against
stale source facts. Inspect or reset that ranking cache with
`asp-graph-turbo cache status`, `asp-graph-turbo cache prune`, and
`asp-graph-turbo cache invalidate`. Use
`--view graph-turbo-request` only when validating or debugging the JSON packet
that will be sent to `asp-graph-turbo`.

`asp-graph-turbo` is the lightweight internal ranker for this path. Its default
ranking dependency is SciPy sparse graph scoring, not PyTorch, PyG, or a GNN
runtime. Future PyG/HeteroData work belongs behind optional lab or offline
rerank surfaces; it must not become an install requirement, hook dependency, or
agent hot-path fallback. The current roadmap is to strengthen typed facts,
profile-specific sparse scoring, failure/read-loop feedback, and command
inflation metrics while keeping the stable search frontier deterministic and
schema-auditable. Its artifact timeline report also surfaces historical
direct-code read-loop risk so cached agent runs can be audited before another
live sandtable pass.

agent semantic client-backend phase 1 is local-native only:

```sh
asp guide
asp doctor
asp providers
asp cache status
asp search --language rust prime --view seeds .
asp query --language rust --treesitter-query '<pattern>' .
asp rust search prime --view seeds .
```

`asp search` and `asp query` are thin routers over the language facades. Use
`--language <rust|typescript|python|julia|org|md>` for ambiguous roots, or pass
an owner/selector path or project root that matches one active provider's
activation coverage so `asp` can route to the same
`asp <language> search|query` boundary without parsing package layout.

Calibrate the local Julia cache hot path after client/cache changes:

```sh
just perf-calibrate-julia-cache
```

Refresh a client hook config after the binaries already exist:

```sh
just install
asp hook install --client codex .
asp hook doctor --client codex .
asp hook install --client claude .
asp hook doctor --client claude .
```

`just install` installs `asp`, `asp-graph-turbo`, `rs-harness`, `ts-harness`,
`py-harness`, and `asp-julia-harness` into
`${SEMANTIC_AGENT_BIN_DIR:-$HOME/.local/bin}` by default, then refreshes the
Codex hook config. Pass a directory argument, such as
`just install /tmp/asp-bin`, to override the install root.

`asp hook install --client <codex|claude>` writes the root client hook
configuration, cache activation, versioned hook policy config, and provider
manifests for this repository. It does not build or install `asp-graph-turbo`,
`rs-harness`, `ts-harness`, `py-harness`, or `asp-julia-harness`; use
`just install` for the full local setup or the `just agent-tools-install-*`
commands for one binary family.

Agent clients invoke the runtime hook entrypoint as `asp hook --client
<codex|claude> --event <event>`. Lifecycle commands stay under the hook
surface: `asp hook install ...` and `asp hook doctor ...`.

Verify a compact AST patch intent without enabling mutation:

```sh
asp ast-patch verify --packet ast-patch.json .
asp ast-patch dry-run --packet ast-patch.json .
```

For text-patch agent adapters such as Codex, `ast-patch` emits a receipt with
`mutationAvailable=false`; apply code changes through the client patch tool.

`doctor` checks the project hook block, PATH binary, activation/provider
manifest sync, and client-specific readiness. Codex also reports user-level
hook trust state; Claude reports non-Codex enforcement probes as not applicable.
It does not prove that the already-running agent thread has reloaded the hook
config. After changing hooks, start a fresh agent session or run a live smoke:

```sh
just agent-hooks-smoke-hook
just agent-hooks-smoke-codex
uv run semantic-sandtable sandtables/root/claude-hook-flow.json
```

The direct smoke replays the hook classifier. The Codex smoke launches the
actual Codex CLI and verifies that a TypeScript source dump is blocked by
`PreToolUse`. The Claude sandtable replays Claude `PreToolUse` payloads through
the same hook classifier.

Run the root hook tests:

```sh
cargo test -p agent-semantic-protocol
cargo test -p agent-semantic-hook
```

Run only the RFC/docs contract tests:

```sh
just check-rfc-docs
```

Run sandtable scenarios:

```sh
uv run semantic-sandtable
uv run semantic-sandtable sandtables/root/codex-hook-dispatcher-flow.json
uv run semantic-sandtable sandtables/root/claude-hook-flow.json
```

Prove a failure-frontier workflow reduces validation/read rounds without losing
hot-block coverage:

```sh
uv run --project packages/python/tools --frozen python -m tools.semantic_sandtable \
  sandtables/fixtures/asp/failure-frontier-real-trigger-replay.json

uv run --project packages/python/tools --frozen python -m tools.semantic_sandtable \
  sandtables/fixtures/asp/failure-frontier-real-trigger-trace-replay.json
```

Both replay gates currently prove `10 -> 4` commands, `0.600` command
reduction, `0.923` stdout-byte reduction, and `4/4` expected hot blocks
covered.

Run the Python policy gate owned by the Python harness:

```sh
just check-python-policy
```

## Notes For Agents

- Prefer compact provider search output over raw source reads.
- Treat `asp` as the public shared CLI and `agent-semantic-hook` as the shared
  classifier implementation; provider hooks
  should publish profile descriptors instead of duplicating platform parsing.
- Use sandtable receipts and real-trigger scenarios to prove that a workflow
  saves commands, bytes, latency, or repeated searches.
- Keep README short. Put design detail in `rfcs/`, `schemas/README.md`, or
  `docs/`.
