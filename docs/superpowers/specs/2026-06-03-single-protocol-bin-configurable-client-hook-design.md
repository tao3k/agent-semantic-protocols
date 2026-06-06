# Single Protocol Bin And Configurable Client Hook Design

## Context

The workspace currently has two Rust crates:

- `agent-semantic-hook`: owns hook runtime library behavior: activation loading, event parsing, classification, provider routes, platform response rendering, config loading, and cache helpers.
- `agent-semantic-protocol`: owns the public `asp` CLI, `hook install`/`hook doctor` lifecycle commands, and the `hook` command runtime for client event dispatch.

The desired user-facing shape is one installed binary: `asp`.
`agent-semantic-hook` should remain a library crate for hook runtime implementation, with no standalone binary or crate-local CLI facade in the normal install surface.

The client hook runtime must not grow a watcher or server loop. Hook invocations are client-side process executions. Server-mode watching and long-lived runtime behavior belong to a separate RFC and implementation lane.

## Goals

1. Make `asp` the only user-facing binary for this feature family.
2. Keep existing hook behavior available through `asp hook ...`.
3. Remove references that instruct users, installers, tests, or generated Codex config to invoke `agent-semantic-hook` directly.
4. Introduce a configurable client-hook rule layer that is typed, schema-visible, and deterministic on each hook invocation.
5. Keep rule behavior compatible with current provider route receipts and hook decision JSON.

## Non-Goals

- No client-side watch mode.
- No daemon, persistent server, or background runtime in the hook crate.
- No general scripting language in the first configurable-rule pass.
- No hidden provider-private string heuristics that bypass the shared hook decision contract.
- No compatibility alias that keeps `agent-semantic-hook` as a supported public command unless a later migration requirement explicitly adds one.

## Single-Bin Design

`asp` becomes the command users install and invoke:

```sh
asp hook install --client codex .
asp hook doctor --client codex .
asp hook pre-tool --client codex
asp ast-patch dry-run --packet semantic-ast-patch.json .
```

`agent-semantic-hook` remains a library dependency of the `agent-semantic-protocol` crate.
Its `src/main.rs` target should be removed from the package target surface or otherwise excluded from install-facing builds. The library API remains the boundary used by the `asp` CLI so the existing classifier, activation, Codex config, provider manifest, and platform-rendering code do not need a large move.

Generated Codex config should call:

```sh
asp hook <event> --client codex
```

No `agent-semantic-hook` command should be generated or supported.

Installer, doctor, skill text, tests, schemas, and sandtable fixtures should converge on the protocol binary name where they refer to user-facing hook execution.

## Configurable Client Hook Design

Add a typed hook runtime config that is loaded on each hook invocation. Because each hook event starts a fresh client-side process, dynamic adjustment is achieved by reading the current config at invocation time. No watcher is required.

Recommended first config path:

```text
.codex/agent-semantic-protocol/hooks/config.toml
```

The activation file should remain the registry of provider capabilities and project activation state. The new config should be optional and layered over defaults. Missing config means current behavior.
Codex install should create `.codex/agent-semantic-protocol/hooks/config.toml` when it is
missing. The generated file should contain schema metadata and commented rule
examples only, so the install does not change policy until the user explicitly
uncomments or adds rules. Existing valid config must be preserved.

`.codex/agent-semantic-protocol/hooks` is durable project hook policy. It
should contain `config.toml`, not generated activation or cache JSON. Generated
activation, provider profile registries, and hook event logs are cache
artifacts and should be written to
`${PRJ_CACHE_HOME}/agent-semantic-protocol/hooks` when `PRJ_CACHE_HOME` is set,
otherwise to the git toplevel `.cache/agent-semantic-protocol/hooks` directory.

First-pass rule model:

```toml
[[rules]]
id = "deny-broad-rust-source-search"
enabled = true
event = "pre-tool"
priority = 100
decision = "deny"
reasonKind = "raw-broad-search"
languageIds = ["rust"]

[rules.match]
tool = "Bash"
commandAny = ["rg", "grep", "fd", "find"]
pathGlobAny = ["**/*.rs"]

[[rules.routes]]
providerId = "rs-harness"
languageId = "rust"
binary = "asp"
kind = "ingest"
argv = ["asp", "rust", "search", "ingest", "items", "tests", "--view", "seeds", "."]
stdinMode = "pipe-candidates"
```

The implementation should compile config rules into typed matchers before classification:

- event matcher
- platform matcher
- tool/action matcher
- command token matcher
- path/glob matcher using existing `globset`
- optional provider/language matcher resolved through activated provider coverage

The classifier should continue to produce the existing hook decision schema. Config-derived decisions should include `fields.configRuleId` so tests and receipts can prove which rule fired without parsing prose.
Rule ids must be unique across the loaded config; duplicate ids are invalid
because they make `fields.configRuleId` receipts ambiguous.
Rules are evaluated by descending `priority`; rules with equal priority keep
their config file order so the tie-break is deterministic and reviewable.
The Rust loader must reject the same schema-shape mistakes that the shared JSON
schema rejects, including invalid identifiers, empty min-length strings,
unsupported events/platforms, duplicate `languageIds`, empty route `argv`, and
invalid route binary names.

## Dependency Recommendation

Use `figment` for config loading. It supports typed `serde` extraction and
source-aware errors while still keeping this client-hook lane invocation-based.
Do not enable live re-read/watch affordances; dynamic adjustment comes from
loading `.codex/agent-semantic-protocol/hooks/config.toml` in each hook process.
`config` is acceptable for a later layered-settings lane, but its watch
features are not needed here.

Do not introduce Rhai/Rego/Cedar in the first pass. They are useful options for later policy backends, but the first pass should keep the rule language small, typed, testable, and schema-visible. CEL or Cedar can be revisited if typed matchers become too limited.

## Data Flow

1. `asp hook ...` owns command parsing and runtime dispatch.
2. The protocol runtime loads activation state through the hook library API.
3. The hook library loads optional client hook config.
4. The hook library parses the incoming platform hook payload.
5. Built-in classifiers and config rules run through one decision pipeline.
6. The first highest-priority deny/guidance decision emits the existing platform response and JSON receipt shape.
7. Allow decisions remain the default when no built-in or configured rule matches.

The built-in policy should remain available as defaults. Config can disable or override selected defaults only through explicit typed settings, not through arbitrary script execution.

`asp hook doctor --client codex .` should validate this
same optional config path. Missing config reports `clientConfigStatus=missing`,
valid config reports `clientConfigStatus=ok`, and invalid config makes doctor
fail with a client-config error instead of silently accepting degraded policy.

## Testing

Single-bin tests:

- `asp hook install --client codex .` writes Codex config that invokes `asp`.
- `asp hook install --client codex .` installs or verifies a PATH-visible `asp` binary before writing bare hook commands.
- every generated Codex hook event invokes `asp hook <event> --client codex`, never the retired hook binary.
- `asp hook doctor --client codex .` reports protocol binary usage.
- No test fixture or generated instruction requires `agent-semantic-hook` as a public binary.
- `cargo metadata` for the workspace no longer exposes an install-facing `agent-semantic-hook` bin target, and the protocol crate owns the only hook-facing bin target.

Config tests:

- missing config preserves current behavior.
- invalid config fails closed with a structured runtime failure decision.
- doctor reports missing/ok client config status and rejects invalid config.
- config file changes are reflected on the next hook invocation without a watcher.
- duplicate rule ids fail closed before classification.
- equal-priority rules keep config order.
- schema-shape mistakes fail closed at runtime instead of becoming inert rules.
- disabled rule does not fire.
- priority chooses the highest-priority matching rule.
- glob/path and command-token rules reproduce at least one current built-in denial.
- config-derived decisions validate against the existing hook decision schema.

Validation commands should stay crate-scoped first:

```sh
direnv exec . rtk --ultra-compact cargo test -p agent-semantic-protocol
direnv exec . rtk --ultra-compact cargo test -p agent-semantic-hook
```

Run broader workspace checks only after the focused tests pass.

## Migration Notes

This is a breaking command-surface cleanup. Do not preserve a public `agent-semantic-hook` compatibility alias or warning shim. Generated Codex config must invoke `asp hook ...`.

Existing installed hooks must be refreshed with:

```sh
asp hook install --client codex .
```

after the protocol binary is installed into the PATH used by the client.
