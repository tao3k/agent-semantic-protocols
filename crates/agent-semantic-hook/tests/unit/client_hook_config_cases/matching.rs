// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

pub(super) use super::common::ClientHookConfig;
pub(super) use super::common::DecisionKind;
pub(super) use super::common::HookClassificationRequest;
pub(super) use super::common::classify_hook_with_config;
pub(super) use super::common::fs;
pub(super) use super::common::json;
pub(super) use super::common::load_client_config;
pub(super) use super::common::load_client_config_for_project;
pub(super) use super::common::registry;
pub(super) use super::common::temp_root;
pub(super) use super::common::with_direct_dispatch_roles;

#[path = "matching/materialization.rs"]
mod materialization;
#[path = "matching/policy_merge.rs"]
mod policy_merge;
