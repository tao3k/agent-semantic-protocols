# Real-Trigger Sandtable Loop Design

Date: 2026-05-31
Status: design-approved

## Purpose

The sandtable should capture one realistic agent exploration loop before it
becomes an edit. The first target is a Rust/Tokio scenario: start Codex with a
large real Rust project and a feature-change intent, let the agent explore with
`rs-harness search`, then stop before implementation. The artifact we need is
the search flow, not the code edit.

This closes the gap between hand-written replay scenarios and actual agent
behavior. It lets the protocol measure whether `search prime`, follow-up search
views, subagent fan-out, and hook deny guides reduce token cost while improving
search precision.

## Scope

In scope:

- Define the RFC path for a real-trigger sandtable loop.
- Define a receipt shape for recorded Codex exploration sessions.
- Convert a recorded receipt into replayable sandtable steps.
- Track command count, output size, elapsed time, repeated-trigger patterns,
  missing facts, confusing next actions, and query-set merge opportunities.
- Assert hook deny guide quality, not just deny behavior.
- Produce a compact sandtable summary explaining what changed for token use,
  search round count, and precision.

Out of scope:

- Running Codex from inside the sandtable runner.
- Simulating LLM reasoning in the runner.
- Making Rust-only search optimizations before the RFC and shared schema are
  clear.
- Recording source contents or full terminal transcripts into scenario JSON.

## Protocol Ownership

The durable path remains:

```text
RFC -> shared schema -> language alignment -> Rust implementation/tests ->
sandtable alignment -> real-project evidence -> optimization loop
```

`docs/10-19-rfcs/10.05-cli-first-harness-ux.org` owns the public search workflow language. It
should gain a section for the real-trigger loop: selected project, task intent,
prime-first requirement, subagent search constraints, edit-stop boundary, and
evidence that feeds back into schema and provider behavior.

`schemas/semantic-sandtable-scenario.v1.schema.json` owns portable replay
metadata. It should gain additive fields for scenario evidence and guide-quality
assertions while keeping normal step execution deterministic.

`schemas/semantic-search-packet.v1.schema.json` owns any shared packet fields
needed to explain query-set composition and merge opportunities. Language facts
still belong under provider-owned `fields`.

## Recording Model

A real-trigger receipt is a compact evidence file produced outside the runner.
It records only the information needed to judge the search flow:

- project identity and workdir, for example Tokio from the Cargo registry or a
  configured checkout;
- the user intent that launched exploration;
- every accepted `rs-harness search ...` command before editing starts;
- every denied raw read or broad raw search with its hook guide;
- subagent search requests and their compact receipts;
- step metrics: elapsed milliseconds, stdout bytes, stderr bytes, and line
  counts;
- qualitative findings: missing prime facts, repeated searches, unclear next
  actions, and candidate query-set merges.

Receipts should not embed source excerpts. If a raw search was needed, the
receipt stores the normalized command shape and the resulting ingest/search
packet summary.

## Replay Model

The sandtable scenario is generated or hand-transcribed from the receipt. It
does not replay Codex. It replays the harness-facing command flow:

```text
prime -> focused search -> owner/items -> tests -> ingest -> hook deny guide
```

The first Rust/Tokio replay should cover:

- `rs-harness search prime --view seeds .`
- one or more focused text/owner/dependency searches selected from prime;
- an owner/items follow-up for a large or public owner;
- a tests follow-up for the selected owner;
- an external candidate stream normalized through `search ingest`;
- one raw broad search denial that must include a useful ingest guide;
- one direct source-read denial that must include a useful owner/items guide;
- optional subagent receipt checks when the real trigger used subagents.

The runner should continue to execute explicit argv arrays and `stdinCommand`
pipes. It should not learn shell-specific tracing semantics beyond existing
hook replay and command execution.

## Guide-Quality Assertions

Hook deny checks should prove that the guide is actionable. A denial is useful
only when it includes:

- the reason category, such as direct source read or raw broad search;
- the matching language and provider route;
- the next safe command as argv-shaped guidance;
- the pipe shape when the safe path is `rg/fd -> search ingest`;
- subagent receipt requirements when the denial concerns subagent output;
- no leaked source text.

These assertions can be expressed as schema-backed hook decision checks plus
compact string checks in sandtable scenarios. If the current platform response
cannot expose enough structured fields, the provider should still emit the
shared `agentHookDecision` packet before platform-specific rendering.

## Metrics And Summary

The runner already records command count, elapsed time, stdout/stderr lines, and
bytes. The real-trigger loop should surface these as a concise summary:

```text
[sandtable-flow] scenario=rust.tokio-real-trigger commands=7 stdoutBytes=...
|prime quality=pass owners=... missing=...
|merge view=owner queries=3 saveCommands=2
|guide raw-search=pass direct-read=pass
|token delta=lower packetBytes=... repeatedSearches=...
```

The summary should answer:

- What did `prime` make cheaper or more precise?
- Which repeated search commands can become a query-set or pipe composition?
- Which hook guides caused the next correct command?
- Which missing facts forced external search or extra rounds?
- What changed for token count, search round count, and precision?

Warnings are enough at first. Once a real-trigger pattern becomes stable, the
same metrics can become `--fail-on-warn` regression gates.

## First Scenario

The first scenario is Rust/Tokio because it has enough structure to exercise the
workflow:

- large real Rust package;
- public API owners;
- async IO search axes;
- feature/cfg facts such as `io-uring` and `tokio_unstable`;
- tests and examples;
- natural opportunity for subagent fan-out;
- natural hook denial cases for raw source reads and broad `rg`.

The initial user intent should describe a feature-level task, not a file-level
task, so the agent must depend on `search prime` quality before choosing search
axes.

## Validation

Implementation should land in this order:

1. Update `docs/10-19-rfcs/10.05-cli-first-harness-ux.org` with the real-trigger loop.
2. Add additive schema fields for evidence and guide-quality assertions.
3. Add or update sandtable runner tests for new schema and summary behavior.
4. Add the first Rust/Tokio real-trigger scenario.
5. Run focused tests for the runner and schema validation.
6. Run the scenario against a representative Tokio checkout when available.

The first closure target is not perfect optimization. It is a recorded,
replayable, budgeted loop that makes search quality and hook-guide quality
visible enough to improve deliberately.
