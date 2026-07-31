use memmap2::{Mmap, MmapOptions};
use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use tokio::fs;

use super::{
    WorkspaceGenerationPointerReader, WorkspaceGenerationSnapshot, WorkspaceMemoryGeneration,
    WorkspaceOwnerSnapshot, WorkspaceRuntimeSelectorRead, WorkspaceSelectorSnapshot,
};

const MAGIC: &[u8; 16] = b"ASPEXACTMMAP0001";
const HEADER_LEN: usize = 128;
const OWNER_ENTRY_LEN: usize = 112;
const SELECTOR_ENTRY_LEN: usize = 104;

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
            let owner = self.owner_entry(selector.owner_index)?;
            let projection = if projection_kind == "source" {
                self.owner_bytes(&owner)?
                    .get(selector.byte_start..selector.byte_end)
                    .ok_or_else(|| {
                        "workspace exact projection selector range is invalid".to_owned()
                    })?
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
            return Ok(WorkspaceRuntimeSelectorRead::Projection {
                generation_digest: self.generation_digest.clone(),
                root_digest: self.root_digest.clone(),
                bytes: projection,
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

struct Header {
    epoch: u64,
    owner_count: usize,
    selector_count: usize,
    owner_table_offset: usize,
    selector_table_offset: usize,
    string_table_offset: usize,
    blob_offset: usize,
    generation_digest_offset: usize,
    generation_digest_len: usize,
    root_digest_offset: usize,
    root_digest_len: usize,
    workspace_id_offset: usize,
    workspace_id_len: usize,
}

pub(super) fn encode_exact_projection_segment(
    generation: &WorkspaceMemoryGeneration,
) -> Result<Vec<u8>, String> {
    generation.validate()?;
    let mut owners = generation.owners.iter().collect::<Vec<_>>();
    owners.sort_by(|left, right| {
        blake3::hash(left.owner_path.as_bytes())
            .as_bytes()
            .cmp(blake3::hash(right.owner_path.as_bytes()).as_bytes())
            .then_with(|| left.owner_path.cmp(&right.owner_path))
    });
    let owner_table_offset = HEADER_LEN;
    let selector_count = owners
        .iter()
        .flat_map(|owner| owner.selectors.iter())
        .map(|selector| 1 + selector.derived_projections.len())
        .sum::<usize>();
    let selector_table_offset = owner_table_offset
        .checked_add(owners.len().saturating_mul(OWNER_ENTRY_LEN))
        .ok_or_else(|| "workspace exact owner table length overflow".to_owned())?;
    let string_table_offset = selector_table_offset
        .checked_add(selector_count.saturating_mul(SELECTOR_ENTRY_LEN))
        .ok_or_else(|| "workspace exact selector table length overflow".to_owned())?;

    let mut strings = Vec::new();
    let generation_digest = push_bytes(&mut strings, generation.generation_digest.as_bytes());
    let root_digest = push_bytes(
        &mut strings,
        generation.source_snapshot.root_digest.as_bytes(),
    );
    let workspace_identity = push_bytes(&mut strings, generation.workspace_identity.as_bytes());
    let mut owner_rows = Vec::with_capacity(owners.len());
    let mut selector_rows = Vec::with_capacity(selector_count);
    let mut blobs = Vec::new();
    for (owner_index, owner) in owners.iter().enumerate() {
        let path = push_bytes(&mut strings, owner.owner_path.as_bytes());
        let digest = push_bytes(&mut strings, owner.content_digest.as_bytes());
        let blob_offset = blobs.len();
        blobs.extend_from_slice(&owner.bytes);
        owner_rows.push((
            *blake3::hash(owner.owner_path.as_bytes()).as_bytes(),
            path,
            digest,
            blob_offset,
            owner.bytes.len(),
            owner_projection_digest(owner),
        ));
        for selector in &owner.selectors {
            let text = push_bytes(&mut strings, selector.selector.as_bytes());
            let source_kind = push_bytes(&mut strings, b"source");
            selector_rows.push((
                projection_key_hash("source", &selector.selector),
                text,
                source_kind,
                owner_index,
                selector.byte_start,
                selector.byte_end,
                0,
                0,
            ));
            for projection in &selector.derived_projections {
                let projection_kind =
                    push_bytes(&mut strings, projection.projection_kind.as_bytes());
                let projection_blob_offset = blobs.len();
                blobs.extend_from_slice(&projection.bytes);
                selector_rows.push((
                    projection_key_hash(&projection.projection_kind, &selector.selector),
                    text,
                    projection_kind,
                    owner_index,
                    selector.byte_start,
                    selector.byte_end,
                    projection_blob_offset,
                    projection.bytes.len(),
                ));
            }
        }
    }
    selector_rows.sort_by(|left, right| {
        left.0.cmp(&right.0).then_with(|| {
            slice_from_range(&strings, left.2)
                .cmp(slice_from_range(&strings, right.2))
                .then_with(|| {
                    slice_from_range(&strings, left.1).cmp(slice_from_range(&strings, right.1))
                })
        })
    });
    let blob_offset = string_table_offset
        .checked_add(strings.len())
        .ok_or_else(|| "workspace exact string table length overflow".to_owned())?;
    let total_len = blob_offset
        .checked_add(blobs.len())
        .ok_or_else(|| "workspace exact segment length overflow".to_owned())?;
    let mut segment = vec![0_u8; total_len];
    segment[0..16].copy_from_slice(MAGIC);
    write_u64(&mut segment, EPOCH_OFFSET, generation.active_epoch)?;
    write_usize(&mut segment, OWNER_COUNT_OFFSET, owners.len())?;
    write_usize(&mut segment, SELECTOR_COUNT_OFFSET, selector_rows.len())?;
    write_usize(&mut segment, OWNER_TABLE_OFFSET, owner_table_offset)?;
    write_usize(&mut segment, SELECTOR_TABLE_OFFSET, selector_table_offset)?;
    write_usize(&mut segment, STRING_TABLE_OFFSET, string_table_offset)?;
    write_usize(&mut segment, BLOB_OFFSET, blob_offset)?;
    write_usize(&mut segment, TOTAL_LEN_OFFSET, total_len)?;
    write_range_header(
        &mut segment,
        GENERATION_DIGEST_OFFSET,
        GENERATION_DIGEST_LEN_OFFSET,
        string_table_offset,
        generation_digest,
    )?;
    write_range_header(
        &mut segment,
        ROOT_DIGEST_OFFSET,
        ROOT_DIGEST_LEN_OFFSET,
        string_table_offset,
        root_digest,
    )?;
    write_range_header(
        &mut segment,
        WORKSPACE_ID_OFFSET,
        WORKSPACE_ID_LEN_OFFSET,
        string_table_offset,
        workspace_identity,
    )?;
    for (index, (hash, path, digest, owner_blob_offset, owner_blob_len, owner_projection_digest)) in
        owner_rows.into_iter().enumerate()
    {
        let start = owner_table_offset + index * OWNER_ENTRY_LEN;
        segment[start..start + 32].copy_from_slice(&hash);
        write_range_entry(&mut segment, start + 32, string_table_offset, path)?;
        write_range_entry(&mut segment, start + 48, string_table_offset, digest)?;
        write_usize(&mut segment, start + 64, blob_offset + owner_blob_offset)?;
        write_usize(&mut segment, start + 72, owner_blob_len)?;
        segment[start + 80..start + 112].copy_from_slice(&owner_projection_digest);
    }
    for (
        index,
        (
            hash,
            selector,
            projection_kind,
            owner_index,
            byte_start,
            byte_end,
            projection_blob_offset,
            projection_blob_len,
        ),
    ) in selector_rows.into_iter().enumerate()
    {
        let start = selector_table_offset + index * SELECTOR_ENTRY_LEN;
        segment[start..start + 32].copy_from_slice(&hash);
        write_range_entry(&mut segment, start + 32, string_table_offset, selector)?;
        write_range_entry(
            &mut segment,
            start + 48,
            string_table_offset,
            projection_kind,
        )?;
        write_usize(&mut segment, start + 64, owner_index)?;
        write_usize(&mut segment, start + 72, byte_start)?;
        write_usize(&mut segment, start + 80, byte_end)?;
        write_usize(
            &mut segment,
            start + 88,
            blob_offset + projection_blob_offset,
        )?;
        write_usize(&mut segment, start + 96, projection_blob_len)?;
    }
    segment[string_table_offset..blob_offset].copy_from_slice(&strings);
    segment[blob_offset..].copy_from_slice(&blobs);
    Ok(segment)
}

pub(super) fn exact_projection_segment_path(generation_path: &Path) -> PathBuf {
    generation_path.with_extension("exact.mmap")
}

fn decode_header(mapping: &[u8]) -> Result<Header, String> {
    if mapping.len() < HEADER_LEN || &mapping[0..16] != MAGIC {
        return Err("workspace exact projection segment header is invalid".to_owned());
    }
    let total_len = read_usize(mapping, TOTAL_LEN_OFFSET, "segment total length")?;
    if total_len != mapping.len() {
        return Err("workspace exact projection segment length mismatch".to_owned());
    }
    let header = Header {
        epoch: read_u64(mapping, EPOCH_OFFSET, "generation epoch")?,
        owner_count: read_usize(mapping, OWNER_COUNT_OFFSET, "owner count")?,
        selector_count: read_usize(mapping, SELECTOR_COUNT_OFFSET, "selector count")?,
        owner_table_offset: read_usize(mapping, OWNER_TABLE_OFFSET, "owner table offset")?,
        selector_table_offset: read_usize(mapping, SELECTOR_TABLE_OFFSET, "selector table offset")?,
        string_table_offset: read_usize(mapping, STRING_TABLE_OFFSET, "string table offset")?,
        blob_offset: read_usize(mapping, BLOB_OFFSET, "blob offset")?,
        generation_digest_offset: read_usize(
            mapping,
            GENERATION_DIGEST_OFFSET,
            "generation digest offset",
        )?,
        generation_digest_len: read_usize(
            mapping,
            GENERATION_DIGEST_LEN_OFFSET,
            "generation digest length",
        )?,
        root_digest_offset: read_usize(mapping, ROOT_DIGEST_OFFSET, "root digest offset")?,
        root_digest_len: read_usize(mapping, ROOT_DIGEST_LEN_OFFSET, "root digest length")?,
        workspace_id_offset: read_usize(mapping, WORKSPACE_ID_OFFSET, "workspace id offset")?,
        workspace_id_len: read_usize(mapping, WORKSPACE_ID_LEN_OFFSET, "workspace id length")?,
    };
    let owner_table_end = header
        .owner_table_offset
        .checked_add(header.owner_count.saturating_mul(OWNER_ENTRY_LEN))
        .ok_or_else(|| "workspace exact owner table overflow".to_owned())?;
    let selector_table_end = header
        .selector_table_offset
        .checked_add(header.selector_count.saturating_mul(SELECTOR_ENTRY_LEN))
        .ok_or_else(|| "workspace exact selector table overflow".to_owned())?;
    if header.owner_table_offset != HEADER_LEN
        || header.selector_table_offset != owner_table_end
        || header.string_table_offset != selector_table_end
        || header.string_table_offset > header.blob_offset
        || header.blob_offset > mapping.len()
    {
        return Err("workspace exact projection table topology is invalid".to_owned());
    }
    Ok(header)
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

fn write_range_header(
    target: &mut [u8],
    offset_field: usize,
    len_field: usize,
    base: usize,
    range: (usize, usize),
) -> Result<(), String> {
    write_usize(target, offset_field, base + range.0)?;
    write_usize(target, len_field, range.1)
}

fn write_range_entry(
    target: &mut [u8],
    field: usize,
    base: usize,
    range: (usize, usize),
) -> Result<(), String> {
    write_usize(target, field, base + range.0)?;
    write_usize(target, field + 8, range.1)
}

fn write_usize(target: &mut [u8], offset: usize, value: usize) -> Result<(), String> {
    let value = u64::try_from(value)
        .map_err(|_| "workspace exact projection value overflows u64".to_owned())?;
    write_u64(target, offset, value)
}

fn write_u64(target: &mut [u8], offset: usize, value: u64) -> Result<(), String> {
    let end = offset
        .checked_add(8)
        .ok_or_else(|| "workspace exact projection write offset overflow".to_owned())?;
    let destination = target
        .get_mut(offset..end)
        .ok_or_else(|| "workspace exact projection write is out of range".to_owned())?;
    destination.copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn read_usize(bytes: &[u8], offset: usize, field: &str) -> Result<usize, String> {
    usize::try_from(read_u64(bytes, offset, field)?)
        .map_err(|_| format!("workspace exact projection {field} overflows usize"))
}

fn read_u64(bytes: &[u8], offset: usize, field: &str) -> Result<u64, String> {
    let end = offset
        .checked_add(8)
        .ok_or_else(|| format!("workspace exact projection {field} offset overflow"))?;
    let raw = bytes
        .get(offset..end)
        .ok_or_else(|| format!("workspace exact projection {field} is out of range"))?;
    Ok(u64::from_le_bytes(raw.try_into().map_err(|_| {
        format!("workspace exact projection {field} is invalid")
    })?))
}

fn checked_entry_offset(
    table_offset: usize,
    index: usize,
    entry_len: usize,
    mapping_len: usize,
) -> Result<usize, String> {
    let start = table_offset
        .checked_add(index.saturating_mul(entry_len))
        .ok_or_else(|| "workspace exact projection entry offset overflow".to_owned())?;
    let end = start
        .checked_add(entry_len)
        .ok_or_else(|| "workspace exact projection entry length overflow".to_owned())?;
    if end > mapping_len {
        return Err("workspace exact projection entry is out of range".to_owned());
    }
    Ok(start)
}

fn read_slice<'a>(
    bytes: &'a [u8],
    offset: usize,
    len: usize,
    field: &str,
) -> Result<&'a [u8], String> {
    let end = offset
        .checked_add(len)
        .ok_or_else(|| format!("workspace exact projection {field} range overflow"))?;
    bytes
        .get(offset..end)
        .ok_or_else(|| format!("workspace exact projection {field} is out of range"))
}

fn read_text<'a>(
    bytes: &'a [u8],
    offset: usize,
    len: usize,
    field: &str,
) -> Result<&'a str, String> {
    std::str::from_utf8(read_slice(bytes, offset, len, field)?)
        .map_err(|error| format!("workspace exact projection {field} is not UTF-8: {error}"))
}
