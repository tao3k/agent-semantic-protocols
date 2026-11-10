// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Integration tests for the ASP Rust workspace policy adapter.

#[path = "integration/publication_admission.rs"]
mod publication_admission;

#[cfg(feature = "workspace-policy")]
#[path = "integration/workspace_policy.rs"]
mod workspace_policy;
