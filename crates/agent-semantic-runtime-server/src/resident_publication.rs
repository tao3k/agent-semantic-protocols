// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Runtime readiness path admission.
//!
//! Runtime artifact slot publication used to live in this module. That second
//! publication authority was removed: immutable bundle publication and bundle
//! retention are now the only artifact lifecycle owners.

pub async fn resident_readiness_root(
    state_home: &std::path::Path,
) -> Result<crate::readiness::RuntimeServerReadinessRoot, String> {
    use std::os::unix::ffi::OsStrExt as _;
    use std::os::unix::fs::MetadataExt as _;
    use std::os::unix::fs::PermissionsExt as _;

    let canonical_state_home = tokio::fs::canonicalize(state_home).await.map_err(|error| {
        format!(
            "canonicalize Runtime State Home {} for readiness identity: {error}",
            state_home.display()
        )
    })?;
    let expected_uid = tokio::fs::symlink_metadata(&canonical_state_home)
        .await
        .map_err(|error| {
            format!(
                "inspect canonical Runtime State Home {}: {error}",
                canonical_state_home.display()
            )
        })?
        .uid();
    let runtime_base =
        agent_semantic_client_db::runtime_server_control::runtime_server_runtime_base(state_home)?;
    let root = agent_semantic_artifacts::RuntimeServingStateLayout::from_root(runtime_base.clone())
        .readiness();
    let socket = root.join("r").join(format!("{}.sock", "0".repeat(32)));
    const MAX_PORTABLE_UNIX_SOCKET_PATH_BYTES: usize = 100;
    if socket.as_os_str().as_bytes().len() > MAX_PORTABLE_UNIX_SOCKET_PATH_BYTES {
        return Err(format!(
            "reasonKind=runtime-readiness-path-unrepresentable Runtime readiness socket identity exceeds portable sun_path budget: bytes={} path={}",
            socket.as_os_str().as_bytes().len(),
            socket.display()
        ));
    }
    tokio::fs::create_dir_all(&root).await.map_err(|error| {
        format!(
            "create Runtime readiness directory {}: {error}",
            root.display()
        )
    })?;
    let canonical_root = tokio::fs::canonicalize(&root).await.map_err(|error| {
        format!(
            "canonicalize Runtime readiness directory {}: {error}",
            root.display()
        )
    })?;
    if !canonical_root.starts_with(&canonical_state_home) {
        return Err(format!(
            "reasonKind=runtime-readiness-state-home-escape Runtime readiness directory escaped canonical State Home: stateHome={} readinessRoot={}",
            canonical_state_home.display(),
            canonical_root.display()
        ));
    }
    for directory in [runtime_base.as_path(), canonical_root.as_path()] {
        let metadata = tokio::fs::symlink_metadata(directory)
            .await
            .map_err(|error| {
                format!(
                    "inspect Runtime readiness directory {}: {error}",
                    directory.display()
                )
            })?;
        if !metadata.file_type().is_dir()
            || metadata.file_type().is_symlink()
            || metadata.uid() != expected_uid
        {
            return Err(format!(
                "Runtime readiness directory is not a non-symlink State Home-owned directory: {}",
                directory.display()
            ));
        }
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o700);
        tokio::fs::set_permissions(directory, permissions)
            .await
            .map_err(|error| {
                format!(
                    "protect Runtime readiness directory {}: {error}",
                    directory.display()
                )
            })?;
    }
    crate::readiness::RuntimeServerReadinessRoot::new(canonical_root)
}
