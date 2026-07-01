# SourceIndex warm search pipe input

The scenario builds a temporary Rust package with `src/lib.rs` containing `source_index_fixture`, refreshes the DB Engine SourceIndex, then runs `asp rust search pipe source_index_fixture --workspace . --view seeds`.
