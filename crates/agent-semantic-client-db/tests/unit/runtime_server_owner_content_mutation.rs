// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_client_db::runtime_server_workspace::RuntimeServerWorkspaceRegistry;
use agent_semantic_client_db::runtime_server_workspace::WorkspaceOwnerSnapshot;
use agent_semantic_client_db::runtime_server_workspace::WorkspaceRecoverySource;

#[allow(
    dead_code,
    reason = "shared projection fixtures are selected independently by test binaries"
)]
#[path = "projection_capability_fixture.rs"]
mod fixture;
#[allow(
    dead_code,
    reason = "shared overlay fixtures are selected independently by test binaries"
)]
#[path = "runtime_server_overlay_admission/fixture.rs"]
mod overlay_fixture;
use overlay_fixture::fixture_root;
use overlay_fixture::generation;

#[path = "runtime_server_overlay_admission/owner_content_mutation.rs"]
mod owner_content_mutation;
