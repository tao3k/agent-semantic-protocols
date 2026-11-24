#![recursion_limit = "256"]
// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later
#![deny(dead_code)]
#![expect(
    clippy::await_holding_lock,
    reason = "unit tests intentionally hold the process-global environment gate across async install fixtures"
)]

#[path = "unit/command/build_profile.rs"]
mod command_build_profile;

#[path = "unit/ast_patch.rs"]
mod ast_patch;
#[path = "unit/context_product_state.rs"]
mod context_product_state;
#[path = "unit/document_owner_items_hot_path.rs"]
mod document_owner_items_hot_path;
#[path = "unit/exact_projection.rs"]
mod exact_projection;
#[path = "unit/healthcheck.rs"]
mod healthcheck;
#[path = "unit/install_binary_existing_state_production_command.rs"]
mod install_binary_existing_state_production_command;
#[path = "unit/install_binary_production_command.rs"]
mod install_binary_production_command;
#[path = "unit/install_binary_test_guard.rs"]
mod install_binary_test_guard;
#[path = "unit/install_provider_cli.rs"]
mod install_provider_cli;
#[path = "unit/command/live_corpus_public_route.rs"]
mod live_corpus_public_route;
#[path = "unit/live_corpus_registered_languages.rs"]
mod live_corpus_registered_languages;
#[path = "unit/package_dependency_boundary.rs"]
mod package_dependency_boundary;
#[path = "unit/paths_command.rs"]
mod paths_command;
#[path = "unit/command/provider_language_facade.rs"]
mod provider_language_facade;
#[path = "unit/runtime_client_handoff.rs"]
mod runtime_client_handoff;
#[path = "unit/runtime_server_start_readiness.rs"]
mod runtime_server_start_readiness;
#[path = "unit/runtime_status_handoff.rs"]
mod runtime_status_handoff;
#[path = "unit/sandtable_fixtures.rs"]
mod sandtable_fixtures;
