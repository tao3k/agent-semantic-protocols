# ASP Schema Manager

`asp-schema-manager` audits the repository's JSON Schema registry without rewriting or
deleting files. It reports reusable validation fragments, direct and transitive
Rust use, unresolved references, and lifecycle recommendations.

Schema families are contractual filename namespaces declared in
`asp-schema-families.v1.json`. Filename prefixes discover membership, while the
stable `familyId`, precedence, owner, and parent relationship remain explicit
registry policy. Cross-family reference candidates are never assigned an
implicit definition owner.

```sh
uv run --project packages/python asp-schema-manager audit --workspace-root .
uv run --project packages/python asp-schema-manager check --workspace-root .
uv run --project packages/python asp-schema-manager families --workspace-root .
```

The lifecycle manifest is explicit policy. An absent Rust string is evidence,
not deletion authority; unreferenced schemas become reviewable removal
candidates and still require a manifest decision before deletion.
