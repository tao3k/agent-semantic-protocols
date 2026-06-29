# ASP fzf SourceIndex Warm Path Expected Output

The command must emit `[search-fzf]` seeds with `source=source-index`, `sourceTrace=sourceIndex:used`, and `finder:skipped`.

The marker provider must not be spawned, native finder collection must not run, and the measured SourceIndex collection time must remain below the scenario `max_total`.
