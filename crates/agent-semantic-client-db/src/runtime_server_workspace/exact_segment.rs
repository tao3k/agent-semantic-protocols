use memmap2::{Mmap, MmapOptions};
use std::cmp::Ordering;
use std::path::{Path, PathBuf};

use super::{
    WorkspaceGenerationSnapshot, WorkspaceOwnerSnapshot, WorkspaceRuntimeSelectorRead,
    WorkspaceSelectorSnapshot,
};
#[path = "exact_segment_client.rs"]
mod client;
#[path = "exact_segment_encoding.rs"]
mod encoding;
#[path = "exact_segment_evidence_context.rs"]
mod evidence_context;
#[path = "exact_segment_format.rs"]
mod format;
#[path = "exact_segment_owner_search.rs"]
mod owner_search;
pub use client::{WorkspaceExactProjectionDataPlaneClient, WorkspaceExactProjectionDataPlaneOpen};
use format::{
    checked_entry_offset, decode_header, read_slice, read_text, read_usize, write_range_entry,
    write_range_header, write_u64, write_usize,
};

// This is an internal mmap layout identity, not a protocol schema version.
// Layout 0005 commits parser-owned selector query keys and owner-local syntax
// diagnostics. Old segments fail
// closed so the publisher rebuilds them through the sole CompleteGeneration
// authority; there is deliberately no legacy decoder.
const MAGIC: &[u8; 16] = b"ASPEXACTMMAP0005";
const HEADER_LEN: usize = 160;
const OWNER_ENTRY_LEN: usize = 160;
const SELECTOR_ENTRY_LEN: usize = 120;
const RELOCATION_ENTRY_LEN: usize = 40;

const EPOCH_OFFSET: usize = 16;
const OWNER_COUNT_OFFSET: usize = 24;
const SELECTOR_COUNT_OFFSET: usize = 32;
const OWNER_TABLE_OFFSET: usize = 40;
const SELECTOR_TABLE_OFFSET: usize = 48;
const STRING_TABLE_OFFSET: usize = 56;
const BLOB_OFFSET: usize = 64;
const TOTAL_LEN_OFFSET: usize = 72;
const GENERATION_DIGEST_OFFSET: usize = 80;
const GENERATION_DIGEST_LEN_OFFSET: usize = 88;
const ROOT_DIGEST_OFFSET: usize = 96;
const ROOT_DIGEST_LEN_OFFSET: usize = 104;
const WORKSPACE_ID_OFFSET: usize = 112;
const WORKSPACE_ID_LEN_OFFSET: usize = 120;
const RELOCATION_TABLE_OFFSET: usize = 128;
const RELOCATION_COUNT_OFFSET: usize = 136;
const CONTEXT_TABLE_OFFSET: usize = 144;
const CONTEXT_COUNT_OFFSET: usize = 152;

#[derive(Debug)]
struct MappedWorkspaceExactProjection {
    mapping: Mmap,
    epoch: u64,
    owner_count: usize,
    selector_count: usize,
    selector_indices_by_owner: Vec<Vec<usize>>,
    owner_table_offset: usize,
    selector_table_offset: usize,
    relocation_table_offset: usize,
    relocation_count: usize,
    context_table_offset: usize,
    context_count: usize,
    generation_digest: String,
    root_digest: String,
}

impl MappedWorkspaceExactProjection {
    async fn open(snapshot: &WorkspaceGenerationSnapshot) -> Result<Self, String> {
        let path = exact_projection_segment_path(Path::new(&snapshot.mmap_segment_path));
        let file = tokio::fs::File::open(&path)
            .await
            .map_err(|error| format!("open workspace exact projection segment: {error}"))?
            .into_std()
            .await;
        let mapping = unsafe {
            // SAFETY: exact projection segments are immutable after atomic
            // publication and this value owns the mapping lifetime.
            MmapOptions::new()
                .map(&file)
                .map_err(|error| format!("map workspace exact projection segment: {error}"))?
        };
        Self::from_mapping(
            mapping,
            snapshot.active_epoch,
            &snapshot.workspace_identity,
            &snapshot.generation_digest,
        )
    }

    fn from_mapping(
        mapping: Mmap,
        expected_epoch: u64,
        expected_workspace_identity: &str,
        expected_generation_digest: &str,
    ) -> Result<Self, String> {
        let header = decode_header(&mapping)?;
        if header.epoch != expected_epoch {
            return Err("workspace exact projection epoch does not match pointer".to_owned());
        }
        let workspace_identity = read_text(
            &mapping,
            header.workspace_id_offset,
            header.workspace_id_len,
            "workspace identity",
        )?;
        if workspace_identity != expected_workspace_identity {
            return Err("workspace exact projection identity does not match pointer".to_owned());
        }
        let generation_digest = read_text(
            &mapping,
            header.generation_digest_offset,
            header.generation_digest_len,
            "generation digest",
        )?
        .to_owned();
        if generation_digest != expected_generation_digest {
            return Err("workspace exact projection generation does not match pointer".to_owned());
        }
        let root_digest = read_text(
            &mapping,
            header.root_digest_offset,
            header.root_digest_len,
            "root digest",
        )?
        .to_owned();
        let mut mapped = Self {
            mapping,
            epoch: header.epoch,
            owner_count: header.owner_count,
            selector_count: header.selector_count,
            selector_indices_by_owner: vec![Vec::new(); header.owner_count],
            owner_table_offset: header.owner_table_offset,
            selector_table_offset: header.selector_table_offset,
            relocation_table_offset: header.relocation_table_offset,
            relocation_count: header.relocation_count,
            context_table_offset: header.context_table_offset,
            context_count: header.context_count,
            generation_digest,
            root_digest,
        };
        for selector_index in 0..mapped.selector_count {
            let owner_index = mapped.selector_entry(selector_index)?.owner_index;
            mapped
                .selector_indices_by_owner
                .get_mut(owner_index)
                .ok_or_else(|| "workspace exact selector owner index is out of range".to_owned())?
                .push(selector_index);
        }
        Ok(mapped)
    }

    fn read_runtime_selector(
        &self,
        projection_kind: super::model::ExactProjectionKind,
        structural_selector: &str,
    ) -> Result<WorkspaceRuntimeSelectorRead, String> {
        if let Some(selector) = self.find_selector(projection_kind, structural_selector)? {
            return self.project_selector(projection_kind, selector);
        }
        let canonical_selector =
            agent_semantic_content_identity::CanonicalItemSelector::parse_root_or_exact_descendant(
                structural_selector,
            );
        if canonical_selector.is_ok() {
            let relocated = self.find_relocated_selectors(structural_selector)?;
            if relocated.len() == 1 {
                let resolved_selector = &relocated[0];
                if let Some(selector) = self.find_selector(projection_kind, resolved_selector)? {
                    return self.project_selector(projection_kind, selector);
                }
                return Ok(WorkspaceRuntimeSelectorRead::ProjectionMissing {
                    generation_digest: self.generation_digest.clone(),
                    root_digest: self.root_digest.clone(),
                    resolved_selector: resolved_selector.clone(),
                });
            }
            if relocated.len() > 1 {
                return Ok(WorkspaceRuntimeSelectorRead::RelocationAmbiguous {
                    generation_digest: self.generation_digest.clone(),
                    root_digest: self.root_digest.clone(),
                    candidates: relocated,
                });
            }
        }
        let (owner_path, owner_reference) = match canonical_selector {
            Ok(selector) => (selector.owner_path()?, false),
            Err(canonical_error) => {
                let owner_path =
                    agent_semantic_client_protocol::workspace_source_mutation::SourceOwnerPath::new(
                        structural_selector.to_owned(),
                    )
                .map_err(|owner_error| {
                    format!(
                        "exact selector reference is neither canonical nor a normalized owner path: canonical={canonical_error}; owner={owner_error}"
                    )
                })?;
                (owner_path.as_str().to_owned(), true)
            }
        };
        let Some((owner_index, owner)) = self.find_owner(&owner_path)? else {
            return Ok(WorkspaceRuntimeSelectorRead::OwnerMissing {
                generation_digest: self.generation_digest.clone(),
                root_digest: self.root_digest.clone(),
            });
        };
        if owner_reference {
            if projection_kind == super::model::ExactProjectionKind::Source {
                return Ok(WorkspaceRuntimeSelectorRead::Projection {
                    generation_digest: self.generation_digest.clone(),
                    root_digest: self.root_digest.clone(),
                    resolved_selector: owner_path,
                    bytes: self.owner_bytes(&owner)?.to_vec(),
                });
            }
            return Ok(WorkspaceRuntimeSelectorRead::ProjectionMissing {
                generation_digest: self.generation_digest.clone(),
                root_digest: self.root_digest.clone(),
                resolved_selector: owner_path,
            });
        }
        Ok(WorkspaceRuntimeSelectorRead::OwnerForRepair {
            generation_digest: self.generation_digest.clone(),
            root_digest: self.root_digest.clone(),
            owner: self.owner_snapshot(owner_index, &owner)?,
        })
    }

    fn project_selector(
        &self,
        projection_kind: super::model::ExactProjectionKind,
        selector: SelectorEntry,
    ) -> Result<WorkspaceRuntimeSelectorRead, String> {
        let resolved_selector = self.selector_text(&selector)?.to_owned();
        let owner = self.owner_entry(selector.owner_index)?;
        let projection = if projection_kind == super::model::ExactProjectionKind::Source {
            self.owner_bytes(&owner)?
                .get(selector.byte_start..selector.byte_end)
                .ok_or_else(|| "workspace exact projection selector range is invalid".to_owned())?
                .to_vec()
        } else {
            read_slice(
                &self.mapping,
                selector.projection_blob_offset,
                selector.projection_blob_len,
                "derived projection bytes",
            )?
            .to_vec()
        };
        Ok(WorkspaceRuntimeSelectorRead::Projection {
            generation_digest: self.generation_digest.clone(),
            root_digest: self.root_digest.clone(),
            resolved_selector,
            bytes: projection,
        })
    }

    fn owner_content_digest(&self, owner_path: &str) -> Result<Option<String>, String> {
        let Some((_, owner)) = self.find_owner(owner_path)? else {
            return Ok(None);
        };
        Ok(Some(self.owner_digest(&owner)?.to_owned()))
    }

    fn contains_owner(&self, expected: &WorkspaceOwnerSnapshot) -> Result<bool, String> {
        let Some((_, owner)) = self.find_owner(&expected.owner_path)? else {
            return Ok(false);
        };
        let expected_content_digest =
            format!("blake3-256:{}", blake3::hash(&expected.bytes).to_hex());
        if expected_content_digest != expected.content_digest
            || self.owner_digest(&owner)? != expected.content_digest
            || owner.projection_digest != owner_projection_digest(expected)
        {
            return Ok(false);
        }
        Ok(true)
    }

    fn owner_snapshot(
        &self,
        owner_index: usize,
        owner: &OwnerEntry,
    ) -> Result<WorkspaceOwnerSnapshot, String> {
        Ok(WorkspaceOwnerSnapshot {
            owner_path: self.owner_path(owner)?.to_owned(),
            authority: self.owner_authority(owner)?,
            content_digest: self.owner_digest(owner)?.to_owned(),
            native_syntax_diagnostic: self.owner_native_syntax_diagnostic(owner)?,
            bytes: self.owner_bytes(owner)?.to_vec(),
            selectors: self.owner_selectors(owner_index)?,
        })
    }

    fn owner_native_syntax_diagnostic(
        &self,
        owner: &OwnerEntry,
    ) -> Result<Option<agent_semantic_search::NativeSyntaxDiagnostic>, String> {
        if owner.diagnostic_len == 0 {
            return Ok(None);
        }
        serde_json::from_slice(read_slice(
            &self.mapping,
            owner.diagnostic_offset,
            owner.diagnostic_len,
            "owner native syntax diagnostic",
        )?)
        .map(Some)
        .map_err(|error| format!("decode owner native syntax diagnostic: {error}"))
    }

    fn owner_selectors(
        &self,
        owner_index: usize,
    ) -> Result<Vec<WorkspaceSelectorSnapshot>, String> {
        let owner_selector_indices = self
            .selector_indices_by_owner
            .get(owner_index)
            .ok_or_else(|| "workspace exact projection owner index is out of range".to_owned())?;
        let mut selector_groups: std::collections::BTreeMap<
            String,
            (
                Option<(usize, usize)>,
                Option<Vec<String>>,
                Vec<super::WorkspaceDerivedProjectionSnapshot>,
            ),
        > = std::collections::BTreeMap::new();
        for &index in owner_selector_indices {
            let entry = self.selector_entry(index)?;
            let selector_text = self.selector_text(&entry)?.to_owned();
            let projection_kind = self.selector_kind(&entry)?;
            let query_keys = self.selector_query_keys(&entry)?;
            let group = selector_groups.entry(selector_text.clone()).or_default();
            if let Some(existing) = &group.1 {
                if existing != &query_keys {
                    return Err(format!(
                        "workspace exact projection contains conflicting query keys for `{selector_text}`"
                    ));
                }
            } else {
                group.1 = Some(query_keys);
            }
            if projection_kind == super::model::ExactProjectionKind::Source {
                if group
                    .0
                    .replace((entry.byte_start, entry.byte_end))
                    .is_some()
                {
                    return Err(format!(
                        "workspace exact projection contains duplicate source selector `{selector_text}`"
                    ));
                }
            } else {
                group.2.push(super::WorkspaceDerivedProjectionSnapshot {
                    projection_kind,
                    bytes: read_slice(
                        &self.mapping,
                        entry.projection_blob_offset,
                        entry.projection_blob_len,
                        "derived projection bytes",
                    )?
                    .to_vec(),
                    evidence_context: self.evidence_context_for_projection(&entry)?,
                });
            }
        }

        let mut selectors = Vec::with_capacity(selector_groups.len());
        for (selector, (source_range, query_keys, mut derived_projections)) in selector_groups {
            let Some((byte_start, byte_end)) = source_range else {
                continue;
            };
            derived_projections
                .sort_by(|left, right| left.projection_kind.cmp(&right.projection_kind));
            selectors.push(WorkspaceSelectorSnapshot {
                selector,
                byte_start,
                byte_end,
                query_keys: query_keys.unwrap_or_default(),
                derived_projections,
            });
        }
        Ok(selectors)
    }

    fn find_owner(&self, owner_path: &str) -> Result<Option<(usize, OwnerEntry)>, String> {
        let hash = *blake3::hash(owner_path.as_bytes()).as_bytes();
        let Some(first) = self.owner_hash_start(&hash)? else {
            return Ok(None);
        };
        self.find_owner_from(first, &hash, owner_path)
    }

    fn owner_hash_start(&self, hash: &[u8; 32]) -> Result<Option<usize>, String> {
        let mut low = 0;
        let mut high = self.owner_count;
        while low < high {
            let middle = low + (high - low) / 2;
            let entry = self.owner_entry(middle)?;
            match entry.hash.cmp(hash) {
                Ordering::Less => low = middle + 1,
                Ordering::Greater => high = middle,
                Ordering::Equal => {
                    let mut first = middle;
                    while first > 0 && self.owner_entry(first - 1)?.hash == *hash {
                        first -= 1;
                    }
                    return Ok(Some(first));
                }
            }
        }
        Ok(None)
    }

    fn find_owner_from(
        &self,
        first: usize,
        hash: &[u8; 32],
        owner_path: &str,
    ) -> Result<Option<(usize, OwnerEntry)>, String> {
        let mut index = first;
        while index < self.owner_count {
            let candidate = self.owner_entry(index)?;
            if candidate.hash != *hash {
                break;
            }
            if self.owner_path(&candidate)? == owner_path {
                return Ok(Some((index, candidate)));
            }
            index += 1;
        }
        Ok(None)
    }

    fn find_selector(
        &self,
        projection_kind: super::model::ExactProjectionKind,
        selector: &str,
    ) -> Result<Option<SelectorEntry>, String> {
        let hash = projection_key_hash(projection_kind.as_str(), selector);
        let Some(first) = self.selector_hash_start(&hash)? else {
            return Ok(None);
        };
        self.find_selector_from(first, &hash, projection_kind, selector)
    }

    fn selector_hash_start(&self, hash: &[u8; 32]) -> Result<Option<usize>, String> {
        let mut low = 0;
        let mut high = self.selector_count;
        while low < high {
            let middle = low + (high - low) / 2;
            let entry = self.selector_entry(middle)?;
            match entry.hash.cmp(hash) {
                Ordering::Less => low = middle + 1,
                Ordering::Greater => high = middle,
                Ordering::Equal => {
                    let mut first = middle;
                    while first > 0 && self.selector_entry(first - 1)?.hash == *hash {
                        first -= 1;
                    }
                    return Ok(Some(first));
                }
            }
        }
        Ok(None)
    }

    fn find_selector_from(
        &self,
        first: usize,
        hash: &[u8; 32],
        projection_kind: super::model::ExactProjectionKind,
        selector: &str,
    ) -> Result<Option<SelectorEntry>, String> {
        let mut index = first;
        while index < self.selector_count {
            let candidate = self.selector_entry(index)?;
            if candidate.hash != *hash {
                break;
            }
            if self.selector_kind(&candidate)? == projection_kind
                && self.selector_text(&candidate)? == selector
            {
                return Ok(Some(candidate));
            }
            index += 1;
        }
        Ok(None)
    }

    fn find_relocated_selectors(&self, selector: &str) -> Result<Vec<String>, String> {
        let requested_identity = relocation_identity(selector)?;
        let hash = relocation_key_hash(requested_identity.as_str());
        let Some(first) = self.relocation_hash_start(&hash)? else {
            return Ok(Vec::new());
        };
        self.collect_relocated_selectors(first, &hash, &requested_identity)
    }

    fn relocation_hash_start(&self, hash: &[u8; 32]) -> Result<Option<usize>, String> {
        let mut low = 0;
        let mut high = self.relocation_count;
        while low < high {
            let middle = low + (high - low) / 2;
            match self.relocation_entry(middle)?.hash.cmp(hash) {
                Ordering::Less => low = middle + 1,
                Ordering::Greater => high = middle,
                Ordering::Equal => {
                    let mut first = middle;
                    while first > 0 && self.relocation_entry(first - 1)?.hash == *hash {
                        first -= 1;
                    }
                    return Ok(Some(first));
                }
            }
        }
        Ok(None)
    }

    fn collect_relocated_selectors(
        &self,
        first: usize,
        hash: &[u8; 32],
        requested_identity: &str,
    ) -> Result<Vec<String>, String> {
        let mut matches = Vec::new();
        let mut index = first;
        while index < self.relocation_count {
            let relocation = self.relocation_entry(index)?;
            if relocation.hash != *hash {
                break;
            }
            let candidate = self.selector_entry(relocation.selector_index)?;
            let candidate_text = self.selector_text(&candidate)?;
            if relocation_identity(candidate_text)? == requested_identity {
                matches.push(candidate_text.to_owned());
            }
            index += 1;
        }
        matches.sort();
        matches.dedup();
        Ok(matches)
    }

    fn relocation_entry(&self, index: usize) -> Result<RelocationEntry, String> {
        if index >= self.relocation_count {
            return Err("workspace exact relocation index is out of range".to_owned());
        }
        let start = checked_entry_offset(
            self.relocation_table_offset,
            index,
            RELOCATION_ENTRY_LEN,
            self.mapping.len(),
        )?;
        let bytes = &self.mapping[start..start + RELOCATION_ENTRY_LEN];
        Ok(RelocationEntry {
            hash: bytes[0..32]
                .try_into()
                .map_err(|_| "workspace exact relocation hash is invalid".to_owned())?,
            selector_index: read_usize(bytes, 32, "relocation selector index")?,
        })
    }

    fn owner_entry(&self, index: usize) -> Result<OwnerEntry, String> {
        if index >= self.owner_count {
            return Err("workspace exact projection owner index is out of range".to_owned());
        }
        let start = checked_entry_offset(
            self.owner_table_offset,
            index,
            OWNER_ENTRY_LEN,
            self.mapping.len(),
        )?;
        let bytes = &self.mapping[start..start + OWNER_ENTRY_LEN];
        Ok(OwnerEntry {
            hash: bytes[0..32]
                .try_into()
                .map_err(|_| "workspace exact owner hash is invalid".to_owned())?,
            path_offset: read_usize(bytes, 32, "owner path offset")?,
            path_len: read_usize(bytes, 40, "owner path length")?,
            digest_offset: read_usize(bytes, 48, "owner digest offset")?,
            digest_len: read_usize(bytes, 56, "owner digest length")?,
            blob_offset: read_usize(bytes, 64, "owner blob offset")?,
            blob_len: read_usize(bytes, 72, "owner blob length")?,
            language_offset: read_usize(bytes, 80, "owner language offset")?,
            language_len: read_usize(bytes, 88, "owner language length")?,
            provider_offset: read_usize(bytes, 96, "owner provider offset")?,
            provider_len: read_usize(bytes, 104, "owner provider length")?,
            diagnostic_offset: read_usize(bytes, 112, "owner diagnostic offset")?,
            diagnostic_len: read_usize(bytes, 120, "owner diagnostic length")?,
            projection_digest: bytes[128..160]
                .try_into()
                .map_err(|_| "workspace exact owner projection digest is invalid".to_owned())?,
        })
    }

    fn selector_entry(&self, index: usize) -> Result<SelectorEntry, String> {
        if index >= self.selector_count {
            return Err("workspace exact projection selector index is out of range".to_owned());
        }
        let start = checked_entry_offset(
            self.selector_table_offset,
            index,
            SELECTOR_ENTRY_LEN,
            self.mapping.len(),
        )?;
        let bytes = &self.mapping[start..start + SELECTOR_ENTRY_LEN];
        Ok(SelectorEntry {
            hash: bytes[0..32]
                .try_into()
                .map_err(|_| "workspace exact selector hash is invalid".to_owned())?,
            selector_offset: read_usize(bytes, 32, "selector text offset")?,
            selector_len: read_usize(bytes, 40, "selector text length")?,
            projection_kind_offset: read_usize(bytes, 48, "projection kind offset")?,
            projection_kind_len: read_usize(bytes, 56, "projection kind length")?,
            owner_index: read_usize(bytes, 64, "selector owner index")?,
            byte_start: read_usize(bytes, 72, "selector byte start")?,
            byte_end: read_usize(bytes, 80, "selector byte end")?,
            projection_blob_offset: read_usize(bytes, 88, "projection blob offset")?,
            projection_blob_len: read_usize(bytes, 96, "projection blob length")?,
            query_keys_offset: read_usize(bytes, 104, "selector query keys offset")?,
            query_keys_len: read_usize(bytes, 112, "selector query keys length")?,
        })
    }

    fn owner_path<'a>(&'a self, owner: &OwnerEntry) -> Result<&'a str, String> {
        read_text(
            &self.mapping,
            owner.path_offset,
            owner.path_len,
            "owner path",
        )
    }

    fn owner_digest<'a>(&'a self, owner: &OwnerEntry) -> Result<&'a str, String> {
        read_text(
            &self.mapping,
            owner.digest_offset,
            owner.digest_len,
            "owner digest",
        )
    }

    fn owner_bytes<'a>(&'a self, owner: &OwnerEntry) -> Result<&'a [u8], String> {
        read_slice(
            &self.mapping,
            owner.blob_offset,
            owner.blob_len,
            "owner bytes",
        )
    }

    fn owner_authority(
        &self,
        owner: &OwnerEntry,
    ) -> Result<Option<agent_semantic_search::ResidentSearchAuthority>, String> {
        if owner.language_len == 0 && owner.provider_len == 0 {
            return Ok(None);
        }
        if owner.language_len == 0 || owner.provider_len == 0 {
            return Err("workspace exact owner authority is incomplete".to_owned());
        }
        Ok(Some(agent_semantic_search::ResidentSearchAuthority {
            language_id: read_text(
                &self.mapping,
                owner.language_offset,
                owner.language_len,
                "owner language",
            )?
            .to_owned()
            .into(),
            provider_id: read_text(
                &self.mapping,
                owner.provider_offset,
                owner.provider_len,
                "owner provider",
            )?
            .to_owned()
            .into(),
        }))
    }

    fn selector_text<'a>(&'a self, selector: &SelectorEntry) -> Result<&'a str, String> {
        read_text(
            &self.mapping,
            selector.selector_offset,
            selector.selector_len,
            "selector text",
        )
    }

    fn selector_kind(
        &self,
        selector: &SelectorEntry,
    ) -> Result<super::model::ExactProjectionKind, String> {
        super::model::ExactProjectionKind::try_from(read_text(
            &self.mapping,
            selector.projection_kind_offset,
            selector.projection_kind_len,
            "projection kind",
        )?)
    }
}

#[derive(Clone, Copy)]
struct OwnerEntry {
    hash: [u8; 32],
    path_offset: usize,
    path_len: usize,
    digest_offset: usize,
    digest_len: usize,
    blob_offset: usize,
    blob_len: usize,
    language_offset: usize,
    language_len: usize,
    provider_offset: usize,
    provider_len: usize,
    diagnostic_offset: usize,
    diagnostic_len: usize,
    projection_digest: [u8; 32],
}

#[derive(Clone, Copy)]
struct SelectorEntry {
    hash: [u8; 32],
    selector_offset: usize,
    selector_len: usize,
    projection_kind_offset: usize,
    projection_kind_len: usize,
    owner_index: usize,
    byte_start: usize,
    byte_end: usize,
    projection_blob_offset: usize,
    projection_blob_len: usize,
    query_keys_offset: usize,
    query_keys_len: usize,
}

#[derive(Clone, Copy)]
struct RelocationEntry {
    hash: [u8; 32],
    selector_index: usize,
}

struct Header {
    epoch: u64,
    owner_count: usize,
    selector_count: usize,
    owner_table_offset: usize,
    selector_table_offset: usize,
    relocation_table_offset: usize,
    relocation_count: usize,
    context_table_offset: usize,
    context_count: usize,
    string_table_offset: usize,
    blob_offset: usize,
    generation_digest_offset: usize,
    generation_digest_len: usize,
    root_digest_offset: usize,
    root_digest_len: usize,
    workspace_id_offset: usize,
    workspace_id_len: usize,
}

pub(super) use encoding::encode_exact_projection_segment;

pub(super) fn exact_projection_segment_path(generation_path: &Path) -> PathBuf {
    generation_path.with_extension("exact.mmap")
}

fn push_bytes(target: &mut Vec<u8>, bytes: &[u8]) -> (usize, usize) {
    let offset = target.len();
    target.extend_from_slice(bytes);
    (offset, bytes.len())
}

fn projection_key_hash(projection_kind: &str, selector: &str) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(projection_kind.as_bytes());
    hasher.update(&[0]);
    hasher.update(selector.as_bytes());
    *hasher.finalize().as_bytes()
}

fn relocation_identity(selector: &str) -> Result<String, String> {
    let canonical =
        agent_semantic_content_identity::CanonicalItemSelector::parse_root_or_exact_descendant(
            selector,
        )?;
    Ok(format!(
        "{}#{}",
        canonical.language_id.as_str(),
        agent_semantic_content_identity::structural_selector::encode_canonical_item_identity_path(
            &canonical.identity()
        )
    ))
}

fn relocation_key_hash(identity: &str) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(identity.as_bytes());
    *hasher.finalize().as_bytes()
}

fn owner_projection_digest(owner: &WorkspaceOwnerSnapshot) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    if let Some(authority) = &owner.authority {
        hasher.update(authority.language_id.as_str().as_bytes());
        hasher.update(&[0]);
        hasher.update(authority.provider_id.as_str().as_bytes());
        hasher.update(&[0]);
    }
    if let Some(diagnostic) = &owner.native_syntax_diagnostic {
        hasher.update(b"native-syntax-diagnostic\0");
        hasher.update(diagnostic.owner_path.as_bytes());
        hasher.update(&[0]);
        hasher.update(diagnostic.content_digest.as_bytes());
        hasher.update(&[0]);
        hasher.update(diagnostic.reason_kind.as_bytes());
        hasher.update(&[0]);
        hasher.update(diagnostic.message.as_bytes());
        hasher.update(&[0]);
    }
    let mut query_key_rows = owner
        .selectors
        .iter()
        .flat_map(|selector| {
            selector
                .query_keys
                .iter()
                .map(move |key| (selector.selector.as_str(), key.as_str()))
        })
        .collect::<Vec<_>>();
    query_key_rows.sort_unstable();
    for (selector, key) in query_key_rows {
        hasher.update(b"query-key\0");
        hasher.update(selector.as_bytes());
        hasher.update(&[0]);
        hasher.update(key.as_bytes());
        hasher.update(&[0]);
    }
    let mut rows = owner
        .selectors
        .iter()
        .flat_map(|selector| {
            std::iter::once((
                "source",
                selector.selector.as_str(),
                selector.byte_start,
                selector.byte_end,
                &[][..],
            ))
            .chain(selector.derived_projections.iter().map(|projection| {
                (
                    projection.projection_kind.as_str(),
                    selector.selector.as_str(),
                    selector.byte_start,
                    selector.byte_end,
                    projection.bytes.as_slice(),
                )
            }))
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        left.0
            .cmp(right.0)
            .then_with(|| left.1.cmp(right.1))
            .then_with(|| left.2.cmp(&right.2))
            .then_with(|| left.3.cmp(&right.3))
            .then_with(|| left.4.cmp(right.4))
    });
    for (projection_kind, selector, byte_start, byte_end, projection_bytes) in rows {
        hash_len_prefixed(&mut hasher, projection_kind.as_bytes());
        hash_len_prefixed(&mut hasher, selector.as_bytes());
        hasher.update(&(byte_start as u64).to_le_bytes());
        hasher.update(&(byte_end as u64).to_le_bytes());
        hash_len_prefixed(&mut hasher, projection_bytes);
    }
    *hasher.finalize().as_bytes()
}

fn hash_len_prefixed(hasher: &mut blake3::Hasher, bytes: &[u8]) {
    hasher.update(&(bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

fn slice_from_range(bytes: &[u8], range: (usize, usize)) -> &[u8] {
    &bytes[range.0..range.0 + range.1]
}
