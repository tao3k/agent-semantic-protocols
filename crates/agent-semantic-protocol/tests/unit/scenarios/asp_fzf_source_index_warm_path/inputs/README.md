# ASP fzf SourceIndex Warm Path Input

The scenario creates a tiny Rust package with `src/lib.rs`, refreshes the Rust-owned SourceIndex, removes the marker-provider side effect file, and then runs:

```sh
asp rust search fzf source_index_fixture owner items tests --workspace . --view seeds
```
