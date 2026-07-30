#![deny(dead_code)]

#[path = "unit/agent_session_interactive_loop.rs"]
mod agent_session_interactive_loop;
#[path = "unit/agent_session_lifecycle_p0.rs"]
mod agent_session_lifecycle_p0;
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
#[path = "unit/global_resident_control.rs"]
mod global_resident_control;
#[path = "unit/resident_query_performance.rs"]
mod resident_query_performance;
#[path = "unit/db/live_source_index_memory.rs"]
mod live_source_index_memory;
#[path = "unit/materialization_fixture.rs"]
mod materialization_fixture;
#[path = "unit/db/project_scoped_turso_performance.rs"]
mod project_scoped_turso_performance;
#[path = "unit/provider_incremental_probe_batch.rs"]
mod provider_incremental_probe_batch;
#[path = "unit/provider_treesitter_read.rs"]
mod provider_treesitter_read;
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
#[path = "unit/workspace_db_ipc.rs"]
mod workspace_db_ipc;
#[path = "unit/workspace_db_owner_election.rs"]
mod workspace_db_owner_election;
#[path = "unit/workspace_db_registry.rs"]
mod workspace_db_registry;
#[path = "unit/workspace_project_resolution.rs"]
mod workspace_project_resolution;
