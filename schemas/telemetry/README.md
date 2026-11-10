<!--
SPDX-FileCopyrightText: 2026 tao3k team and Contributors
SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later
-->

# ASP telemetry semantic-convention registry

This directory is the normative telemetry vocabulary for ASP. The YAML model is
the source of truth. Rust constants, JSON Schema projections, Turso columns, and
external OpenTelemetry mappings are generated artifacts and must not introduce
attributes independently.

The registry is intentionally vendor-neutral. OpenTelemetry provides trace,
metric, log, and propagation semantics; ASP owns search, evidence, graph,
router, selector, generation, and resident-runtime vocabulary.

The initial registry is `model/asp-performance.yaml`. Its generated JSON Schema
projection is `../runtime-server-opentelemetry-performance-event.v1.schema.json`.
External GenAI conventions are mappings, not the authority for ASP identities.

The resident ingress wire contract is independently versioned as
`../runtime-server-performance-observation.v1.schema.json`. It carries a
pre-span observation from a command boundary to the Runtime Server; the server
adds trace identity, timestamps, status, and normalized attributes before the
event becomes the OpenTelemetry performance projection above.

The long-lived receiver, backpressure queue, batch processor, Turso exporter,
and shutdown sequence are Tokio-owned. A synchronous command can report a
failure before any Tokio reactor exists, so that boundary uses one nonblocking
Unix datagram send only; it must never create a runtime, await I/O, open Turso,
or participate in server lifecycle.

Workspace identity is never inferred from a path or recomputed with Git on a
failure boundary. The command adapter carries the already-known project root
and resolves only an admitted identity through the Runtime Server admission
catalog's mmap/cache lookup. A missing admission leaves the optional identity
unset instead of publishing a guessed value.

Diagnostics query the resident reader lane through the versioned
`runtime-server-performance-query` request/receipt contracts. They never open
the Turso file from a second process. A query forces the OpenTelemetry batch
processor to flush before reading, so callers do not use sleeps, lock retries,
or eventual-consistency guesses.
