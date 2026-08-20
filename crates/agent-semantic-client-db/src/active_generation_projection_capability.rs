//! Tokio-owned publication authority for active-generation selector projection capabilities.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use tokio::sync::watch;

/// Stable schema identifier for the projection-capability publication receipt.
pub const ACTIVE_GENERATION_PROJECTION_CAPABILITY_SCHEMA_ID: &str =
    "agent.semantic-protocols.active-generation-projection-capability";

/// Stable schema version for the projection-capability publication receipt.
pub const ACTIVE_GENERATION_PROJECTION_CAPABILITY_SCHEMA_VERSION: &str = "1";

/// Lifecycle state carried by a projection-capability receipt.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ActiveGenerationCapabilityState {
    Building,
    Ready,
    Failed,
}

/// Projection forms that an admitted selector can materialize.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ActiveGenerationProjectionMode {
    Source,
    CallableSkeleton,
}

/// Projection capabilities committed for one canonical selector.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveGenerationSelectorCapability {
    pub selector: String,
    pub owner_path: String,
    pub projection_modes: BTreeSet<ActiveGenerationProjectionMode>,
}

/// Immutable provider and selector capability input carried by canonical materialization.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveGenerationProjectionCapabilityManifest {
    pub provider_catalog_digest: String,
    pub selectors: Vec<ActiveGenerationSelectorCapability>,
}

impl ActiveGenerationProjectionCapabilityManifest {
    pub fn single_selector(
        provider_catalog_digest: String,
        selector: String,
        owner_path: String,
        projection_modes: BTreeSet<ActiveGenerationProjectionMode>,
    ) -> Result<Self, String> {
        let manifest = Self {
            provider_catalog_digest,
            selectors: vec![ActiveGenerationSelectorCapability {
                selector,
                owner_path,
                projection_modes,
            }],
        };
        manifest.validate()?;
        Ok(manifest)
    }

    /// Builds a manifest from parser-owned source-index selector projections.
    pub fn from_source_index(
        provider_catalog_digest: String,
        selectors: &[crate::ClientDbSourceIndexSelector],
    ) -> Result<Self, String> {
        let selectors = selectors
            .iter()
            .map(|selector| {
                let mut projection_modes = BTreeSet::from([ActiveGenerationProjectionMode::Source]);
                for projection in &selector.derived_projections {
                    projection_modes.insert(match projection.projection_kind {
                        crate::runtime_server_workspace::ExactProjectionKind::Source => {
                            ActiveGenerationProjectionMode::Source
                        }
                        crate::runtime_server_workspace::ExactProjectionKind::CallableSkeleton => {
                            ActiveGenerationProjectionMode::CallableSkeleton
                        }
                    });
                }
                ActiveGenerationSelectorCapability {
                    selector: selector.selector_id.as_str().to_owned(),
                    owner_path: selector.owner_path.as_str().to_owned(),
                    projection_modes,
                }
            })
            .collect();
        let manifest = Self {
            provider_catalog_digest,
            selectors,
        };
        manifest.validate()?;
        Ok(manifest)
    }

    /// Validates a materialization-time capability manifest before generation publication.
    pub fn validate(&self) -> Result<(), String> {
        validate_digest("providerCatalogDigest", &self.provider_catalog_digest)?;
        validate_selector_capabilities(&self.selectors)
    }

    /// Binds this immutable manifest to one generation publication epoch.
    pub fn into_ready_receipt(
        self,
        workspace_identity: String,
        generation_digest: String,
        root_digest: String,
        publication_epoch: u64,
    ) -> Result<ActiveGenerationProjectionCapabilityReceipt, String> {
        self.validate()?;
        let receipt = ActiveGenerationProjectionCapabilityReceipt {
            schema_id: ACTIVE_GENERATION_PROJECTION_CAPABILITY_SCHEMA_ID.to_owned(),
            schema_version: ACTIVE_GENERATION_PROJECTION_CAPABILITY_SCHEMA_VERSION.to_owned(),
            state: ActiveGenerationCapabilityState::Ready,
            workspace_identity,
            generation_digest,
            root_digest,
            provider_catalog_digest: self.provider_catalog_digest,
            provider_catalog_readable: true,
            publication_epoch,
            selectors: self.selectors,
            failure: None,
        };
        receipt.validate()?;
        Ok(receipt)
    }
}

/// Durable, generation-bound authority consumed by exact Search and Query reads.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveGenerationProjectionCapabilityReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub state: ActiveGenerationCapabilityState,
    pub workspace_identity: String,
    pub generation_digest: String,
    pub root_digest: String,
    pub provider_catalog_digest: String,
    pub provider_catalog_readable: bool,
    pub publication_epoch: u64,
    pub selectors: Vec<ActiveGenerationSelectorCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<ActiveGenerationProjectionCapabilityFailure>,
}

/// Typed failure attached to a failed capability publication.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveGenerationProjectionCapabilityFailure {
    pub reason_kind: ActiveGenerationProjectionCapabilityFailureKind,
    pub message: String,
}

/// Stable reason categories for capability publication failure.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ActiveGenerationProjectionCapabilityFailureKind {
    ProviderCatalogUnavailable,
    OwnerNotAdmitted,
    ProjectionCapabilityMissing,
    ProjectionSegmentUnavailable,
}

/// Single-writer Tokio publisher for capability epochs.
#[derive(Debug)]
pub struct ActiveGenerationProjectionCapabilityPublisher {
    sender: watch::Sender<Option<ActiveGenerationProjectionCapabilityReceipt>>,
}

/// Tokio watch reader for the latest committed capability epoch.
#[derive(Clone, Debug)]
pub struct ActiveGenerationProjectionCapabilityReader {
    receiver: watch::Receiver<Option<ActiveGenerationProjectionCapabilityReceipt>>,
}

/// Creates one Tokio-owned publisher with any number of independent readers.
pub fn active_generation_projection_capability_channel() -> (
    ActiveGenerationProjectionCapabilityPublisher,
    ActiveGenerationProjectionCapabilityReader,
) {
    let (sender, receiver) = watch::channel(None);
    (
        ActiveGenerationProjectionCapabilityPublisher { sender },
        ActiveGenerationProjectionCapabilityReader { receiver },
    )
}

impl ActiveGenerationProjectionCapabilityPublisher {
    /// Validates and atomically publishes a strictly newer capability epoch.
    pub fn publish(
        &self,
        receipt: ActiveGenerationProjectionCapabilityReceipt,
    ) -> Result<(), String> {
        receipt.validate()?;
        let current_epoch = self
            .sender
            .borrow()
            .as_ref()
            .map(|current| current.publication_epoch);
        if current_epoch.is_some_and(|epoch| receipt.publication_epoch <= epoch) {
            return Err(
                "active generation projection capability publication epoch is not monotonic"
                    .to_owned(),
            );
        }
        self.sender.send_replace(Some(receipt));
        Ok(())
    }

    /// Creates a reader attached to the current publication epoch.
    pub fn subscribe(&self) -> ActiveGenerationProjectionCapabilityReader {
        ActiveGenerationProjectionCapabilityReader {
            receiver: self.sender.subscribe(),
        }
    }
}

impl ActiveGenerationProjectionCapabilityReader {
    /// Returns the current immutable receipt without waiting or performing I/O.
    pub fn current(&self) -> Option<ActiveGenerationProjectionCapabilityReceipt> {
        self.receiver.borrow().clone()
    }

    /// Awaits the next published receipt and fails when the writer lifecycle closes.
    pub async fn changed(&mut self) -> Result<ActiveGenerationProjectionCapabilityReceipt, String> {
        loop {
            self.receiver.changed().await.map_err(|_| {
                "active generation projection capability publisher closed".to_owned()
            })?;
            if let Some(receipt) = self.receiver.borrow_and_update().clone() {
                return Ok(receipt);
            }
        }
    }
}

#[cfg(test)]
pub(crate) fn test_projection_capability_manifest() -> ActiveGenerationProjectionCapabilityManifest
{
    ActiveGenerationProjectionCapabilityManifest::single_selector(
        "blake3-256:0000000000000000000000000000000000000000000000000000000000000000".to_owned(),
        "rust://fixture/src/lib.rs#item/function/fixture".to_owned(),
        "src/lib.rs".to_owned(),
        std::collections::BTreeSet::from([ActiveGenerationProjectionMode::Source]),
    )
    .expect("test projection capability manifest")
}

impl ActiveGenerationProjectionCapabilityReceipt {
    pub fn manifest(&self) -> ActiveGenerationProjectionCapabilityManifest {
        ActiveGenerationProjectionCapabilityManifest {
            provider_catalog_digest: self.provider_catalog_digest.clone(),
            selectors: self.selectors.clone(),
        }
    }

    /// Validates schema identity and generation publication invariants.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != ACTIVE_GENERATION_PROJECTION_CAPABILITY_SCHEMA_ID {
            return Err("active generation projection capability schemaId drift".to_owned());
        }
        if self.schema_version != ACTIVE_GENERATION_PROJECTION_CAPABILITY_SCHEMA_VERSION {
            return Err("active generation projection capability schemaVersion drift".to_owned());
        }
        if self.workspace_identity.trim().is_empty() {
            return Err(
                "active generation projection capability workspaceIdentity is empty".to_owned(),
            );
        }
        if self.publication_epoch == 0 {
            return Err(
                "active generation projection capability publicationEpoch is zero".to_owned(),
            );
        }
        validate_digest("generationDigest", &self.generation_digest)?;
        validate_digest("rootDigest", &self.root_digest)?;
        validate_digest("providerCatalogDigest", &self.provider_catalog_digest)?;

        match self.state {
            ActiveGenerationCapabilityState::Ready => self.validate_ready(),
            ActiveGenerationCapabilityState::Failed if self.failure.is_none() => {
                Err("failed active generation projection capability lacks failure".to_owned())
            }
            ActiveGenerationCapabilityState::Building | ActiveGenerationCapabilityState::Failed => {
                Ok(())
            }
        }
    }

    /// Returns whether this ready receipt admits the requested selector projection.
    pub fn admits(&self, selector: &str, mode: ActiveGenerationProjectionMode) -> bool {
        self.state == ActiveGenerationCapabilityState::Ready
            && self.selectors.iter().any(|capability| {
                capability.selector == selector && capability.projection_modes.contains(&mode)
            })
    }

    fn validate_ready(&self) -> Result<(), String> {
        if !self.provider_catalog_readable {
            return Err("ready active generation has an unreadable provider catalog".to_owned());
        }
        if self.failure.is_some() {
            return Err("ready active generation carries a failure".to_owned());
        }
        validate_selector_capabilities(&self.selectors)
    }
}

fn validate_selector_capabilities(
    capabilities: &[ActiveGenerationSelectorCapability],
) -> Result<(), String> {
    if capabilities.is_empty() {
        // An empty selector set is an explicit capability boundary for a
        // Ready generation with no exact projections. Individual exact
        // selector requests still fail closed through `admits`.
    }
    let mut selectors = BTreeSet::new();
    for capability in capabilities {
        if capability.selector.trim().is_empty() || capability.owner_path.trim().is_empty() {
            return Err("active generation selector capability has an empty identity".to_owned());
        }
        if capability.projection_modes.is_empty() {
            return Err(format!(
                "active generation selector capability has no projection mode: {}",
                capability.selector
            ));
        }
        if !selectors.insert(capability.selector.as_str()) {
            return Err(format!(
                "active generation selector capability is duplicated: {}",
                capability.selector
            ));
        }
    }
    Ok(())
}

fn validate_digest(label: &str, digest: &str) -> Result<(), String> {
    let digest = digest
        .strip_prefix("blake3-256:")
        .or_else(|| digest.strip_prefix("sha256:"))
        .unwrap_or(digest);
    if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!(
            "active generation projection capability {label} is not a v1 digest"
        ));
    }
    Ok(())
}
