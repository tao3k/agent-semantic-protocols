//! Cache command helpers for the `asp` client.

mod command;
mod locator_artifact;
mod probe;
mod request;
pub(crate) use request::search_cache_forwarded_args;
mod structural_index_import;
mod writeback;
mod writeback_artifact_events;
mod writeback_common;
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
pub(crate) use command::{source_index_refresh_index_owner, source_index_refresh_phase};
#[cfg(test)]
pub(crate) use probe::generation_file_hashes_match;
pub(crate) use probe::{apply_provider_cache_probe, cache_hit_receipt, provider_cache_probe};
pub(crate) use writeback::{
    write_prompt_output_cache_after_provider_success,
    write_query_packet_cache_after_provider_success,
    write_search_packet_cache_after_provider_success,
};
