#![deny(dead_code)]

#[path = "unit/agent_session_interactive_loop.rs"]
mod agent_session_interactive_loop;
#[path = "unit/agent_session_lifecycle_p0.rs"]
mod agent_session_lifecycle_p0;
#[path = "unit/agent_session_runtime_proxy.rs"]
mod agent_session_runtime_proxy;
#[path = "unit/codex_multi_agent_control_plane_owner.rs"]
mod codex_multi_agent_control_plane_owner;
#[path = "unit/context_run_mvcc.rs"]
mod context_run_mvcc;
#[path = "unit/db.rs"]
mod db;
#[path = "unit/db/engine/mod.rs"]
mod db_engine;
#[path = "unit/db/engine_provider_command.rs"]
mod db_engine_provider_command;
#[path = "unit/db/engine_source_index.rs"]
mod db_engine_source_index;
#[path = "unit/db/gerbil_dependency_index.rs"]
mod db_gerbil_dependency_index;
#[path = "unit/env.rs"]
mod env;
#[path = "unit/graph_turbo_cache.rs"]
mod graph_turbo_cache;
#[path = "unit/db/live_source_index_memory.rs"]
mod live_source_index_memory;
#[path = "unit/db/project_scoped_turso_performance.rs"]
mod project_scoped_turso_performance;
#[path = "unit/projection_fixture.rs"]
mod projection_fixture;
#[path = "unit/provider_incremental_probe_batch.rs"]
mod provider_incremental_probe_batch;
#[path = "unit/provider_treesitter_read.rs"]
mod provider_treesitter_read;
#[path = "unit/runtime_generation_admission_gate.rs"]
mod runtime_generation_admission_gate;
#[path = "unit/runtime_server_admission_catalog.rs"]
mod runtime_server_admission_catalog;
#[path = "unit/runtime_server_control.rs"]
mod runtime_server_control;
#[path = "unit/runtime_cache_control.rs"]
mod runtime_cache_control;
#[path = "unit/runtime_server_diagnostics.rs"]
mod runtime_server_diagnostics;
#[path = "unit/runtime_server_generation_admission.rs"]
mod runtime_server_generation_admission;
#[path = "unit/runtime_server_generation_restore.rs"]
mod runtime_server_generation_restore;
#[path = "unit/runtime_server_graph_turbo.rs"]
mod runtime_server_graph_turbo;
#[path = "unit/runtime_server_hook_evaluation.rs"]
mod runtime_server_hook_evaluation;
#[path = "unit/runtime_server_overlay_admission.rs"]
mod runtime_server_overlay_admission;
#[path = "unit/runtime_server_runtime.rs"]
mod runtime_server_runtime;
#[path = "unit/runtime_server_supervisor_reconciliation.rs"]
mod runtime_server_supervisor_reconciliation;
#[path = "unit/runtime_server_workspace.rs"]
mod runtime_server_workspace;
#[path = "unit/runtime_server_workspace_recovery.rs"]
mod runtime_server_workspace_recovery;
#[path = "unit/runtime_server_workspace_resident.rs"]
mod runtime_server_workspace_resident;
#[path = "unit/selector_generation_evidence.rs"]
mod selector_generation_evidence;
#[path = "unit/db/snapshot_fixture.rs"]
mod snapshot_fixture;
#[path = "unit/db/source_index_refresh_perf.rs"]
mod source_index_refresh_perf;
#[path = "unit/test_support.rs"]
mod test_support;
#[path = "unit/turso_mvcc_benchmark.rs"]
mod turso_mvcc_benchmark;
#[path = "unit/turso_mvcc_partition.rs"]
mod turso_mvcc_partition;
#[path = "unit/turso_source_index_materialization.rs"]
mod turso_source_index_materialization;
#[path = "unit/workspace_db_ipc.rs"]
mod workspace_db_ipc;
#[path = "unit/workspace_db_owner_election.rs"]
mod workspace_db_owner_election;
#[path = "unit/workspace_db_registry.rs"]
mod workspace_db_registry;
#[path = "unit/workspace_project_resolution.rs"]
mod workspace_project_resolution;
