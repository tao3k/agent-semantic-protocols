// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::{workspace_generation_directory, workspace_generation_pointer_path};
use std::path::Path;

#[test]
fn producer_and_consumer_share_one_scoped_generation_address() {
    let store = Path::new("/tmp/asp-runtime-server-test/workspaces");
    let project = Path::new("/checkout/crates/client-db");
    let directory = workspace_generation_directory(store, "workspace-test", project).unwrap();
    assert_eq!(
        workspace_generation_pointer_path(store, "workspace-test", project).unwrap(),
        directory.join("active-generation.pointer")
    );
    assert!(directory.starts_with(store.join("workspace-test").join("scopes")));
    assert!(directory.ends_with("generations"));
}

#[test]
fn source_scope_address_rejects_ambiguous_identity() {
    assert!(
        workspace_generation_directory(Path::new("/tmp/workspaces"), "", Path::new("/checkout"))
            .is_err()
    );
    assert!(
        workspace_generation_directory(
            Path::new("/tmp/workspaces"),
            "workspace-test",
            Path::new("relative/project")
        )
        .is_err()
    );
}
