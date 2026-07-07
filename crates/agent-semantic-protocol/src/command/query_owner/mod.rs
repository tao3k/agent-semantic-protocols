//! ASP-owned bounded owner queries.
//!
//! This path handles explicit owner-file queries without spawning a language
//! provider. Heavy parsing, fallback resolution, and rendering live in owner
//! modules so the command facade stays thin.

mod item;
mod owner_path;
mod python_imports;
mod render;
mod request;
pub(crate) mod runner;
mod rust_items;
mod structural_selector;
mod tree_sitter_items;

pub(super) use item::OwnerItem;
pub(super) use runner::run_asp_fast_owner_query_command;
pub(super) use rust_items::collect_syn_rust_owner_items;
