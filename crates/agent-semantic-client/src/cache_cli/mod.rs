//! Cache command helpers for the `asp` client.

mod command;
mod project_registry_gc_args;
pub use project_registry_gc_args::{
    project_registry_clean_clap_command, project_registry_gc_clap_command,
};
mod project_registry_gc_command;
pub(crate) use project_registry_gc_command::run_project_registry_clean;

#[cfg(test)]
#[path = "../../tests/unit/cache_cli/project_registry_gc_args.rs"]
mod project_registry_gc_args_tests;

pub(crate) use command::run_cache;
