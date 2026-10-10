# Expected

The source-index pressure gate runs in the DB Engine crate. One writer refreshes
the Turso-backed source-index while multiple readers query the same client DB.

The run must:

- complete without Turso WAL lock errors;
- return only hit, miss, or bounded busy lookup states during pressure;
- preserve the final indexed owner as a hit;
- avoid provider/native finder execution;
- keep the source-index pressure gate under a subsecond budget.
