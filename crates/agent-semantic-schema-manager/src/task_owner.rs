// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Tracked Tokio boundary for Schema Manager filesystem and digest work.

use agent_semantic_workspace_scheduler::RuntimeServerOwnedTask;

pub(super) async fn run_blocking<T, F>(name: &'static str, operation: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    RuntimeServerOwnedTask::spawn_blocking(name, operation)
        .join()
        .await?
}
