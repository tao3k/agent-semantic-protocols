// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Tokio self-scheduling and deterministic joins for independent ASP workspace tasks.

mod plan_order;

pub use plan_order::join_tasks_in_plan_order;
