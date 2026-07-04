# SourceIndex warm search pipe expected output

The rendered search pipe output must include `sourceTrace=sourceIndex:used`, `finder:skipped`, `ownerCoverage=bestOwner=src/lib.rs`, and `nextCommand=asp rust search owner src/lib.rs items --query source_index_fixture --workspace . --view seeds`.

The performance gate asserts `sourceIndexHit=true`, `providerProcessCount=0`, `nativeFinderProcessCount=0`, and SourceIndex lookup duration inside the benchmark ceiling.
