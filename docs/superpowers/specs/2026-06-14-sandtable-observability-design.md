# Sandtable Agent Observability Design

Date: 2026-06-14
Status: design-approved

## Purpose

Sandtable runs need to explain what an agent actually did from the initial
intent through the final answer. The target workflow is a live Claude SDK run
against a large Rust library such as Tokio: capture the visible conversation,
tool calls, command results, hook guidance, semantic search packets, and final
answer; then analyze the captured process for quality problems that can improve
`graph_turbo`, search packet guidance, and sandtable expectations.

This design adds an observability layer without turning sandtable scenarios
into raw log archives. The complete visible process is saved as local trace
artifacts. Portable sandtable receipts keep only the structured projection
needed for replay, comparison, and quality scoring.

The system never records hidden model reasoning. It records visible assistant
messages, tool calls, tool results, command I/O, hook decisions, provider dev
logs, explicit assistant decision summaries, and final answers.

## Goals

1. Capture complete visible agent sessions for live sandtable runs.
2. Preserve command stdout/stderr as local artifacts while keeping scenario JSON
   compact and source-safe.
3. Build a session-level receipt that joins Claude SDK messages, command
   traces, hook events, provider dev-command logs, and final-answer evidence.
4. Add an analyzer that reports search-flow quality, evidence use, command
   efficiency, hook-follow behavior, and graph-turbo improvement candidates.
5. Feed analyzer findings into graph-turbo feedback and calibration surfaces
   only through explicit receipts and reports, not hidden ranking heuristics.

## Non-Goals

- Do not capture hidden chain-of-thought or infer it from private runtime state.
- Do not embed full source excerpts, terminal transcripts, or Claude message
  streams in sandtable scenario JSON.
- Do not make the sandtable runner simulate LLM reasoning.
- Do not automatically mutate graph-turbo ranking weights from a single trace.
- Do not make Rust/Tokio observations a Rust-only protocol shortcut. The event
  and receipt schemas remain language-neutral.

## Existing Surfaces

The implementation should extend the current sandtable tooling rather than
replace it:

- `packages/python/tools/src/tools/semantic_sandtable/claude_sdk_runner.py`
  already runs Claude SDK prompts and emits JSON or stream-json messages.
- `agent_observations.py` and `agent_observation_pipe.py` already summarize
  token cost, ASP command flow, direct-read loops, hook denial observations,
  pipe-flow precision, and final-answer presence.
- `trace_record.py` already records command events and writes command output
  files under a trace root.
- `trace_receipts.py` already converts command traces into
  `semantic-sandtable-receipt` packets.
- `trace_comparison_cli.py` and failure-frontier evaluators already compare
  baseline and candidate traces.
- `semantic-dev-command-log.v1` already captures provider-level command events
  when provider dev logging is enabled.

The missing piece is a first-class agent session artifact that joins these
surfaces into one ordered, analyzable process.

## Architecture

The observability workflow has two durable layers.

Layer 1 is the raw session artifact. It is local, complete, and replayable for
analysis. It may contain visible assistant text, command output files, hook
payloads, and message streams. It is not checked into sandtable scenarios by
default.

Layer 2 is the structured projection. It is compact, schema-valid, and safe to
use in tests. It contains command counts, output sizes, token costs, answer
metadata, evidence references, quality findings, graph-turbo feedback
candidates, and links back to raw artifact paths.

```text
Claude SDK run
  -> raw session events and outputs
  -> agent session receipt
  -> sandtable receipt and scenario evidence
  -> analyzer quality report
  -> graph-turbo feedback candidates
```

## Artifact Layout

A session trace root should use this layout:

```text
.cache/agent-semantic-protocol/sandtable-sessions/<session-id>/
  manifest.json
  events.jsonl
  messages.jsonl
  commands/
    <event-id>.json
  outputs/
    <event-id>.stdout
    <event-id>.stderr
  provider-logs/
    <language>/<provider>/commands/<event-id>.jsonl
  receipts/
    agent-session-receipt.json
    sandtable-receipt.json
  reports/
    quality-report.json
    graph-turbo-feedback.json
    text-summary.txt
```

`events.jsonl` is the canonical ordered event stream. `messages.jsonl` stores
raw Claude SDK message payloads. `commands/` stores normalized command metadata.
`outputs/` stores exact stdout/stderr bytes when available. Receipts and reports
are derived artifacts and can be regenerated.

The trace root is a local artifact path. Scenario JSON stores only a receipt
path, session id, and compact metrics.

## Event Model

Add `schemas/semantic-agent-session-event.v1.schema.json` for one JSONL event.
The first version should support these event kinds:

- `session.start`
- `user.intent`
- `assistant.visible-message`
- `tool.request`
- `tool.result`
- `command.start`
- `command.result`
- `hook.decision`
- `provider.dev-command`
- `search.packet-summary`
- `answer.final`
- `session.stop`

Common fields:

- `schemaId`
- `schemaVersion`
- `eventId`
- `sessionId`
- `ordinal`
- `timestampUtc`
- `kind`
- `source`
- `parentEventId`
- `toolUseId`
- `commandId`
- `artifactRefs`
- `fields`

Events should reference large content through artifact refs rather than inline
payloads. A visible assistant message can include a short preview and a
`messages.jsonl` pointer. A command result can include byte counts, line counts,
exit code, elapsed milliseconds, output fingerprints, and output file refs.

## Session Receipt

Add `schemas/semantic-agent-session-receipt.v1.schema.json`. It is the compact
join across session events and existing command traces.

Required top-level fields:

- `schemaId = agent.semantic-protocols.semantic-agent-session-receipt`
- `schemaVersion = 1`
- `sessionId`
- `scenarioId`
- `language`
- `project`
- `intent`
- `agent`
- `model`
- `startedAtUtc`
- `finishedAtUtc`
- `editBoundary`
- `artifactRoot`
- `summary`
- `answer`
- `commands`
- `qualityFindings`

The `summary` object should include:

- total turns
- assistant visible-message count
- tool request/result count
- command count
- ASP command count
- search/query/check/guide counts
- denied command count
- repeated command count
- direct-read risk count
- stdout/stderr bytes
- elapsed milliseconds
- token cost when available

The `answer` object should include:

- `present`
- `afterLastToolUse`
- `textBytes`
- `textLineCount`
- `messageEventId`
- `evidenceRefs`
- `groundingStatus`
- `preview`

`groundingStatus` is one of `grounded`, `weak`, `ungrounded`, or `unknown`. The
analyzer computes it from visible citations to commands, selectors, packet
summaries, failure-frontier entries, or graph-turbo evidence.

## Sandtable Projection

`semantic-sandtable-receipt.v1` remains the command-flow replay receipt. It
should gain only additive links to the session layer:

- `agentSessionReceiptPath`
- `agentSessionId`
- `answer`
- `qualityFindings`

The replay model still executes explicit commands and validates expected output
facts. It does not replay Claude, hidden reasoning, or full message streams.

`semantic-sandtable-scenario.v1` should allow evidence metadata to point at an
agent session receipt:

```json
{
  "evidence": {
    "source": "live-agent",
    "receiptPath": ".../sandtable-receipt.json",
    "agentSessionReceiptPath": ".../agent-session-receipt.json"
  }
}
```

## CLI Shape

Extend the existing `semantic-sandtable` CLI with session-level commands:

```sh
uv run --project packages/python/tools --frozen python -m tools.semantic_sandtable \
  --record-agent-session \
  --agent claude-sdk \
  --scenario-id rust.tokio-agent-observability \
  --language rust \
  --provider rs-harness \
  --project-name tokio \
  --intent "Explain how Tokio wires AsyncRead readiness to runtime IO drivers" \
  --trace-root .cache/agent-semantic-protocol/sandtable-sessions \
  --output-format stream-json
```

Build projections from a recorded session:

```sh
uv run --project packages/python/tools --frozen python -m tools.semantic_sandtable \
  --build-agent-session-receipt <session-root> \
  --output <session-root>/receipts/agent-session-receipt.json
```

Run the analyzer:

```sh
uv run --project packages/python/tools --frozen python -m tools.semantic_sandtable \
  --analyze-agent-session <session-root> \
  --quality-report <session-root>/reports/quality-report.json \
  --graph-turbo-feedback <session-root>/reports/graph-turbo-feedback.json
```

The existing `--build-receipt-from-trace`, `--compare-traces`, and
`--list-trace-sessions` commands continue to work for command-only traces.

## Capture Flow

1. The runner creates a session id and manifest.
2. The runner starts Claude SDK with stream-json output, hook events enabled,
   ASP command budgets configured, and provider dev logging enabled.
3. Each SDK message is written to `messages.jsonl` and projected into
   `events.jsonl`.
4. Tool-use blocks become `tool.request` events.
5. Tool-result blocks become `tool.result` events. If they contain command
   output, the output is written to `outputs/` and referenced by fingerprint.
6. ASP commands found in tool-use blocks become command metadata records.
7. Provider dev-command logs are copied or linked into `provider-logs/` and
   joined by session id, command text, hook run id, or time window.
8. The final assistant answer becomes `answer.final`.
9. The receipt builder reads the event stream and writes the session receipt.
10. The analyzer reads the receipt plus selected output previews and writes
    reports.

If a run stops early because a command budget, timeout, missing SDK package, or
missing auth token is hit, the session still emits `session.stop` with status
`partial` and a quality finding explaining why no final answer is available.

## Analyzer

Add an analyzer module under
`packages/python/tools/src/tools/semantic_sandtable/agent_session_analyzer.py`.
It consumes the session receipt and raw event references.

Quality dimensions:

1. Search-flow quality
   - prime before pipe
   - pipe before direct code reads
   - query selector after exact selector
   - no repeated identical search
   - no ignored `nextCommand` when a later command widened the search

2. Evidence quality
   - final answer exists after last tool use
   - final answer references at least one packet, selector, failure-frontier
     entry, or command result
   - selected evidence comes from parser/provider outputs, not memory-only text
   - answer does not overclaim when evidence grade is unknown

3. Command efficiency
   - ASP command count and total command count
   - repeated commands
   - stdout/stderr bytes
   - direct-read risk count
   - adjacent read windows and duplicate selectors
   - avoidable fanout

4. Hook-follow quality
   - denied commands are followed by the recommended safe route
   - hook feedback category is preserved in structured findings
   - no retry of the same denied raw source read or broad search

5. Graph-turbo improvement candidates
   - missing high-value nodes or relations mentioned by the answer
   - low-precision packet output that forced extra commands
   - repeated searches that can become query-set or pipe composition
   - ranked evidence that was ignored because a next action was unclear
   - graph facts that should be boosted, suppressed, or split by profile

Each finding has:

- `id`
- `kind`
- `severity`
- `message`
- `evidenceRefs`
- `recommendedAction`
- `graphTurboFeedback`

Severity levels are `info`, `warning`, and `error`. MVP gates should warn
rather than fail unless a scenario explicitly sets `--fail-on-warn`.

## Graph-Turbo Feedback

The analyzer writes
`reports/graph-turbo-feedback.json` as an explicit candidate packet. It is not
applied automatically.

The packet should include:

- session id and scenario id
- source receipt path
- matched selectors and packet node ids
- missed or under-ranked facts
- repeated query groups
- expected rank or next-action change
- confidence
- reason

Graph-turbo calibration may consume this packet only through an explicit
command, for example:

```sh
uv run --project packages/python/asp_graph_turbo --frozen graph-turbo calibrate \
  --feedback reports/graph-turbo-feedback.json \
  --output reports/graph-turbo-calibration.json
```

This keeps ranking changes reviewable and prevents one noisy live run from
silently changing default behavior.

## Rust/Tokio MVP Scenario

The first scenario should target a real Tokio checkout or cached registry copy.
The prompt should ask a feature-level or architecture-level question that
requires semantic exploration rather than a file-level lookup.

Example intent:

```text
Explain how Tokio connects AsyncRead readiness with runtime IO driver wakeups,
and identify the safest owner areas to inspect before editing behavior.
```

The expected successful flow is:

```text
guide -> search prime -> search pipe -> owner/lexical or reasoning follow-up
-> exact query/read -> answer.final
```

The analyzer should report whether the answer was grounded in provider facts
and whether graph-turbo or search guidance could have reduced the command
count.

## Error Handling

Malformed JSONL events are skipped with a warning finding and copied into an
`invalid-events` count. Missing output files become artifact warnings, not hard
failures. Missing final answers are errors for live-agent scenarios and warnings
for partial sessions. Missing provider dev logs are warnings unless the scenario
requires provider-level observability.

Sensitive values should be redacted before events are written. The first pass
uses exact-key redaction for environment variables and known token fields. The
redacted value should preserve byte-count and fingerprint metadata only when the
fingerprint cannot reveal the secret.

## Testing

Schema tests:

- `semantic-agent-session-event.v1.schema.json`
- `semantic-agent-session-receipt.v1.schema.json`
- additive sandtable scenario and receipt fields
- graph-turbo feedback report packet

Unit tests:

- Claude SDK message stream to session events
- tool request/result correlation by `tool_use_id`
- command output artifact writing and fingerprinting
- final-answer extraction after last tool use
- provider dev-command join by session id and fallback time window
- analyzer findings for repeated search, ignored next command, direct-read
  risk, denied-command retry, and weak grounding
- graph-turbo feedback candidate generation

CLI tests:

- `--record-agent-session` writes manifest, events, messages, outputs, and a
  partial receipt on budget stop
- `--build-agent-session-receipt` is deterministic for fixture events
- `--analyze-agent-session` writes quality and feedback reports
- command-only trace flows still pass unchanged

Regression tests:

- no source output is embedded in sandtable scenario JSON
- no hidden reasoning field is accepted by the schema
- live-agent scenarios can require final-answer grounding
- warnings can be promoted to failures through scenario policy

## Implementation Order

1. Update the RFC surface under
   `docs/10-19-rfcs/10.05-cli-first-harness-ux/` with the session
   observability boundary and include it from `10.05-cli-first-harness-ux.org`.
2. Add the session event, session receipt, and feedback packet schemas.
3. Extend the sandtable schema and receipt schema with additive session links.
4. Add session artifact writing around the existing Claude SDK runner.
5. Add receipt building from session events.
6. Add the analyzer and graph-turbo feedback report.
7. Add fixture-based tests, then one Rust/Tokio live-agent scenario gated by
   environment availability.
8. Run focused sandtable and schema tests before broad repo gates.

## Success Criteria

The MVP is complete when one live or fixture-backed Claude SDK run can produce:

- a raw session artifact with visible messages, tool calls, command outputs, and
  final-answer event;
- a schema-valid agent session receipt;
- a schema-valid sandtable receipt linked to the session receipt;
- a quality report that identifies at least command efficiency, search-flow,
  hook-follow, and answer-grounding findings;
- a graph-turbo feedback candidate packet;
- focused tests proving that scenario JSON stays compact and source-safe.
