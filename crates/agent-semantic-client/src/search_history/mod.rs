// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Search command history audit via the graph-turbo artifact timeline.

mod artifact_events;
mod history_audit;
#[path = "../search_history_paths.rs"]
mod search_history_paths;

#[cfg(test)]
pub(crate) use history_audit::artifact_events_packet;
pub(crate) use history_audit::run_search_history;
