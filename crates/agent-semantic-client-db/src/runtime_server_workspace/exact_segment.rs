use memmap2::{Mmap, MmapOptions};
use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use tokio::fs;

use super::{
    WorkspaceGenerationPointerReader, WorkspaceGenerationSnapshot, WorkspaceOwnerSnapshot,
    WorkspaceRuntimeSelectorRead, WorkspaceSelectorSnapshot,
};
#[path = "exact_segment_encoding.rs"]
mod encoding;
#[path = "exact_segment_format.rs"]
mod format;
use format::{
    checked_entry_offset, decode_header, read_slice, read_text, read_usize, write_range_entry,
    write_range_header, write_u64, write_usize,
};

const MAGIC: &[u8; 16] = b"ASPEXACTMMAP0002";
const HEADER_LEN: usize = 144;
const OWNER_ENTRY_LEN: usize = 112;
const SELECTOR_ENTRY_LEN: usize = 104;
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

#[derive(Debug)]
pub struct WorkspaceExactProjectionDataPlaneClient {
    pointer: WorkspaceGenerationPointerReader,
    mapped: MappedWorkspaceExactProjection,
}

#[derive(Debug)]
pub enum WorkspaceExactProjectionDataPlaneOpen {
    Ready(WorkspaceExactProjectionDataPlaneClient),
    Missing,
    RecoveryRequired { reason: String },
}

impl WorkspaceExactProjectionDataPlaneClient {
    pub async fn open_state(
        pointer_path: &Path,
    ) -> Result<WorkspaceExactProjectionDataPlaneOpen, String> {
        if !fs::try_exists(pointer_path)
            .await
            .map_err(|error| format!("inspect workspace generation pointer: {error}"))?
        {
            return Ok(WorkspaceExactProjectionDataPlaneOpen::Missing);
        }
        match Self::open(pointer_path).await {
            Ok(client) => Ok(WorkspaceExactProjectionDataPlaneOpen::Ready(client)),
            Err(reason) => Ok(WorkspaceExactProjectionDataPlaneOpen::RecoveryRequired { reason }),
        }
    }

    pub async fn open(pointer_path: &Path) -> Result<Self, String> {
        let pointer = WorkspaceGenerationPointerReader::open(pointer_path).await?;
        let snapshot = pointer.read()?;
        let mapped = MappedWorkspaceExactProjection::open(&snapshot).await?;
        Ok(Self { pointer, mapped })
    }

    pub fn read_runtime_selector(
        &self,
        projection_kind: &str,
        structural_selector: &str,
    ) -> Result<WorkspaceRuntimeSelectorRead, String> {
        self.mapped
            .read_runtime_selector(projection_kind, structural_selector)
    }

    /// Return the resident content identity for one exact owner without
    /// opening the workspace database or contacting the control plane.
    pub fn owner_content_digest(&self, owner_path: &str) -> Result<Option<String>, String> {
        self.mapped.owner_content_digest(owner_path)
    }

    pub fn contains_owner(&self, owner: &WorkspaceOwnerSnapshot) -> Result<bool, String> {
        self.mapped.contains_owner(owner)
    }

    pub async fn refresh_if_changed(&mut self) -> Result<bool, String> {
        let snapshot = self.pointer.read()?;
        if self.mapped.epoch == snapshot.active_epoch {
            return Ok(false);
        }
        self.mapped = MappedWorkspaceExactProjection::open(&snapshot).await?;
        Ok(true)
    }
}

#[derive(Debug)]
struct MappedWorkspaceExactProjection {
    mapping: Mmap,
    epoch: u64,
    owner_count: usize,
    selector_count: usize,
    owner_table_offset: usize,
    selector_table_offset: usize,
    relocation_table_offset: usize,
    relocation_count: usize,
    generation_digest: String,
    root_digest: String,
}

impl MappedWorkspaceExactProjection {
    async fn open(snapshot: &WorkspaceGenerationSnapshot) -> Result<Self, String> {
        let path = exact_projection_segment_path(Path::new(&snapshot.mmap_segment_path));
        let file = std::fs::File::open(&path)
            .map_err(|error| format!("open workspace exact projection segment: {error}"))?;
        let mapping = unsafe {
            // SAFETY: exact projection segments are immutable after atomic
            // publication and this value owns the mapping lifetime.
            MmapOptions::new()
                .map(&file)
                .map_err(|error| format!("map workspace exact projection segment: {error}"))?
        };
        let header = decode_header(&mapping)?;
        if header.epoch != snapshot.active_epoch {
            return Err("workspace exact projection epoch does not match pointer".to_owned());
        }
        let workspace_identity = read_text(
            &mapping,
            header.workspace_id_offset,
            header.workspace_id_len,
            "workspace identity",
        )?;
        if workspace_identity != snapshot.workspace_identity {
            return Err("workspace exact projection identity does not match pointer".to_owned());
        }
        let generation_digest = read_text(
            &mapping,
            header.generation_digest_offset,
            header.generation_digest_len,
            "generation digest",
        )?
        .to_owned();
        if generation_digest != snapshot.generation_digest {
            return Err("workspace exact projection generation does not match pointer".to_owned());
        }
        let root_digest = read_text(
            &mapping,
            header.root_digest_offset,
            header.root_digest_len,
            "root digest",
        )?
        .to_owned();
        Ok(Self {
            mapping,
            epoch: header.epoch,
            owner_count: header.owner_count,
            selector_count: header.selector_count,
            owner_table_offset: header.owner_table_offset,
            selector_table_offset: header.selector_table_offset,
            relocation_table_offset: header.relocation_table_offset,
            relocation_count: header.relocation_count,
            generation_digest,
            root_digest,
        })
    }

    fn read_runtime_selector(
        &self,
        projection_kind: &str,
        structural_selector: &str,
    ) -> Result<WorkspaceRuntimeSelectorRead, String> {
        super::selector_overlay::validate_projection_kind(projection_kind)?;
        if let Some(selector) = self.find_selector(projection_kind, structural_selector)? {
            return self.project_selector(projection_kind, selector);
        }
        let relocated = self.find_relocated_selectors(projection_kind, structural_selector)?;
        if relocated.len() == 1 {
            return self.project_selector(projection_kind, relocated[0].1);
        }
        if relocated.len() > 1 {
            return Ok(WorkspaceRuntimeSelectorRead::RelocationAmbiguous {
                generation_digest: self.generation_digest.clone(),
                root_digest: self.root_digest.clone(),
                candidates: relocated
                    .into_iter()
                    .map(|(selector, _)| selector)
                    .collect(),
            });
        }
        let owner_path = structural_selector
            .split_once("://")
            .and_then(|(_, selector)| selector.split_once('#'))
            .map(|(owner_path, _)| owner_path)
            .ok_or_else(|| "exact structural selector is missing its owner path".to_owned())?;
        let Some((owner_index, owner)) = self.find_owner(owner_path)? else {
            return Ok(WorkspaceRuntimeSelectorRead::OwnerMissing {
                generation_digest: self.generation_digest.clone(),
                root_digest: self.root_digest.clone(),
            });
        };
        Ok(WorkspaceRuntimeSelectorRead::OwnerForRepair {
            generation_digest: self.generation_digest.clone(),
            root_digest: self.root_digest.clone(),
            owner: self.owner_snapshot(owner_index, &owner)?,
        })
    }

    fn project_selector(
        &self,
        projection_kind: &str,
        selector: SelectorEntry,
    ) -> Result<WorkspaceRuntimeSelectorRead, String> {
        let resolved_selector = self.selector_text(&selector)?.to_owned();
        let owner = self.owner_entry(selector.owner_index)?;
        let projection = if projection_kind == "source" {
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
            content_digest: self.owner_digest(owner)?.to_owned(),
            bytes: self.owner_bytes(owner)?.to_vec(),
            selectors: self.owner_selectors(owner_index)?,
        })
    }

    fn owner_selectors(
        &self,
        owner_index: usize,
    ) -> Result<Vec<WorkspaceSelectorSnapshot>, String> {
        let mut selectors = Vec::new();
        for index in 0..self.selector_count {
            let entry = self.selector_entry(index)?;
            if entry.owner_index == owner_index && self.selector_kind(&entry)? == "source" {
                let selector_text = self.selector_text(&entry)?;
                let mut derived_projections = Vec::new();
                for derived_index in 0..self.selector_count {
                    let derived = self.selector_entry(derived_index)?;
                    if derived.owner_index == owner_index
                        && self.selector_text(&derived)? == selector_text
                        && self.selector_kind(&derived)? != "source"
                    {
                        derived_projections.push(super::WorkspaceDerivedProjectionSnapshot {
                            projection_kind: self.selector_kind(&derived)?.to_owned(),
                            bytes: read_slice(
                                &self.mapping,
                                derived.projection_blob_offset,
                                derived.projection_blob_len,
                                "derived projection bytes",
                            )?
                            .to_vec(),
                        });
                    }
                }
                derived_projections
                    .sort_by(|left, right| left.projection_kind.cmp(&right.projection_kind));
                selectors.push(WorkspaceSelectorSnapshot {
                    selector: selector_text.to_owned(),
                    byte_start: entry.byte_start,
                    byte_end: entry.byte_end,
                    derived_projections,
                });
            }
        }
        selectors.sort_by(|left, right| left.selector.cmp(&right.selector));
        Ok(selectors)
    }

    fn find_owner(&self, owner_path: &str) -> Result<Option<(usize, OwnerEntry)>, String> {
        let hash = *blake3::hash(owner_path.as_bytes()).as_bytes();
        let mut low = 0;
        let mut high = self.owner_count;
        while low < high {
            let middle = low + (high - low) / 2;
            let entry = self.owner_entry(middle)?;
            match entry.hash.cmp(&hash) {
                Ordering::Less => low = middle + 1,
                Ordering::Greater => high = middle,
                Ordering::Equal => {
                    let mut first = middle;
                    while first > 0 && self.owner_entry(first - 1)?.hash == hash {
                        first -= 1;
                    }
                    let mut index = first;
                    while index < self.owner_count {
                        let candidate = self.owner_entry(index)?;
                        if candidate.hash != hash {
                            break;
                        }
                        if self.owner_path(&candidate)? == owner_path {
                            return Ok(Some((index, candidate)));
                        }
                        index += 1;
                    }
                    return Ok(None);
                }
            }
        }
        Ok(None)
    }

    fn find_selector(
        &self,
        projection_kind: &str,
        selector: &str,
    ) -> Result<Option<SelectorEntry>, String> {
        let hash = projection_key_hash(projection_kind, selector);
        let mut low = 0;
        let mut high = self.selector_count;
        while low < high {
            let middle = low + (high - low) / 2;
            let entry = self.selector_entry(middle)?;
            match entry.hash.cmp(&hash) {
                Ordering::Less => low = middle + 1,
                Ordering::Greater => high = middle,
                Ordering::Equal => {
                    let mut first = middle;
                    while first > 0 && self.selector_entry(first - 1)?.hash == hash {
                        first -= 1;
                    }
                    let mut index = first;
                    while index < self.selector_count {
                        let candidate = self.selector_entry(index)?;
                        if candidate.hash != hash {
                            break;
                        }
                        if self.selector_kind(&candidate)? == projection_kind
                            && self.selector_text(&candidate)? == selector
                        {
                            return Ok(Some(candidate));
                        }
                        index += 1;
                    }
                    return Ok(None);
                }
            }
        }
        Ok(None)
    }

    fn find_relocated_selectors(
        &self,
        projection_kind: &str,
        selector: &str,
    ) -> Result<Vec<(String, SelectorEntry)>, String> {
        let requested_identity = relocation_identity(selector)?;
        let hash = relocation_key_hash(projection_kind, requested_identity.as_str());
        let mut low = 0;
        let mut high = self.relocation_count;
        while low < high {
            let middle = low + (high - low) / 2;
            match self.relocation_entry(middle)?.hash.cmp(&hash) {
                Ordering::Less => low = middle + 1,
                Ordering::Greater => high = middle,
                Ordering::Equal => {
                    let mut first = middle;
                    while first > 0 && self.relocation_entry(first - 1)?.hash == hash {
                        first -= 1;
                    }
                    let mut matches = Vec::new();
                    let mut index = first;
                    while index < self.relocation_count {
                        let relocation = self.relocation_entry(index)?;
                        if relocation.hash != hash {
                            break;
                        }
                        let candidate = self.selector_entry(relocation.selector_index)?;
                        let candidate_text = self.selector_text(&candidate)?;
                        if self.selector_kind(&candidate)? == projection_kind
                            && relocation_identity(candidate_text)? == requested_identity
                        {
                            matches.push((candidate_text.to_owned(), candidate));
                        }
                        index += 1;
                    }
                    matches.sort_by(|left, right| left.0.cmp(&right.0));
                    matches.dedup_by(|left, right| left.0 == right.0);
                    return Ok(matches);
                }
            }
        }
        Ok(Vec::new())
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
            projection_digest: bytes[80..112]
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

    fn selector_text<'a>(&'a self, selector: &SelectorEntry) -> Result<&'a str, String> {
        read_text(
            &self.mapping,
            selector.selector_offset,
            selector.selector_len,
            "selector text",
        )
    }

    fn selector_kind<'a>(&'a self, selector: &SelectorEntry) -> Result<&'a str, String> {
        read_text(
            &self.mapping,
            selector.projection_kind_offset,
            selector.projection_kind_len,
            "projection kind",
        )
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
    agent_semantic_content_identity::CanonicalItemSelector::parse_root_or_exact_descendant(
        selector,
    )?;
    let (language_id, body) = selector
        .split_once("://")
        .ok_or_else(|| "canonical selector is missing its language".to_owned())?;
    let (_, fragment) = body
        .split_once('#')
        .ok_or_else(|| "canonical selector is missing its identity fragment".to_owned())?;
    Ok(format!("{language_id}#{fragment}"))
}

fn relocation_key_hash(projection_kind: &str, identity: &str) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(projection_kind.as_bytes());
    hasher.update(&[0]);
    hasher.update(identity.as_bytes());
    *hasher.finalize().as_bytes()
}

fn owner_projection_digest(owner: &WorkspaceOwnerSnapshot) -> [u8; 32] {
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
    let mut hasher = blake3::Hasher::new();
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
