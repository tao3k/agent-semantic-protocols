//! Source-snapshot identity and resolution evidence bound to provider digests.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Schema identifier for deterministic source snapshot evidence.
pub const SOURCE_SNAPSHOT_SCHEMA_ID: &str = "asp.source-snapshot.v1";
/// Digest algorithm contract used to bind paths and file contents into a snapshot root.
pub const SOURCE_SNAPSHOT_ALGORITHM: &str = "blake3-merkle-v1";

/// Commit a provider identity or registry fingerprint to the artifact hash domain.
pub fn provider_digest(identity: impl AsRef<[u8]>) -> String {
    crate::ArtifactHash::blake3(identity).value
}
/// Schema identifier for authority and state evidence from snapshot-bound resolution.
pub const SOURCE_RESOLUTION_SCHEMA_ID: &str = "asp.source-resolution.v1";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
/// Authority-bearing origin of the source bytes represented by a snapshot.
pub enum SourceSnapshotKind {
    /// Bytes read from the live filesystem.
    Filesystem,
    /// Unsaved bytes supplied by an editor buffer overlay.
    EditorBuffer,
    /// Bytes resolved from an immutable Git tree.
    GitTree,
    /// Snapshot derived by applying explicit dirty-path overlays to a base root.
    DerivedOverlay,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// Digest evidence for one immutable view of source content and any overlay lineage.
pub struct SourceSnapshotEvidence {
    /// Schema contract for interpreting the snapshot evidence fields.
    pub schema_id: String,
    /// Digest algorithm used for the snapshot root and related digests.
    pub algorithm: String,
    /// Deterministic digest of the complete source snapshot.
    pub root_digest: String,
    /// Origin of the source bytes represented by this snapshot.
    pub source_kind: SourceSnapshotKind,
    /// Number of path/content leaves committed into the root digest.
    pub leaf_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Parent snapshot root when this evidence represents an overlay.
    pub base_root_digest: Option<String>,
    /// Digest binding the snapshot to the provider implementation that produced it.
    pub provider_digest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Digest of the ordered dirty-path set applied over the base snapshot.
    pub dirty_paths_digest: Option<String>,
}

impl SourceSnapshotEvidence {
    /// Create snapshot evidence with explicit digest lineage and provider binding.
    pub fn new(
        root_digest: impl Into<String>,
        source_kind: SourceSnapshotKind,
        leaf_count: usize,
        provider_digest: impl Into<String>,
    ) -> Self {
        Self {
            schema_id: SOURCE_SNAPSHOT_SCHEMA_ID.to_owned(),
            algorithm: SOURCE_SNAPSHOT_ALGORITHM.to_owned(),
            root_digest: root_digest.into(),
            source_kind,
            leaf_count,
            base_root_digest: None,
            provider_digest: provider_digest.into(),
            dirty_paths_digest: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
/// Runtime authority that supplied a source resolution result.
pub enum ResolutionAuthority {
    /// A live parser evaluated the snapshot-bound owner directly.
    LiveParser,
    /// A content-addressed cache supplied an artifact bound to the snapshot digest.
    ContentCache,
    /// A derived semantic index supplied the resolution result.
    DerivedIndex,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
/// Observable outcome of resolving an owner or item against a bound snapshot.
pub enum ResolutionState {
    /// Live parsing found the requested owner or item.
    LiveHit,
    /// A digest-matched cached artifact satisfied the request.
    ArtifactCacheHit,
    /// The requested owner path is absent from the bound snapshot.
    OwnerNotInSnapshot,
    /// The selector path belongs to a different snapshot namespace.
    SelectorPathNamespaceMismatch,
    /// The owner parsed successfully but did not contain the requested item.
    ItemNotInLiveOwner,
    /// Live parsing failed before an authoritative item decision could be made.
    ParserFailed,
    /// No snapshot-compatible derived index was available.
    IndexUnavailable,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// Evidence connecting a resolution decision to snapshot and parser/index artifacts.
pub struct ResolutionEvidence {
    /// Schema contract for interpreting this resolution receipt.
    pub schema_id: String,
    /// Snapshot root against which the resolution decision was made.
    pub snapshot_root: String,
    /// Runtime authority that supplied the decision.
    pub authority: ResolutionAuthority,
    /// Observable resolution outcome.
    pub state: ResolutionState,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Normalized owner path addressed by the request, when one was resolved.
    pub owner_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Digest of the owner bytes used for live or cached resolution.
    pub owner_blob_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Digest of the parser artifact used to derive structural evidence.
    pub parser_artifact_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Digest of the derived index artifact used for cached resolution.
    pub index_artifact_digest: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// Resolution evidence paired with the exact source snapshot it was evaluated against.
pub struct SnapshotBoundResolution {
    /// Exact source snapshot that constrains the resolution namespace.
    pub source_snapshot: SourceSnapshotEvidence,
    /// Authority, state, owner, and artifact evidence for the decision.
    pub resolution_evidence: ResolutionEvidence,
}

impl SnapshotBoundResolution {
    /// Bind resolution evidence to a snapshot only when both carry the same root digest.
    pub fn new(
        source_snapshot: SourceSnapshotEvidence,
        resolution_evidence: ResolutionEvidence,
    ) -> Result<Self, String> {
        if source_snapshot.root_digest != resolution_evidence.snapshot_root {
            return Err(format!(
                "source snapshot root `{}` does not match resolution snapshot root `{}`",
                source_snapshot.root_digest, resolution_evidence.snapshot_root
            ));
        }

        Ok(Self {
            source_snapshot,
            resolution_evidence,
        })
    }
}

impl ResolutionEvidence {
    /// Create a snapshot-bound resolution receipt from authority, state, and artifact evidence.
    pub fn new(
        snapshot_root: impl Into<String>,
        authority: ResolutionAuthority,
        state: ResolutionState,
    ) -> Self {
        Self {
            schema_id: SOURCE_RESOLUTION_SCHEMA_ID.to_owned(),
            snapshot_root: snapshot_root.into(),
            authority,
            state,
            owner_path: None,
            owner_blob_digest: None,
            parser_artifact_digest: None,
            index_artifact_digest: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Workspace path-to-digest snapshot with deterministic root and overlay operations.
pub struct WorkspaceSnapshot {
    root_digest: String,
    leaves: BTreeMap<String, String>,
    base_root_digest: Option<String>,
    dirty_paths_digest: Option<String>,
}

impl WorkspaceSnapshot {
    /// Build a deterministic workspace snapshot from normalized path and file digests.
    pub fn from_file_hashes<I, P, H>(file_hashes: I) -> Self
    where
        I: IntoIterator<Item = (P, H)>,
        P: Into<String>,
        H: Into<String>,
    {
        let leaves = file_hashes
            .into_iter()
            .map(|(path, hash)| (normalize_snapshot_path(&path.into()), hash.into()))
            .collect::<BTreeMap<_, _>>();
        let root_digest = merkle_root(&leaves);
        Self {
            root_digest,
            leaves,
            base_root_digest: None,
            dirty_paths_digest: None,
        }
    }

    /// Borrow the digest that commits to the complete workspace snapshot.
    pub fn root_digest(&self) -> &str {
        &self.root_digest
    }

    /// Return the content digest bound to a workspace-relative source path.
    pub fn file_digest(&self, workspace_relative_path: &str) -> Option<&str> {
        self.leaves
            .get(&normalize_snapshot_path(workspace_relative_path))
            .map(String::as_str)
    }

    /// Report whether the workspace-relative path belongs to this snapshot.
    pub fn contains_path(&self, workspace_relative_path: &str) -> bool {
        self.file_digest(workspace_relative_path).is_some()
    }

    /// Materialize schema and digest evidence for this snapshot and provider binding.
    pub fn evidence(
        &self,
        source_kind: SourceSnapshotKind,
        provider_digest: impl Into<String>,
    ) -> SourceSnapshotEvidence {
        let mut evidence = SourceSnapshotEvidence::new(
            self.root_digest.clone(),
            source_kind,
            self.leaves.len(),
            provider_digest,
        );
        evidence.base_root_digest.clone_from(&self.base_root_digest);
        evidence
            .dirty_paths_digest
            .clone_from(&self.dirty_paths_digest);
        evidence
    }

    /// Bind a fully materialized current snapshot to the Merkle delta that
    /// produced it from an already published base root.
    pub fn overlay_evidence<I, P, D, Q>(
        &self,
        source_kind: SourceSnapshotKind,
        provider_digest: impl Into<String>,
        base_root_digest: impl Into<String>,
        changed_paths: I,
        removed_paths: D,
    ) -> Result<SourceSnapshotEvidence, String>
    where
        I: IntoIterator<Item = P>,
        P: Into<String>,
        D: IntoIterator<Item = Q>,
        Q: Into<String>,
    {
        let changed_leaves = changed_paths
            .into_iter()
            .map(Into::into)
            .map(|path| normalize_snapshot_path(&path))
            .map(|path| {
                self.leaves
                    .get(&path)
                    .cloned()
                    .map(|digest| (path.clone(), digest))
                    .ok_or_else(|| {
                        format!(
                            "Merkle overlay changed path is absent from current snapshot: {path}"
                        )
                    })
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        let removed_paths = removed_paths
            .into_iter()
            .map(Into::into)
            .map(|path| normalize_snapshot_path(&path))
            .collect::<BTreeSet<_>>();
        if let Some(path) = removed_paths
            .iter()
            .find(|path| self.leaves.contains_key(path.as_str()))
        {
            return Err(format!(
                "Merkle overlay removed path remains in current snapshot: {path}"
            ));
        }
        if changed_leaves.is_empty() && removed_paths.is_empty() {
            return Err("Merkle overlay evidence requires at least one changed path".to_string());
        }
        let mut evidence = SourceSnapshotEvidence::new(
            self.root_digest.clone(),
            source_kind,
            self.leaves.len(),
            provider_digest,
        );
        evidence.base_root_digest = Some(base_root_digest.into());
        evidence.dirty_paths_digest =
            Some(overlay_dirty_paths_digest(&changed_leaves, &removed_paths));
        Ok(evidence)
    }

    /// Derive a snapshot by applying explicit path/digest updates over this base root.
    pub fn with_overlay<I, P, H>(&self, file_hashes: I) -> Self
    where
        I: IntoIterator<Item = (P, H)>,
        P: Into<String>,
        H: Into<String>,
    {
        let overlay_leaves = file_hashes
            .into_iter()
            .map(|(path, hash)| (normalize_snapshot_path(&path.into()), hash.into()))
            .collect::<BTreeMap<_, _>>();
        let dirty_paths_digest = merkle_root(&overlay_leaves);
        let mut leaves = self.leaves.clone();
        leaves.extend(overlay_leaves);
        let root_digest = merkle_root(&leaves);

        Self {
            root_digest,
            leaves,
            base_root_digest: Some(self.root_digest.clone()),
            dirty_paths_digest: Some(dirty_paths_digest),
        }
    }

    /// Derive a snapshot from upserted blob digests and deleted paths.
    pub fn with_overlay_delta<I, P, H, D, Q>(&self, file_hashes: I, deleted_paths: D) -> Self
    where
        I: IntoIterator<Item = (P, H)>,
        P: Into<String>,
        H: Into<String>,
        D: IntoIterator<Item = Q>,
        Q: Into<String>,
    {
        let overlay_leaves = file_hashes
            .into_iter()
            .map(|(path, hash)| (normalize_snapshot_path(&path.into()), hash.into()))
            .collect::<BTreeMap<_, _>>();
        let deleted_paths = deleted_paths
            .into_iter()
            .map(Into::into)
            .map(|path| normalize_snapshot_path(&path))
            .collect::<BTreeSet<_>>();

        let dirty_paths_digest = overlay_dirty_paths_digest(&overlay_leaves, &deleted_paths);
        let mut leaves = self.leaves.clone();
        for path in deleted_paths {
            leaves.remove(&path);
        }
        leaves.extend(overlay_leaves);
        let root_digest = merkle_root(&leaves);

        Self {
            root_digest,
            leaves,
            base_root_digest: Some(self.root_digest.clone()),
            dirty_paths_digest: Some(dirty_paths_digest),
        }
    }

    /// Reconcile a complete live path/digest view against this Merkle snapshot.
    ///
    /// Callers with a trusted dirty-path set should prefer `with_overlay_delta`.
    /// This adapter exists for filesystem discovery paths that must derive the
    /// delta before provider or language candidate selection.
    pub fn reconcile<I, P, H>(&self, file_hashes: I) -> Self
    where
        I: IntoIterator<Item = (P, H)>,
        P: Into<String>,
        H: Into<String>,
    {
        let current_leaves = file_hashes
            .into_iter()
            .map(|(path, hash)| (normalize_snapshot_path(&path.into()), hash.into()))
            .collect::<BTreeMap<_, _>>();
        let changed_leaves = current_leaves
            .iter()
            .filter(|(path, hash)| self.leaves.get(*path) != Some(*hash))
            .map(|(path, hash)| (path.clone(), hash.clone()))
            .collect::<Vec<_>>();
        let deleted_paths = self
            .leaves
            .keys()
            .filter(|path| !current_leaves.contains_key(*path))
            .cloned()
            .collect::<Vec<_>>();
        self.with_overlay_delta(changed_leaves, deleted_paths)
    }
}

fn normalize_snapshot_path(path: &str) -> String {
    path.replace('\\', "/")
        .split('/')
        .filter(|component| !component.is_empty() && *component != ".")
        .fold(Vec::<&str>::new(), |mut components, component| {
            if component == ".." {
                components.pop();
            } else {
                components.push(component);
            }
            components
        })
        .join("/")
}

fn overlay_dirty_paths_digest(
    changed_leaves: &BTreeMap<String, String>,
    removed_paths: &BTreeSet<String>,
) -> String {
    let mut dirty_leaves = changed_leaves
        .iter()
        .map(|(path, digest)| {
            let operation_digest = crate::hash_blob(format!("upsert\0{digest}").as_bytes()).value;
            (path.clone(), operation_digest)
        })
        .collect::<BTreeMap<_, _>>();
    for path in removed_paths {
        dirty_leaves.insert(path.clone(), crate::hash_blob(b"delete").value);
    }
    merkle_root(&dirty_leaves)
}

fn merkle_root(leaves: &BTreeMap<String, String>) -> String {
    let children = leaves
        .iter()
        .enumerate()
        .map(|(ordinal, (path, digest))| crate::ArtifactChildRef {
            role: "source".to_owned(),
            name: path.clone(),
            child_hash: crate::hash_leaf(crate::ArtifactLeafInput {
                codec: "text",
                media_type: "application/vnd.asp.source-content-digest",
                payload: digest.as_bytes(),
            }),
            ordinal: u64::try_from(ordinal).expect("source snapshot leaf count exceeds u64"),
        })
        .collect();
    let root = crate::hash_node(&crate::ArtifactNodeInput {
        kind: crate::ArtifactKind::new("sourceSnapshot"),
        schema_id: SOURCE_SNAPSHOT_SCHEMA_ID.to_owned(),
        schema_version: "1".to_owned(),
        producer_hash: None,
        payload_hash: None,
        metadata_hash: None,
        children,
    });
    debug_assert_eq!(root.algorithm, crate::HASH_ALGORITHM_BLAKE3);
    root.value
}
