// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Application service for State Home diagnostics.
//!
//! This module composes Runtime identity and Client DB manifest ownership. CLI
//! modules may render its result but must not materialize physical state.

use agent_semantic_client_core::state_core::ResolvedState;
use agent_semantic_client_core::state_core::StateLocateReport;
use agent_semantic_client_db::ClientDbEngine;

pub(crate) fn locate_current_workspace() -> Result<StateLocateReport, String> {
    let cwd = std::env::current_dir().map_err(|error| format!("resolve cwd: {error}"))?;
    let state = ResolvedState::resolve(&cwd)?;
    state.ensure_workspace_state_layout()?;
    let engine = ClientDbEngine::from_resolved_state(&state);
    engine.write_manifest()?;
    let engine_report = engine.inspect();
    let mut report = state.locate_report();
    report.db_path = engine_report.db_path;
    report.backend = engine_report.backend.to_string();
    Ok(report)
}
