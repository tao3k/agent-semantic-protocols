<!--
SPDX-FileCopyrightText: 2026 tao3k team and Contributors
SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only
-->

# Inputs

The Rust libtest fixture creates a temporary global state root and six child
processes. Each child registers a distinct `session_id` into the same shared
`project_id/root_session_id/name` route so the Turso registry exercises its
route convergence point.
