// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Command tree for the `asp` binary.

pub(crate) mod active_provider_projection;
mod agent_session;
mod ast_patch;
mod cli_help;
mod dispatch;
mod document_provider;
mod healthcheck;
mod install_binary_config_admission;
mod install_provider;
mod provider_install_receipt;

#[cfg(feature = "live-corpus-test")]
pub(crate) mod live_corpus;
mod org_archive;
mod org_capture;
mod org_capture_contract_materialize;
pub(crate) mod org_capture_interactive;
mod org_recall;
mod paths;
pub(crate) mod protocol_binary;
mod protocol_version;
mod provider_dispatch;
#[path = "provider_install_registry_branch/mod.rs"]
mod provider_install_registry;
mod provider_roots;
mod provider_selector;
mod provider_usage;
mod root_language_facade;
mod runtime_server;
mod schema;
mod search_config;

pub(crate) use dispatch::run_protocol_command;
pub(crate) use dispatch::run_protocol_command_started;
pub(in crate::command) use protocol_version::protocol_version_line;
pub(in crate::command) use protocol_version::run_protocol_version_command;
