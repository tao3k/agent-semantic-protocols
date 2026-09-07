# Expected

The DB Engine pressure gate spawns multiple child processes that share one
State Core `client.turso` file.

The run must:

- complete without `client.turso-wal` lock errors;
- keep bootstrap and write operations behind `client.turso.operation.lock`;
- keep each child-internal DB open/write/read operation under a subsecond cap;
- read back every final `process-cache-status` generation;
- avoid retired DB routes and provider/native finder execution.
