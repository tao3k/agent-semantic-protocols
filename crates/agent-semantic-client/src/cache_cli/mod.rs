//! Cache command helpers for the `asp` client.

mod command;
mod locator_artifact;
mod probe;
mod project_registry_gc_args;
pub use project_registry_gc_args::project_registry_gc_clap_command;
mod project_registry_gc_command;
mod request;
pub(crate) use request::search_cache_forwarded_args;
mod source_index_evidence;
mod structural_index_import;
mod turso_migration_args;
pub use turso_migration_args::cache_migration_clap_command;
mod turso_migration_command;
mod writeback;
mod writeback_artifact_events;
mod writeback_common;
mod writeback_generation;
mod writeback_manifest;
mod writeback_packet;
mod writeback_provider_export;
mod writeback_request;

#[cfg(test)]
#[path = "../../tests/unit/cache_cli/locator_artifact.rs"]
mod locator_artifact_tests;
#[cfg(test)]
#[path = "../../tests/unit/cache_cli/project_registry_gc_args.rs"]
mod project_registry_gc_args_tests;
#[cfg(test)]
#[path = "../../tests/unit/cache_cli/turso_migration_args.rs"]
mod turso_migration_args_tests;

pub(crate) use command::run_cache;
#[cfg(test)]
pub(crate) use command::{source_index_refresh_index_owner, source_index_refresh_phase};
#[cfg(test)]
pub(crate) use probe::generation_file_hashes_match;
pub(crate) use probe::{apply_provider_cache_probe, cache_hit_receipt, provider_cache_probe};
pub(crate) use turso_migration_command::run_cache_migration;
pub(crate) use writeback::{
    write_prompt_output_cache_after_provider_success,
    write_search_packet_cache_after_provider_success,
};
