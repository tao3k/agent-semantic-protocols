// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use std::path::{Path, PathBuf};

pub async fn read_provider_install_catalog(
    catalog_path: impl Into<PathBuf>,
) -> Result<agent_semantic_provider_protocol::ProviderInstallRegister, String> {
    let path = catalog_path.into();
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|error| format!("read provider install catalog {}: {error}", path.display()))?;
    agent_semantic_provider_protocol::parse_provider_install_register(&bytes)
}

pub fn provider_install_catalog_path(root: &Path) -> PathBuf {
    root.join("schemas/provider-install-register.json")
}
