// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::path::Path;

use agent_semantic_client_core::ClientCachePath;

#[test]
fn client_cache_path_normalizes_wire_path_text() {
    let path = Path::new("/tmp/agent-semantic-protocol/./client/../client.turso");
    let wire_path = ClientCachePath::from_path(path);

    assert_eq!(
        wire_path.as_str(),
        "/tmp/agent-semantic-protocol/client.turso"
    );
}
