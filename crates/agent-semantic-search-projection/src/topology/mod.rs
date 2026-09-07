// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Compact graph rendering for schema-backed `search` packets.

mod actions;
mod aliases;
mod api;
mod header;
mod packet;
mod pipeline;
mod profiles;

pub use api::SEARCH_ROOT_ID;
pub use api::TERSE_GRAPH_MICRO_LEGEND;
pub use api::TopologyProjectionOptions;
pub use api::render_search_topology_projection;
