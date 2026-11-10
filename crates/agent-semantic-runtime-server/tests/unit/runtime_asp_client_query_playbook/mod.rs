// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Composes focused Runtime Query playbook test owners.

// Preserve the original test module's explicit access to its source owner's
// private helpers after splitting the oversized case file.
use super::{
    AspClientOperationError, AspClientWorkspaceQueryPlaybookRequest, InitializedWorkspace,
    admit_cold_query_owner_paths, bind_query_materialization_to_request,
    durable_exact_execution_matches_current, durable_projection_is_direct,
    emit_runtime_search_trace_observation, materialize_query_playbook_receipt,
    process_cold_owner_content_digest, query_playbook_generation_provider_targets,
    read_query_projection_handoff, record_settled_client_timing_observations,
    resident_exact_query_projections, runtime_search_trace_budget_micros,
    try_process_cold_exact_owner_replay, workspace_query_materialization_key,
};

mod cases;
