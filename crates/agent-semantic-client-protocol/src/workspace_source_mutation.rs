// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Language-neutral workspace source mutation contract.

use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeSet;

/// Canonical schema identity for workspace source mutations.
pub const WORKSPACE_SOURCE_MUTATION_SCHEMA_ID: &str =
    "agent.semantic-protocols.workspace-source-mutation";
/// Schema version carried by the contract payload.
pub const WORKSPACE_SOURCE_MUTATION_SCHEMA_VERSION: &str = "1";

macro_rules! non_empty_identity {
    ($name:ident) => {
        #[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Creates a non-empty identity.
            pub fn new(value: impl Into<String>) -> Result<Self, WorkspaceSourceMutationError> {
                let value = value.into();
                if value.is_empty() {
                    return Err(WorkspaceSourceMutationError::EmptyIdentity {
                        identity: stringify!($name),
                    });
                }
                Ok(Self(value))
            }

            /// Returns the canonical string form.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

non_empty_identity!(WorkspaceMutationId);
non_empty_identity!(WorkspaceIdentity);
non_empty_identity!(WorkspaceGenerationDigest);
non_empty_identity!(SourceSnapshotDigest);

/// Canonical workspace-relative owner path.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct SourceOwnerPath(String);

impl SourceOwnerPath {
    /// Creates a normalized workspace-relative path.
    pub fn new(value: impl Into<String>) -> Result<Self, WorkspaceSourceMutationError> {
        let value = value.into();
        let valid = !value.is_empty()
            && !value.starts_with('/')
            && !value.contains('\\')
            && value
                .split('/')
                .all(|segment| !segment.is_empty() && segment != "." && segment != "..");
        if !valid {
            return Err(WorkspaceSourceMutationError::InvalidOwnerPath { owner_path: value });
        }
        Ok(Self(value))
    }

    /// Returns the canonical path string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One source owner whose content changed.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChangedSourceOwner {
    /// Workspace-relative canonical owner path.
    pub owner_path: SourceOwnerPath,
    /// Content identity observed by the client ingress.
    pub source_snapshot_digest: SourceSnapshotDigest,
}

/// One source owner removed from the workspace.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemovedSourceOwner {
    /// Workspace-relative canonical owner path.
    pub owner_path: SourceOwnerPath,
}

/// Atomic mutation consumed by the ASP Server resident writer.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceSourceMutation {
    /// Contract identity.
    pub schema_id: String,
    /// Contract version; version never appears in the Rust type namespace.
    pub schema_version: String,
    /// Stable id used to coalesce duplicate delivery.
    pub mutation_id: WorkspaceMutationId,
    /// Canonical workspace identity owned by the server.
    pub workspace_identity: WorkspaceIdentity,
    /// Generation observed before this mutation, when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_generation_digest: Option<WorkspaceGenerationDigest>,
    /// Changed owners in strictly increasing canonical path order.
    pub changed_owners: Vec<ChangedSourceOwner>,
    /// Removed owners in strictly increasing canonical path order.
    pub removed_owners: Vec<RemovedSourceOwner>,
}

impl WorkspaceSourceMutation {
    /// Builds and validates a deterministic mutation payload.
    pub fn new(
        mutation_id: WorkspaceMutationId,
        workspace_identity: WorkspaceIdentity,
        base_generation_digest: Option<WorkspaceGenerationDigest>,
        changed_owners: Vec<ChangedSourceOwner>,
        removed_owners: Vec<RemovedSourceOwner>,
    ) -> Result<Self, WorkspaceSourceMutationError> {
        let mutation = Self {
            schema_id: WORKSPACE_SOURCE_MUTATION_SCHEMA_ID.to_owned(),
            schema_version: WORKSPACE_SOURCE_MUTATION_SCHEMA_VERSION.to_owned(),
            mutation_id,
            workspace_identity,
            base_generation_digest,
            changed_owners,
            removed_owners,
        };
        mutation.validate()?;
        Ok(mutation)
    }

    /// Verifies schema identity, stable ordering, uniqueness, and disjoint sets.
    pub fn validate(&self) -> Result<(), WorkspaceSourceMutationError> {
        if self.schema_id != WORKSPACE_SOURCE_MUTATION_SCHEMA_ID
            || self.schema_version != WORKSPACE_SOURCE_MUTATION_SCHEMA_VERSION
        {
            return Err(WorkspaceSourceMutationError::SchemaIdentityMismatch);
        }
        if self.changed_owners.is_empty() && self.removed_owners.is_empty() {
            return Err(WorkspaceSourceMutationError::EmptyMutation);
        }

        let changed = ordered_owner_paths(
            self.changed_owners
                .iter()
                .map(|owner| owner.owner_path.as_str()),
            "changedOwners",
        )?;
        let removed = ordered_owner_paths(
            self.removed_owners
                .iter()
                .map(|owner| owner.owner_path.as_str()),
            "removedOwners",
        )?;

        if let Some(owner_path) = changed.intersection(&removed).next() {
            return Err(WorkspaceSourceMutationError::ConflictingOwner {
                owner_path: (*owner_path).to_owned(),
            });
        }
        Ok(())
    }
}

fn ordered_owner_paths<'a>(
    owner_paths: impl Iterator<Item = &'a str>,
    field: &'static str,
) -> Result<BTreeSet<&'a str>, WorkspaceSourceMutationError> {
    let mut previous = None;
    let mut owners = BTreeSet::new();
    for owner_path in owner_paths {
        if owner_path.is_empty() {
            return Err(WorkspaceSourceMutationError::EmptyIdentity {
                identity: "SourceOwnerPath",
            });
        }
        if previous.is_some_and(|previous| previous >= owner_path) {
            return Err(WorkspaceSourceMutationError::OwnersNotStrictlyOrdered { field });
        }
        owners.insert(owner_path);
        previous = Some(owner_path);
    }
    Ok(owners)
}

/// Validation failures for the workspace mutation boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkspaceSourceMutationError {
    /// A typed identity was empty.
    EmptyIdentity { identity: &'static str },
    /// Schema id or version did not match the contract.
    SchemaIdentityMismatch,
    /// The mutation did not contain a changed or removed owner.
    EmptyMutation,
    /// An owner path was not normalized or escaped the workspace.
    InvalidOwnerPath { owner_path: String },
    /// An owner set was unordered or contained a duplicate.
    OwnersNotStrictlyOrdered { field: &'static str },
    /// The same owner appeared in both changed and removed sets.
    ConflictingOwner { owner_path: String },
}

impl std::fmt::Display for WorkspaceSourceMutationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyIdentity { identity } => write!(formatter, "{identity} must not be empty"),
            Self::SchemaIdentityMismatch => {
                formatter.write_str("workspace mutation schema identity mismatch")
            }
            Self::EmptyMutation => formatter.write_str("workspace mutation must not be empty"),
            Self::InvalidOwnerPath { owner_path } => {
                write!(
                    formatter,
                    "invalid workspace-relative owner path: {owner_path}"
                )
            }
            Self::OwnersNotStrictlyOrdered { field } => {
                write!(formatter, "{field} must be strictly ordered and unique")
            }
            Self::ConflictingOwner { owner_path } => {
                write!(
                    formatter,
                    "owner appears in changed and removed sets: {owner_path}"
                )
            }
        }
    }
}

impl std::error::Error for WorkspaceSourceMutationError {}
