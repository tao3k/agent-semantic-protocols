// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::RuntimeServerWorkspaceRegistry;

#[test]
fn unadmitted_workspace_durability_is_typed_absence() {
    let registry = RuntimeServerWorkspaceRegistry::new(
        std::env::temp_dir().join("asp-runtime-workspace-durability-unadmitted"),
    )
    .expect("construct runtime workspace registry");

    let durability = registry
        .generation_durability(
            "workspace-not-admitted",
            std::path::Path::new("/workspace/not-admitted"),
        )
        .expect("read unadmitted workspace durability");

    assert_eq!(durability, None);
    assert_eq!(registry.workspace_count(), 0);
}
