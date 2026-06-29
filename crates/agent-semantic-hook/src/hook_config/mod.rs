//! Optional client-side hook rules loaded on each hook invocation.

mod asp_session_policy;
mod core;

pub use asp_session_policy::AspSessionPolicy;
pub use core::{
    ClientHookConfig, default_client_config_path, default_client_config_template,
    default_client_config_template_for_source_extensions, load_client_config,
    load_client_config_for_project,
};
