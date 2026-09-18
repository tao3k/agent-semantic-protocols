# Inputs

The Rust libtest fixture creates a temporary global state root and six child
processes. Each child registers a distinct `session_id` into the same shared
`project_id/root_session_id/name` route so the Turso registry exercises its
route convergence point.
