#![deny(dead_code)]

#[path = "unit/projection_capability_fixture.rs"]
mod fixture;

#[path = "unit/active_generation_projection_capability.rs"]
mod active_generation_projection_capability;
#[path = "unit/agent_session_registry_publication.rs"]
mod agent_session_registry_publication;
#[path = "unit/agent_session_runtime_proxy.rs"]
mod agent_session_runtime_proxy;
#[path = "unit/codex_multi_agent_control_plane_owner.rs"]
mod codex_multi_agent_control_plane_owner;
#[path = "unit/content_binding.rs"]
mod content_binding;
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
#[path = "unit/runtime_cache_control.rs"]
mod runtime_cache_control;
#[path = "unit/runtime_generation_admission_gate.rs"]
mod runtime_generation_admission_gate;
#[path = "unit/runtime_provider_register.rs"]
mod runtime_provider_register;
#[path = "unit/runtime_search_authority_fixture.rs"]
mod runtime_search_authority_fixture;
#[path = "unit/runtime_search_service_cancellation.rs"]
mod runtime_search_service_cancellation;
#[path = "unit/runtime_server_admission_catalog.rs"]
mod runtime_server_admission_catalog;
#[path = "unit/runtime_server_control.rs"]
mod runtime_server_control;
#[path = "unit/runtime_server_control_authority.rs"]
mod runtime_server_control_authority;
#[path = "unit/runtime_server_control_security.rs"]
mod runtime_server_control_security;
#[path = "unit/runtime_server_diagnostics.rs"]
mod runtime_server_diagnostics;
#[path = "unit/runtime_server_endpoint_v1_migration.rs"]
mod runtime_server_endpoint_v1_migration;
#[path = "unit/runtime_server_generation_admission.rs"]
mod runtime_server_generation_admission;
#[path = "unit/runtime_server_generation_restore.rs"]
mod runtime_server_generation_restore;
#[path = "unit/runtime_server_health.rs"]
mod runtime_server_health;
#[path = "unit/runtime_server_lifecycle_coordinator.rs"]
mod runtime_server_lifecycle_coordinator;
#[path = "unit/runtime_server_operator_stop.rs"]
mod runtime_server_operator_stop;
#[path = "unit/runtime_server_overlay_admission.rs"]
mod runtime_server_overlay_admission;
#[path = "unit/runtime_server_owner_receipt.rs"]
mod runtime_server_owner_receipt;
#[path = "unit/runtime_server_runtime.rs"]
mod runtime_server_runtime;
#[path = "unit/runtime_server_state_home_isolation.rs"]
mod runtime_server_state_home_isolation;
#[path = "unit/runtime_server_supervisor_endpoint_v1_migration.rs"]
mod runtime_server_supervisor_endpoint_v1_migration;
#[path = "unit/runtime_server_supervisor_reconciliation.rs"]
mod runtime_server_supervisor_reconciliation;
#[path = "unit/runtime_server_workspace.rs"]
mod runtime_server_workspace;
#[path = "unit/runtime_server_workspace_recovery.rs"]
mod runtime_server_workspace_recovery;
#[path = "unit/runtime_server_workspace_resident.rs"]
mod runtime_server_workspace_resident;
#[path = "unit/runtime_telemetry_bus.rs"]
mod runtime_telemetry_bus;
#[path = "unit/search_incident.rs"]
mod search_incident;
#[path = "unit/selector_generation_evidence.rs"]
mod selector_generation_evidence;
#[path = "unit/seqlock_json_memory.rs"]
mod seqlock_json_memory;
#[path = "unit/session_control_plane.rs"]
mod session_control_plane;
#[path = "unit/session_control_plane_ipc.rs"]
mod session_control_plane_ipc;
#[path = "unit/db/snapshot_fixture.rs"]
mod snapshot_fixture;
#[path = "unit/source_index_fixture.rs"]
mod source_index_fixture;
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
#[path = "unit/workspace_db_registry.rs"]
mod workspace_db_registry;
#[path = "unit/workspace_project_resolution.rs"]
mod workspace_project_resolution;
#[path = "unit/workspace_runtime_selector_wire.rs"]
mod workspace_runtime_selector_wire;
#[path = "unit/workspace_runtime_session_capability.rs"]
mod workspace_runtime_session_capability;
