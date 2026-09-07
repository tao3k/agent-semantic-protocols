# Input

The scenario creates a temporary Rust package containing `src/lib.rs` with
`source_index_fixture`, refreshes the Rust client SourceIndex, removes the
provider marker, and then runs:

```sh
asp fd -query source_index_fixture --workspace .
```
