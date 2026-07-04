# Search Owner SourceIndex Missing DB Expected Output

The rendered owner-items trace must include `sourceIndex status=missing-db`,
`source=source-index`, `reason=sourceIndex:missing-db`, and
`next=asp_cache_source-index_refresh` before parser fallback output.

The gate asserts provider process count remains zero and the trace stays under
the benchmark stdout budget.
