# Parser Compact Fixtures

This directory is the root contract for parser-owned compact output. Language
providers keep their local implementation tests, but shared snapshot cases live
here so workflow changes can compare languages with the same fixture shape.

The layout follows the snapshot loop:

- `cases/<feature-class>/<case-id>/<language>.json` classifies the case and
  defines the provider commands.
- `projects/<feature-class>/<case-id>/<language>/` contains the fixture project
  used by the provider command.
- `real-output/<feature-class>/<case-id>/<language>/` is generated from the real
  provider output during `--check-provider`.
- `expected-output/<feature-class>/<case-id>/<language>/` is the target snapshot
  checked into the repository.

The runner compares normalized real output with expected output. Query-packet
snapshots intentionally omit `matches[].code`; compact code lives in a sibling
`code.<language-extension>` file such as `code.rs`, `code.py`, or `code.ts` so
code expectations keep language identity without JSON escaping or duplicate
maintenance. The runner only wraps provider commands and compares artifacts;
compact parsing remains owned by `rs-harness`, `ts-harness`, or `py-harness`.

`real-library` cases are fixed source-slice fixtures, not full checked-out
upstream projects. Each Rust, TypeScript, and Python provider must keep at
least three real-library cases from distinct upstream repositories so compact
algorithm changes are measured against large, stable source shapes. Full
project-root exploration belongs in sandtable scenarios, where search flow,
owner resolution, and hook guidance can be measured separately from compact
code projection snapshots.

Token-cost snapshots use the deterministic `byte` tokenizer by default. Pass a
different `--tokenizer` only when intentionally migrating the full expected
token-cost baseline. Refresh expected output with:

```sh
uv run parser-compact-snapshots --case <case-id> --refresh
```
