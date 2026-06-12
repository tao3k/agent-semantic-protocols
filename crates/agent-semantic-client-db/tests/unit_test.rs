#![deny(dead_code)]

#[path = "unit/db.rs"]
mod db;
#[path = "unit/db/artifact_events.rs"]
mod db_artifact_events;
#[path = "unit/db/invalidation.rs"]
mod db_invalidation;
#[path = "unit/db/provider_command_selection.rs"]
mod db_provider_command_selection;
#[path = "unit/db/recent_generations.rs"]
mod db_recent_generations;
#[path = "unit/db/structural_index.rs"]
mod db_structural_index;
#[path = "unit/db/syntax_query.rs"]
mod db_syntax_query;
#[path = "unit/db/syntax_query_flush.rs"]
mod db_syntax_query_flush;
#[path = "unit/db/syntax_query_structural.rs"]
mod db_syntax_query_structural;
