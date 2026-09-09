<!--
SPDX-FileCopyrightText: 2026 tao3k team and Contributors
SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later
-->

@/Users/guangtao/.agent-semantic-protocols/resources/org/templates/ASP_ORG_SKILL.org

# Project Workflow

## Project Environment Entry Point

Run project-scoped commands only through `.devenv/devenv-profile-exec`.
Do not use `direnv exec .` in this repository. Keep RTK inside the explicit
devenv entry point when a filtered RTK wrapper exists; use the raw command
through the same entry point when exact protocol or failure output is evidence.

## ASP Org Agent Flow

The included `ASP_ORG.org` owns durable Org planning, specifications,
adversarial review, and agent-state workflow. Keep state path and layout rules
in that skill instead of duplicating them here.

## Plugin Hook Contract

Treat the installed Codex plugin and its `hooks.json` as a fixed host contract.
Rebuilding or reinstalling the ASP binary, providers, Runtime Server, or
workspace-owner state does not authorize reinstalling the plugin, rewriting
project trust, or requiring a Codex restart. Only refresh the plugin when
`hooks.json` or another plugin payload actually changed and the task explicitly
requires publishing that change; verify that payload drift first. Recover
Runtime Server and workspace-owner failures independently from plugin
trust/configuration.

## Search Protocol Changes

When work touches semantic search behavior, query composition, search output,
search packets, or agent-facing search guidance, follow this order:

1. Update the search RFC first. Use Org format and keep the protocol intent in
   `docs/10-19-rfcs/`, especially
   `docs/10-19-rfcs/10.05-interactive-graph-first-progressive-searchloop.org`
   when the interactive SearchLoop, evidence graph, progressive search, public
   client, or agent workflow changes. Git is the only archive; the working tree
   contains only the current server-first architecture. Use
   `docs/10-19-rfcs/10.06-agent-search-projection.org` when work changes search projection
   rendering, graph-derived rank, LLM-oriented code reasoning projection, or
   graph facts that should be available across the agent-facing `search`
   interface.
2. Define the shared contract in `schemas/` before provider implementation.
   Keep language-neutral search semantics in shared schemas, and put
   language-specific facts under provider-owned fields or provider schemas.
   If a schema change is breaking, create a new versioned schema instead of
   changing the existing contract in place.
3. Align other language providers to the RFC and schema contract. Update
   package-local common schema copies and provider registry descriptors where
   needed so Rust, TypeScript, Python, Julia, and future providers can converge
   on the same search packet shape.
4. Implement the Rust provider after the RFC and schema are clear. Add or update
   ASP Rust tests for CLI parsing, compact output, JSON packet validation,
   registry descriptors, and any new query-composition behavior.
5. Return to this repository's sandbox or sandtable tests and align them with
   the updated protocol. Prefer scenario coverage that validates the real
   provider binary and the shared JSON schema rather than only checking docs.
6. Exercise the workflow against a representative real project. Capture command
   count, packet size, latency, repeated-trigger patterns, missing facts, and
   confusing next actions. Feed any protocol gap back through the RFC and schema
   path before widening implementation.
7. Only after real-project evidence is reviewed, optimize provider behavior or
   agent guidance. Keep optimizations parser-owned and contract-visible; avoid
   hidden string heuristics or provider-private shortcuts that other languages
   cannot reproduce.

When `search playbook` exposes several independent semantic axes, prefer a
fan-out/fan-in exploration step before editing. The playbook is the sole
public search operation; its fixed internal order is rg acquisition,
provider-native syntax, Tantivy lexical retrieval, then Python Graphs:

- fan out only independent axes, such as dependency API usage, source owners,
  test reachability, and policy findings
- assign each worker one bounded command group and require compact
  `[search-subagent]` evidence instead of source dumps
- use the client runtime's multi-agent or tool-parallel execution when
  available, but keep the protocol valid when those features are disabled by
  running the same command groups sequentially
- fan in through a parent `[search-synthesis]` decision that chooses the next
  focused search or the edit boundary

Do not encode client feature flags such as `multi_agent` or
`tool_parallelism` as required provider behavior. They are execution
accelerators; the shared search contract is the prime packet, query-set packet,
subagent receipt, and synthesis evidence.

Do not jump straight from a real-project pain point to a Rust-only fix when the
issue changes the shared search contract. The durable path is:

```text
RFC -> shared schema -> language alignment -> Rust implementation/tests ->
sandtable alignment -> real-project evidence -> optimization loop
```

## Python Policy Checks

Use the ASP Python dependency API as the owner for Python policy checks in
this repository. Do not copy Python policy logic into the sandtable runner and
do not add a second provider policy command surface. Invoke the public API
from build/test ownership when you need the actual policy gate:

```sh
uv run --project languages/asp-python --frozen python -c 'from asp_python import assert_asp_python_clean; assert_asp_python_clean(".")'
```

If `just` is available in the active shell, `just check-python-policy` is the
same API gate and `just report-python-policy` prints the API report without
making the shell step fail. These recipes import
`languages/asp-python` through its own project environment so
the current repository consumes one policy authority without a provider CLI.
