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
just agent-tools-install-hook "$HOME/.local/bin"
just agent-tools-install-rust "$HOME/.local/bin"
just agent-tools-install-typescript "$HOME/.local/bin"
just agent-tools-install-python "$HOME/.local/bin"
```

agent semantic client-backend phase 1 is local-native only:

```sh
asp guide
asp doctor
asp providers
asp cache status
asp rust search prime --view seeds .
```

Refresh a client hook config after the binaries already exist:

```sh
asp hook install --client codex .
asp hook doctor --client codex .
asp hook install --client claude .
asp hook doctor --client claude .
```

`asp hook install --client <codex|claude>` writes the root client hook
configuration, cache activation, versioned hook policy config, and provider
manifests for this repository. It does not build or install `rs-harness`,
`ts-harness`, or `py-harness`; use the `just agent-tools-install-*` commands
for those binaries.

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

Run sandtable scenarios:

```sh
uv run semantic-sandtable
uv run semantic-sandtable sandtables/root/codex-hook-dispatcher-flow.json
uv run semantic-sandtable sandtables/root/claude-hook-flow.json
```

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
