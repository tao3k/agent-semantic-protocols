//! Cache command helpers for the `asp` client.

mod command;
mod locator_artifact;
mod probe;
mod request;
mod structural_index_import;
mod writeback;
mod writeback_analysis_metadata;
mod writeback_artifact_events;
mod writeback_common;
mod writeback_db_reset;
mod writeback_generation;
mod writeback_manifest;
mod writeback_packet;
mod writeback_provider_export;
mod writeback_request;
mod writeback_route_receipt;

#[cfg(test)]
#[path = "../../tests/unit/cache_cli/locator_artifact.rs"]
mod locator_artifact_tests;

pub(crate) use command::run_cache;
#[cfg(test)]
pub(crate) use probe::generation_file_hashes_match;
pub(crate) use probe::{apply_provider_cache_probe, cache_hit_receipt, provider_cache_probe};
pub(crate) use writeback::{
    write_prompt_output_cache_after_provider_success,
    write_query_packet_cache_after_provider_success,
    write_search_packet_cache_after_provider_success,
};
