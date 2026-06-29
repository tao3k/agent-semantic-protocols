# SourceIndex warm search pipe expected output

The rendered search pipe output must include `sourceTrace=sourceIndex:used`, `finder:skipped`, `ownerCoverage=bestOwner=src/lib.rs`, and `nextCommand=asp rust query --selector src/lib.rs:1:2 --workspace . --code`.

The performance gate asserts `sourceIndexHit=true`, `providerProcessCount=0`, `nativeFinderProcessCount=0`, and SourceIndex lookup duration inside the benchmark ceiling.
