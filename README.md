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
  hook decisions, hook profiles, sandtable scenarios, and receipts.
- `crates/semantic-agent-hook/`: Rust root hook runtime for Codex/agent hook
  classification and provider routing.
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

Install agent-facing tools and Codex hook config:

```sh
just agent-hooks-install
just agent-hooks-doctor
```

Install the same tools into a user-owned bin directory. The directory must be
on the PATH that Codex uses to run hooks:

```sh
mkdir -p "$HOME/.local/bin"
export PATH="$HOME/.local/bin:$PATH"
just agent-hooks-install "$HOME/.local/bin"
just agent-hooks-doctor "$HOME/.local/bin"
```

Install individual agent tools when only one boundary changed:

```sh
just agent-tools-install-hook "$HOME/.local/bin"
just agent-tools-install-rust "$HOME/.local/bin"
just agent-tools-install-typescript "$HOME/.local/bin"
just agent-tools-install-python "$HOME/.local/bin"
```

Refresh only the Codex hook config after the binaries already exist:

```sh
semantic-agent-hook install --client codex .
semantic-agent-hook doctor --client codex .
```

`semantic-agent-hook install` writes the root Codex hook block and merged
profile registry for this repository. It does not build or install
`semantic-agent-hook`, `rs-harness`, `ts-harness`, or `py-harness`; use the
`just agent-tools-install-*` commands for those binaries.

`doctor` checks the project hook block, PATH binary, profile registry, and
Codex user-level hook trust state. It does not prove that the already-running
agent thread has reloaded the hook config. After changing hooks, start a fresh
Codex session or run a live smoke:

```sh
just agent-hooks-smoke-hook
just agent-hooks-smoke-codex
```

The direct smoke replays the hook classifier. The Codex smoke launches the
actual Codex CLI and verifies that a TypeScript source dump is blocked by
`PreToolUse`.

Run the root hook tests:

```sh
cargo test -p semantic-agent-hook
```

Run sandtable scenarios:

```sh
uv run semantic-sandtable
uv run semantic-sandtable sandtables/root/codex-hook-dispatcher-flow.json
```

Run the Python policy gate owned by the Python harness:

```sh
just check-python-policy
```

## Notes For Agents

- Prefer compact provider search output over raw source reads.
- Treat `semantic-agent-hook` as the shared classifier; provider hooks should
  publish profile descriptors instead of duplicating platform parsing.
- Use sandtable receipts and real-trigger scenarios to prove that a workflow
  saves commands, bytes, latency, or repeated searches.
- Keep README short. Put design detail in `rfcs/`, `schemas/README.md`, or
  `docs/`.
