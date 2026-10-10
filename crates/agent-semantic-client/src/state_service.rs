// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Application service for State Home diagnostics.
//!
//! This module composes Runtime identity and Client DB manifest ownership. CLI
//! modules may render its result but must not materialize physical state.

use agent_semantic_client_core::state_core::ResolvedState;
use agent_semantic_client_core::state_core::StateLocateReport;
use agent_semantic_client_db::ClientDbEngine;
use serde::Serialize;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StateHomeSyncReceipt {
    pub(crate) schema_id: &'static str,
    pub(crate) schema_version: u64,
    pub(crate) state: &'static str,
    pub(crate) state_home: std::path::PathBuf,
    pub(crate) removed_entries: Vec<std::path::PathBuf>,
    pub(crate) removed_bytes: u64,
}

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

/// Converge physical State Home output to the closed V1 layout contract.
pub(crate) fn sync_state_home_contract() -> Result<StateHomeSyncReceipt, String> {
    let state_home = agent_semantic_runtime::state_core::resolve_state_home()?;
    let layout = agent_semantic_artifacts::StateHomeLayout::new(&state_home);
    let stale_trash = layout.purge_trash()?;
    let entries = layout.non_contract_entries()?;
    let mut removed_entries = stale_trash.relative_paths;
    removed_entries.extend(entries.iter().map(|entry| entry.relative_path.clone()));
    let mut removed_bytes = entries.iter().fold(stale_trash.byte_count, |total, entry| {
        total.saturating_add(entry.byte_count)
    });
    let mut staged = Vec::with_capacity(entries.len());
    for entry in &entries {
        match layout.stage_non_contract_entry(entry) {
            Ok(removal) => staged.push(removal),
            Err(error) => {
                let mut rollback_errors = Vec::new();
                while let Some(removal) = staged.pop() {
                    if let Err(rollback_error) = removal.rollback() {
                        rollback_errors.push(rollback_error);
                    }
                }
                return if rollback_errors.is_empty() {
                    Err(error)
                } else {
                    Err(format!(
                        "{error}; reasonKind=state-home-contract-sync-rollback-failed errors={}",
                        rollback_errors.join(" | ")
                    ))
                };
            }
        }
    }
    let mut reap_errors = Vec::new();
    for removal in staged {
        if let Err(error) = removal.commit() {
            reap_errors.push(error);
        }
    }
    if !reap_errors.is_empty() {
        return Err(format!(
            "reasonKind=state-home-contract-sync-reap-failed errors={}",
            reap_errors.join(" | ")
        ));
    }
    let completed_trash = layout.purge_trash()?;
    removed_bytes = removed_bytes.saturating_add(completed_trash.byte_count);
    removed_entries.extend(completed_trash.relative_paths);
    removed_entries.sort();
    removed_entries.dedup();
    if !layout.non_contract_entries()?.is_empty() {
        return Err("reasonKind=state-home-contract-sync-incomplete".to_owned());
    }
    Ok(StateHomeSyncReceipt {
        schema_id: "agent.semantic-protocols.state-home-sync-receipt",
        schema_version: 1,
        state: "converged",
        state_home,
        removed_entries,
        removed_bytes,
    })
}
