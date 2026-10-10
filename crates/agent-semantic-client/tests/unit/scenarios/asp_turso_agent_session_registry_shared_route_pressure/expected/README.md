# Expected

The registry pressure gate spawns multiple child processes that all attempt to
register the same `project_id/root_session_id/name` route.

The run must:

- complete without Turso WAL lock errors;
- complete without unique constraint failures;
- converge to one routable shared route row;
- keep each child-internal registry open/register operation under 500ms;
- avoid provider/native finder execution.
