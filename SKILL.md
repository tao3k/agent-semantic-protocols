---
name: agent-semantic-protocols
description: Use when working with the language provider binaries maintained by agent-semantic-protocols, including asp hook installs, compact semantic search flow, and non-JSON agent command guidance.
---

# Agent Semantic Protocols

<!-- ASP_INSTALLED_SKILL_NOTICE -->

## Provider Activation

This root `SKILL.md` is the embedded template source used by `asp hook install`.
The installed copy under `.agents/skills/agent-semantic-protocols/SKILL.md` is
rendered from this template plus the current hook activation. Do not hard-code
the active language list here: activation is owned by provider binary detection
and `asp.toml`.

<!-- ASP_PROVIDER_SUMMARY -->

Start with `asp <language> guide .` when a task needs the provider-owned tool
map. Use `asp providers` or `asp doctor` when the active language or provider
binary is unclear.

## Rules

- Use the `asp <language>` facade for agent exploration; provider binaries are
  implementation/debug surfaces.
- Do not add `--json` during agent exploration. `--json` is for schema tests,
  validators, receipts, and IDE integrations.
- When a search/query term contains shell metacharacters copied from docs or
  code, such as backticks, pipes, `$`, globs, braces, or spaces, pass it as a
  single-quoted argv literal. For example:
  ``--query-set 'Start with `asp <language> guide .`'``. If the text
  itself contains single quotes, narrow it into separate safe terms or use a
  provider-documented file/stdin surface; do not interpolate raw prose into a
  shell command.
- `asp hook install --client codex .` installs hooks, provider activation, and
  the rendered skill for the detected providers.
- Search is discovery and should not inline source code.
- Query with `--code` is for exact or unique code extraction.
- Tree-sitter query is the syntax base; native parser facts enrich the
  capture/frontier.
- Hook config may disable provider `ast-patch`; when disabled, patch with
  `apply_patch` after exact locator/code evidence.

## Complex Flows

### Hook Recovery

When a hook blocks a raw source read or broad raw search, follow the recovery
route printed in the hook message. Do not retry `Read`, `cat`, `sed`, `rg`, or
source-dump commands on the same matched source.

```sh
asp <language> query --from-hook direct-source-read --selector <path-or-range> --code .
asp <language> query --from-hook direct-source-read --selector <glob-or-path> --term <term> --surface owners,tests --view seeds .
```

### Search Before Code

```sh
asp <language> search prime --view seeds .
asp <language> search fzf <term> owner tests --view seeds .
asp <language> search owner <owner-path> items --query '<symbol-or-a|b|c>' .
asp <language> query <owner-path> --term <candidate> --names-only .
asp <language> query <owner-path> --term <candidate> --code .
```

Use `--names-only` for broad owner-local prefixes before requesting code.

### Tree-sitter Locate Then Code

```sh
asp <language> query guide treesitter .
asp <language> query --treesitter-query '<pattern>' .
asp <language> query --selector <path-or-range> --treesitter-query '<narrow-pattern>' --code .
```

Without `--code`, tree-sitter query output should be a capture/frontier locator.
With `--code`, stdout should be pure source for an exact selector or unique
match.

### Verification

```sh
asp <language> check --changed .
asp <language> ast-patch dry-run --packet <semantic-ast-patch.json> .
```

Use provider `ast-patch` only for structural/mechanical edits after a dry-run
receipt and only when hook config enables that path.
