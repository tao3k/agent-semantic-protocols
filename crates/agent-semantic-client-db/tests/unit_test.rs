#![deny(dead_code)]

#[path = "unit/agent_session_interactive_loop.rs"]
mod agent_session_interactive_loop;
#[path = "unit/agent_session_lifecycle_p0.rs"]
mod agent_session_lifecycle_p0;
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
#[path = "unit/db/snapshot_fixture.rs"]
mod snapshot_fixture;
#[path = "unit/db/source_index_refresh_perf.rs"]
mod source_index_refresh_perf;
