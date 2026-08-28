//! Provider installation branch boundary.

use crate::command::{installed_provider_artifacts, protocol_binary};

mod archive;
mod binary;
mod cli_support;
mod core;
mod development;
mod release;
mod target;
mod workspace;
mod workspace_receipt;

pub(crate) use core::run_install_command;
