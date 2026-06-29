# Expected

The command emits compact query-wrapper output with:

- `source=source-index`
- `sourceTrace=sourceIndex:used`
- `finder:skipped`
- `packages=src/lib.rs`
- no provider marker file

The scenario gate parses `collectMs` from stdout and requires it to stay within
`benchmark.toml` while provider and native finder process counts remain zero.
