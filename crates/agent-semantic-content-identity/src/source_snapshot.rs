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

    /// Whether two receipts identify the same provider-bound source content.
    ///
    /// Snapshot kind and overlay lineage describe how the content was obtained;
    /// they do not change the identity of an equal canonical root.
    pub fn has_same_content_identity(&self, other: &Self) -> bool {
        self.schema_id == other.schema_id
            && self.algorithm == other.algorithm
            && self.root_digest == other.root_digest
            && self.leaf_count == other.leaf_count
            && self.provider_digest == other.provider_digest
    }

    /// Render the snapshot root in the shared integrity-reference domain.
    ///
    /// The snapshot producer owns this conversion because only it can bind the
    /// Merkle algorithm label to the root bytes. Runtime callers must not infer
    /// an algorithm from a bare digest string.
    pub fn root_integrity_reference(&self) -> Result<String, String> {
        if self.algorithm != SOURCE_SNAPSHOT_ALGORITHM {
            return Err(format!(
                "source snapshot algorithm is not admitted: {}",
                self.algorithm
            ));
        }
        if self.root_digest.len() != 64
            || !self
                .root_digest
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err("source snapshot root is not a canonical BLAKE3 digest".to_owned());
        }
        Ok(format!(
            "blake3-256:{}",
            self.root_digest.to_ascii_lowercase()
        ))
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

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
/// Workspace path-to-digest snapshot with deterministic root and overlay operations.
pub struct WorkspaceSnapshot {
    root_digest: String,
    leaves: BTreeMap<String, String>,
    base_root_digest: Option<String>,
    dirty_paths_digest: Option<String>,
    overlay_base_leaves: BTreeMap<String, Option<String>>,
}

impl WorkspaceSnapshot {
    /// Build the canonical workspace snapshot directly from source bytes.
    ///
    /// Callers that own source bytes must use this constructor instead of
    /// choosing a digest encoding independently from the Merkle contract.
    pub fn from_file_bytes<I, P, B>(files: I) -> Self
    where
        I: IntoIterator<Item = (P, B)>,
        P: Into<String>,
        B: AsRef<[u8]>,
    {
        Self::from_file_hashes(files.into_iter().map(|(path, bytes)| {
            (
                path,
                crate::exact_selector_merkle::blake3_content_digest_v1(bytes.as_ref())
                    .as_str()
                    .to_owned(),
            )
        }))
    }

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
            overlay_base_leaves: BTreeMap::new(),
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

    /// Validate the complete canonical-root plus accumulated-overlay evidence.
    pub fn validate(&self) -> Result<(), String> {
        if self.root_digest != merkle_root(&self.leaves) {
            return Err("workspace snapshot root digest does not match its leaves".to_owned());
        }
        if self
            .leaves
            .keys()
            .chain(self.overlay_base_leaves.keys())
            .any(|path| normalize_snapshot_path(path) != *path)
        {
            return Err("workspace snapshot contains a non-normalized path".to_owned());
        }
        if self.overlay_base_leaves.is_empty() {
            if self.base_root_digest.is_some() || self.dirty_paths_digest.is_some() {
                return Err(
                    "workspace snapshot without dirty paths cannot carry overlay evidence"
                        .to_owned(),
                );
            }
            return Ok(());
        }
        if self.base_root_digest.is_none() || self.dirty_paths_digest.is_none() {
            return Err("workspace snapshot dirty paths require base and dirty digests".to_owned());
        }
        if self
            .overlay_base_leaves
            .iter()
            .any(|(path, base)| self.leaves.get(path).map(String::as_str) == base.as_deref())
        {
            return Err("workspace snapshot overlay contains a reverted path".to_owned());
        }
        let changed_leaves = self
            .overlay_base_leaves
            .keys()
            .filter_map(|path| {
                self.leaves
                    .get(path)
                    .map(|digest| (path.clone(), digest.clone()))
            })
            .collect::<BTreeMap<_, _>>();
        let removed_paths = self
            .overlay_base_leaves
            .keys()
            .filter(|path| !self.leaves.contains_key(*path))
            .cloned()
            .collect::<BTreeSet<_>>();
        if self.dirty_paths_digest.as_deref()
            != Some(overlay_dirty_paths_digest(&changed_leaves, &removed_paths).as_str())
        {
            return Err(
                "workspace snapshot dirty-path digest does not match its overlay".to_owned(),
            );
        }
        Ok(())
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
        self.with_overlay_delta(file_hashes, std::iter::empty::<String>())
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

        let mut leaves = self.leaves.clone();
        let mut overlay_base_leaves = self.overlay_base_leaves.clone();
        for (path, digest) in overlay_leaves {
            if !overlay_base_leaves.contains_key(&path) {
                overlay_base_leaves.insert(path.clone(), self.leaves.get(&path).cloned());
            }
            leaves.insert(path.clone(), digest);
            if leaves.get(&path) == overlay_base_leaves.get(&path).and_then(Option::as_ref) {
                overlay_base_leaves.remove(&path);
            }
        }
        for path in deleted_paths {
            if !overlay_base_leaves.contains_key(&path) {
                overlay_base_leaves.insert(path.clone(), self.leaves.get(&path).cloned());
            }
            leaves.remove(&path);
            if overlay_base_leaves.get(&path).is_some_and(Option::is_none) {
                overlay_base_leaves.remove(&path);
            }
        }
        let root_digest = merkle_root(&leaves);
        let changed_leaves = overlay_base_leaves
            .keys()
            .filter_map(|path| {
                leaves
                    .get(path)
                    .map(|digest| (path.clone(), digest.clone()))
            })
            .collect::<BTreeMap<_, _>>();
        let removed_paths = overlay_base_leaves
            .keys()
            .filter(|path| !leaves.contains_key(*path))
            .cloned()
            .collect::<BTreeSet<_>>();
        let (base_root_digest, dirty_paths_digest) = if overlay_base_leaves.is_empty() {
            (None, None)
        } else {
            (
                self.base_root_digest
                    .clone()
                    .or_else(|| Some(self.root_digest.clone())),
                Some(overlay_dirty_paths_digest(&changed_leaves, &removed_paths)),
            )
        };

        Self {
            root_digest,
            leaves,
            base_root_digest,
            dirty_paths_digest,
            overlay_base_leaves,
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
    let file_digests = leaves.iter().map(|(path, digest)| {
        let digest =
            crate::exact_selector_merkle::parse_content_digest_v1(digest).unwrap_or_else(|_| {
                crate::exact_selector_merkle::blake3_content_digest_v1(digest.as_bytes())
            });
        (path.clone(), digest)
    });
    crate::workspace_merkle_v1::WorkspacePathMerkleTreeV1::from_file_digests(file_digests)
        .expect("workspace snapshot paths are normalized and unique")
        .root_digest()
        .as_str()
        .to_owned()
}
