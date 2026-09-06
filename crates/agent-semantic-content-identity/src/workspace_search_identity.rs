// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Admits one ProjectId/WorkspaceId search scope against exact source identity.

use std::fmt;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

/// Schema identifier for an admitted workspace Search identity.
pub const WORKSPACE_SEARCH_IDENTITY_SCHEMA_ID: &str =
    "agent.semantic-protocols.workspace-search-identity";
/// Schema version for admitted workspace Search identities.
pub const WORKSPACE_SEARCH_IDENTITY_SCHEMA_VERSION: &str = "1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Whether Search is bound to the workspace or one selected package.
pub enum WorkspaceSearchScopeKindV1 {
    Package,
    Workspace,
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Candidate roots and counters supplied to Search identity admission.
pub struct WorkspaceSearchIdentityInputV1 {
    pub language_id: String,
    pub provider_id: String,
    pub scope_kind: WorkspaceSearchScopeKindV1,
    pub requested_discovery_root: PathBuf,
    pub cargo_workspace_root: PathBuf,
    pub selected_package_root: Option<PathBuf>,
    pub provider_tool_root: PathBuf,
    pub envelope_root: PathBuf,
    pub root_count: u32,
    pub owner_count: u32,
    pub leaf_count: u32,
    pub selector_count: u32,
    pub source_snapshot_root_digest: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Validated ProjectId/WorkspaceId Search scope and content root.
pub struct WorkspaceSearchIdentityV1 {
    language_id: String,
    provider_id: String,
    scope_kind: WorkspaceSearchScopeKindV1,
    requested_discovery_root: PathBuf,
    cargo_workspace_root: PathBuf,
    selected_package_root: Option<PathBuf>,
    provider_tool_root: PathBuf,
    envelope_root: PathBuf,
    root_count: u32,
    owner_count: u32,
    leaf_count: u32,
    selector_count: u32,
    source_snapshot_root_digest: [u8; 32],
    identity_digest: [u8; 32],
}

impl WorkspaceSearchIdentityV1 {
    pub fn admit(
        input: WorkspaceSearchIdentityInputV1,
    ) -> Result<Self, WorkspaceSearchIdentityErrorV1> {
        if input.language_id.is_empty() || input.provider_id.is_empty() {
            return Err(WorkspaceSearchIdentityErrorV1::MissingProviderIdentity);
        }
        for (field, path) in [
            (
                "requestedDiscoveryRoot",
                input.requested_discovery_root.as_path(),
            ),
            ("cargoWorkspaceRoot", input.cargo_workspace_root.as_path()),
            ("providerToolRoot", input.provider_tool_root.as_path()),
            ("envelopeRoot", input.envelope_root.as_path()),
        ] {
            require_absolute(field, path)?;
        }
        if let Some(selected_package_root) = input.selected_package_root.as_deref() {
            require_absolute("selectedPackageRoot", selected_package_root)?;
        }
        match input.scope_kind {
            WorkspaceSearchScopeKindV1::Package => {
                let selected_package_root = input
                    .selected_package_root
                    .as_deref()
                    .ok_or(WorkspaceSearchIdentityErrorV1::MissingSelectedPackageRoot)?;
                if input.requested_discovery_root != selected_package_root
                    || input.envelope_root != selected_package_root
                {
                    return Err(WorkspaceSearchIdentityErrorV1::PackageScopeMismatch {
                        requested_discovery_root: input.requested_discovery_root,
                        selected_package_root: selected_package_root.to_path_buf(),
                        envelope_root: input.envelope_root,
                    });
                }
            }
            WorkspaceSearchScopeKindV1::Workspace => {
                if input.selected_package_root.is_some() {
                    return Err(WorkspaceSearchIdentityErrorV1::UnexpectedSelectedPackageRoot);
                }
                if input.requested_discovery_root != input.cargo_workspace_root
                    || input.envelope_root != input.cargo_workspace_root
                {
                    return Err(WorkspaceSearchIdentityErrorV1::WorkspaceScopeMismatch {
                        requested_discovery_root: input.requested_discovery_root,
                        cargo_workspace_root: input.cargo_workspace_root,
                        envelope_root: input.envelope_root,
                    });
                }
            }
        }
        if input.root_count != 1
            || input.owner_count == 0
            || input.owner_count != input.leaf_count
            || input.selector_count == 0
        {
            return Err(WorkspaceSearchIdentityErrorV1::IncompleteOwnerEnvelope {
                root_count: input.root_count,
                owner_count: input.owner_count,
                leaf_count: input.leaf_count,
                selector_count: input.selector_count,
            });
        }
        let identity_digest = derive_workspace_search_identity_digest_v1(&input);
        Ok(Self {
            language_id: input.language_id,
            provider_id: input.provider_id,
            scope_kind: input.scope_kind,
            requested_discovery_root: input.requested_discovery_root,
            cargo_workspace_root: input.cargo_workspace_root,
            selected_package_root: input.selected_package_root,
            provider_tool_root: input.provider_tool_root,
            envelope_root: input.envelope_root,
            root_count: input.root_count,
            owner_count: input.owner_count,
            leaf_count: input.leaf_count,
            selector_count: input.selector_count,
            source_snapshot_root_digest: input.source_snapshot_root_digest,
            identity_digest,
        })
    }

    pub fn language_id(&self) -> &str {
        &self.language_id
    }

    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }

    pub fn scope_kind(&self) -> WorkspaceSearchScopeKindV1 {
        self.scope_kind
    }

    pub fn requested_discovery_root(&self) -> &Path {
        &self.requested_discovery_root
    }

    pub fn cargo_workspace_root(&self) -> &Path {
        &self.cargo_workspace_root
    }

    pub fn selected_package_root(&self) -> Option<&Path> {
        self.selected_package_root.as_deref()
    }

    pub fn provider_tool_root(&self) -> &Path {
        &self.provider_tool_root
    }

    pub fn envelope_root(&self) -> &Path {
        &self.envelope_root
    }

    pub fn root_count(&self) -> u32 {
        self.root_count
    }

    pub fn owner_count(&self) -> u32 {
        self.owner_count
    }

    pub fn leaf_count(&self) -> u32 {
        self.leaf_count
    }

    pub fn selector_count(&self) -> u32 {
        self.selector_count
    }

    pub fn source_snapshot_root_digest(&self) -> &[u8; 32] {
        &self.source_snapshot_root_digest
    }

    pub fn identity_digest(&self) -> &[u8; 32] {
        &self.identity_digest
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Deterministic failure while admitting a workspace Search identity.
pub enum WorkspaceSearchIdentityErrorV1 {
    IncompleteOwnerEnvelope {
        root_count: u32,
        owner_count: u32,
        leaf_count: u32,
        selector_count: u32,
    },
    MissingProviderIdentity,
    MissingSelectedPackageRoot,
    NonAbsolutePath {
        field: &'static str,
        path: PathBuf,
    },
    PackageScopeMismatch {
        requested_discovery_root: PathBuf,
        selected_package_root: PathBuf,
        envelope_root: PathBuf,
    },
    PackageRootOutsideWorkspace {
        cargo_workspace_root: PathBuf,
        package_root: PathBuf,
    },
    UnexpectedSelectedPackageRoot,
    WorkspaceScopeMismatch {
        requested_discovery_root: PathBuf,
        cargo_workspace_root: PathBuf,
        envelope_root: PathBuf,
    },
}

impl fmt::Display for WorkspaceSearchIdentityErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for WorkspaceSearchIdentityErrorV1 {}

/// Resolves a requested discovery root to one canonical workspace member root.
pub fn resolve_workspace_member_root_v1(
    cargo_workspace_root: &Path,
    package_root: &Path,
) -> Result<PathBuf, WorkspaceSearchIdentityErrorV1> {
    require_absolute("cargoWorkspaceRoot", cargo_workspace_root)?;
    let workspace_root = normalize_absolute_path_v1(cargo_workspace_root)?;
    let resolved = if package_root.is_absolute() {
        normalize_absolute_path_v1(package_root)?
    } else {
        normalize_absolute_path_v1(&workspace_root.join(package_root))?
    };
    if !resolved.starts_with(&workspace_root) {
        return Err(
            WorkspaceSearchIdentityErrorV1::PackageRootOutsideWorkspace {
                cargo_workspace_root: workspace_root,
                package_root: resolved,
            },
        );
    }
    Ok(resolved)
}

fn require_absolute(
    field: &'static str,
    path: &Path,
) -> Result<(), WorkspaceSearchIdentityErrorV1> {
    if path.is_absolute() {
        Ok(())
    } else {
        Err(WorkspaceSearchIdentityErrorV1::NonAbsolutePath {
            field,
            path: path.to_path_buf(),
        })
    }
}

fn normalize_absolute_path_v1(path: &Path) -> Result<PathBuf, WorkspaceSearchIdentityErrorV1> {
    require_absolute("path", path)?;
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(Path::new("/")),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(component) => normalized.push(component),
        }
    }
    Ok(normalized)
}

fn derive_workspace_search_identity_digest_v1(input: &WorkspaceSearchIdentityInputV1) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hash_field(&mut hasher, input.language_id.as_bytes());
    hash_field(&mut hasher, input.provider_id.as_bytes());
    hash_field(
        &mut hasher,
        match input.scope_kind {
            WorkspaceSearchScopeKindV1::Package => b"package",
            WorkspaceSearchScopeKindV1::Workspace => b"workspace",
        },
    );
    hash_path(&mut hasher, &input.requested_discovery_root);
    hash_path(&mut hasher, &input.cargo_workspace_root);
    match input.selected_package_root.as_deref() {
        Some(path) => {
            hash_field(&mut hasher, b"selected");
            hash_path(&mut hasher, path);
        }
        None => hash_field(&mut hasher, b"none"),
    }
    hash_path(&mut hasher, &input.provider_tool_root);
    hash_path(&mut hasher, &input.envelope_root);
    hash_field(&mut hasher, &input.root_count.to_le_bytes());
    hash_field(&mut hasher, &input.owner_count.to_le_bytes());
    hash_field(&mut hasher, &input.leaf_count.to_le_bytes());
    hash_field(&mut hasher, &input.selector_count.to_le_bytes());
    hash_field(&mut hasher, &input.source_snapshot_root_digest);
    *hasher.finalize().as_bytes()
}

fn hash_path(hasher: &mut blake3::Hasher, path: &Path) {
    hash_field(hasher, path.as_os_str().as_encoded_bytes());
}

fn hash_field(hasher: &mut blake3::Hasher, field: &[u8]) {
    hasher.update(&(field.len() as u64).to_le_bytes());
    hasher.update(field);
}

#[cfg(test)]
#[path = "../tests/unit/workspace_search_identity.rs"]
mod tests;
