// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::collections::HashMap;

pub fn graph_frontier_has_only_owner_or_topology_nodes(kinds: &HashMap<String, String>) -> bool {
    !kinds.is_empty()
        && kinds.values().all(|kind| {
            matches!(
                kind.as_str(),
                "owner" | "workspace" | "provider-root" | "submodule"
            )
        })
}
