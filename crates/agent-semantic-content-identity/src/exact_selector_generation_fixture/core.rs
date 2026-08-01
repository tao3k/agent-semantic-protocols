use std::cmp::Ordering;
use std::fmt;
use std::ops::Range;

pub const EXACT_SELECTOR_GENERATION_FIXTURE_SCHEMA_ID: &str =
    "agent.semantic-protocols.exact-selector-generation-fixture";
pub const EXACT_SELECTOR_GENERATION_FIXTURE_SCHEMA_VERSION: &str = "1";

const MAGIC: &[u8; 8] = b"ASPXGFV1";
pub(crate) const DIGEST_LEN: usize = 32;
const FIXED_HEADER_LEN: usize = 8 + 4 + 4 + 4 + 4 + 2 + 2 + (DIGEST_LEN * 6);
const FIXTURE_DIGEST_OFFSET: usize = FIXED_HEADER_LEN - DIGEST_LEN;
const RECORD_HEADER_LEN: usize = 4 + 4 + 4 + 1 + 8 + 8 + (DIGEST_LEN * 3);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ExactSelectorProjectionModeV1 {
    Source = 1,
    CallableSkeleton = 2,
}

impl ExactSelectorProjectionModeV1 {
    fn from_byte(value: u8) -> Result<Self, ExactSelectorGenerationFixtureErrorV1> {
        match value {
            1 => Ok(Self::Source),
            2 => Ok(Self::CallableSkeleton),
            _ => Err(ExactSelectorGenerationFixtureErrorV1::InvalidProjectionMode(value)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactSelectorGenerationIdentityV1 {
    pub language_id: String,
    pub provider_id: String,
    pub workspace_root_digest: [u8; DIGEST_LEN],
    pub workspace_identity_digest: [u8; DIGEST_LEN],
    pub parser_identity_digest: [u8; DIGEST_LEN],
    pub query_pack_digest: [u8; DIGEST_LEN],
    pub generation_digest: [u8; DIGEST_LEN],
    pub selector_count: u32,
    pub owner_count: u32,
    pub leaf_count: u32,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactSelectorGenerationRecordV1 {
    pub structural_selector: String,
    pub owner_path: String,
    pub owner_subtree_digest: [u8; DIGEST_LEN],
    pub source_blob_digest: [u8; DIGEST_LEN],
    pub normalized_parser_facts_digest: [u8; DIGEST_LEN],
    pub projection_mode: ExactSelectorProjectionModeV1,
    pub source_byte_range: Range<u64>,
    pub projection: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactSelectorGenerationRecordViewV1<'a> {
    pub structural_selector: &'a str,
    pub owner_path: &'a str,
    pub owner_subtree_digest: &'a [u8; DIGEST_LEN],
    pub source_blob_digest: &'a [u8; DIGEST_LEN],
    pub normalized_parser_facts_digest: &'a [u8; DIGEST_LEN],
    pub projection_mode: ExactSelectorProjectionModeV1,
    pub source_byte_range: Range<u64>,
    pub projection: &'a [u8],
}

#[derive(Clone, Copy, Debug)]
pub struct ExactSelectorGenerationFixtureViewV1<'a> {
    bytes: &'a [u8],
    language_id: &'a str,
    provider_id: &'a str,
    workspace_root_digest: &'a [u8; DIGEST_LEN],
    workspace_identity_digest: &'a [u8; DIGEST_LEN],
    parser_identity_digest: &'a [u8; DIGEST_LEN],
    query_pack_digest: &'a [u8; DIGEST_LEN],
    generation_digest: &'a [u8; DIGEST_LEN],
    fixture_digest: &'a [u8; DIGEST_LEN],
    record_count: u32,
    selector_count: u32,
    owner_count: u32,
    leaf_count: u32,
    offsets_start: usize,
    records_start: usize,
}

impl<'a> ExactSelectorGenerationFixtureViewV1<'a> {
    pub fn attach(
        bytes: &'a [u8],
        expected_generation_digest: &[u8; DIGEST_LEN],
        expected_fixture_digest: &[u8; DIGEST_LEN],
    ) -> Result<Self, ExactSelectorGenerationFixtureErrorV1> {
        if bytes.len() < FIXED_HEADER_LEN || bytes.get(..MAGIC.len()) != Some(MAGIC) {
            return Err(ExactSelectorGenerationFixtureErrorV1::InvalidHeader);
        }
        let record_count = read_u32(bytes, 8)?;
        let owner_count = read_u32(bytes, 12)?;
        let leaf_count = read_u32(bytes, 16)?;
        let selector_count = read_u32(bytes, 20)?;
        if record_count == 0
            || record_count != selector_count
            || owner_count == 0
            || owner_count != leaf_count
        {
            return Err(
                ExactSelectorGenerationFixtureErrorV1::IncompleteGeneration {
                    record_count,
                    selector_count,
                    owner_count,
                    leaf_count,
                },
            );
        }
        let language_len = usize::from(read_u16(bytes, 24)?);
        let provider_len = usize::from(read_u16(bytes, 26)?);
        let workspace_root_digest = digest_at(bytes, 28)?;
        let workspace_identity_digest = digest_at(bytes, 60)?;
        let parser_identity_digest = digest_at(bytes, 92)?;
        let query_pack_digest = digest_at(bytes, 124)?;
        let generation_digest = digest_at(bytes, 156)?;
        let fixture_digest = digest_at(bytes, FIXTURE_DIGEST_OFFSET)?;
        if generation_digest != expected_generation_digest {
            return Err(ExactSelectorGenerationFixtureErrorV1::GenerationMismatch);
        }
        if fixture_digest != expected_fixture_digest {
            return Err(ExactSelectorGenerationFixtureErrorV1::FixtureMismatch);
        }
        let language_start = FIXED_HEADER_LEN;
        let provider_start = language_start
            .checked_add(language_len)
            .ok_or(ExactSelectorGenerationFixtureErrorV1::InvalidHeader)?;
        let offsets_start = provider_start
            .checked_add(provider_len)
            .ok_or(ExactSelectorGenerationFixtureErrorV1::InvalidHeader)?;
        let offsets_len = usize::try_from(record_count)
            .ok()
            .and_then(|count| count.checked_add(1))
            .and_then(|count| count.checked_mul(8))
            .ok_or(ExactSelectorGenerationFixtureErrorV1::InvalidHeader)?;
        let records_start = offsets_start
            .checked_add(offsets_len)
            .ok_or(ExactSelectorGenerationFixtureErrorV1::InvalidHeader)?;
        if records_start > bytes.len() {
            return Err(ExactSelectorGenerationFixtureErrorV1::InvalidHeader);
        }
        let final_offset = read_u64(
            bytes,
            offsets_start
                .checked_add(
                    usize::try_from(record_count)
                        .map_err(|_| ExactSelectorGenerationFixtureErrorV1::InvalidHeader)?
                        .checked_mul(8)
                        .ok_or(ExactSelectorGenerationFixtureErrorV1::InvalidHeader)?,
                )
                .ok_or(ExactSelectorGenerationFixtureErrorV1::InvalidHeader)?,
        )?;
        let records_len = u64::try_from(bytes.len() - records_start)
            .map_err(|_| ExactSelectorGenerationFixtureErrorV1::InvalidHeader)?;
        if final_offset != records_len {
            return Err(ExactSelectorGenerationFixtureErrorV1::InvalidOffset);
        }
        let language_id = utf8_slice(bytes, language_start, provider_start)?;
        let provider_end = offsets_start;
        let provider_id = utf8_slice(bytes, provider_start, provider_end)?;
        if language_id.is_empty() || provider_id.is_empty() {
            return Err(ExactSelectorGenerationFixtureErrorV1::InvalidHeader);
        }
        Ok(Self {
            bytes,
            language_id,
            provider_id,
            workspace_root_digest,
            workspace_identity_digest,
            parser_identity_digest,
            query_pack_digest,
            generation_digest,
            fixture_digest,
            record_count,
            selector_count,
            owner_count,
            leaf_count,
            offsets_start,
            records_start,
        })
    }

    pub fn language_id(&self) -> &'a str {
        self.language_id
    }

    pub fn provider_id(&self) -> &'a str {
        self.provider_id
    }

    pub fn workspace_root_digest(&self) -> &'a [u8; DIGEST_LEN] {
        self.workspace_root_digest
    }

    pub fn workspace_identity_digest(&self) -> &'a [u8; DIGEST_LEN] {
        self.workspace_identity_digest
    }

    pub fn parser_identity_digest(&self) -> &'a [u8; DIGEST_LEN] {
        self.parser_identity_digest
    }

    pub fn query_pack_digest(&self) -> &'a [u8; DIGEST_LEN] {
        self.query_pack_digest
    }

    pub fn generation_digest(&self) -> &'a [u8; DIGEST_LEN] {
        self.generation_digest
    }

    pub fn fixture_digest(&self) -> &'a [u8; DIGEST_LEN] {
        self.fixture_digest
    }

    pub fn record_count(&self) -> u32 {
        self.record_count
    }

    pub fn owner_count(&self) -> u32 {
        self.owner_count
    }

    pub fn selector_count(&self) -> u32 {
        self.selector_count
    }

    pub fn leaf_count(&self) -> u32 {
        self.leaf_count
    }

    pub fn lookup(
        &self,
        structural_selector: &str,
    ) -> Result<
        Option<ExactSelectorGenerationRecordViewV1<'a>>,
        ExactSelectorGenerationFixtureErrorV1,
    > {
        let mut low = 0_u32;
        let mut high = self.record_count;
        while low < high {
            let middle = low + ((high - low) / 2);
            let record = self.record_at(middle)?;
            match record.structural_selector.cmp(structural_selector) {
                Ordering::Less => low = middle + 1,
                Ordering::Greater => high = middle,
                Ordering::Equal => return Ok(Some(record)),
            }
        }
        Ok(None)
    }

    fn record_at(
        &self,
        index: u32,
    ) -> Result<ExactSelectorGenerationRecordViewV1<'a>, ExactSelectorGenerationFixtureErrorV1>
    {
        if index >= self.record_count {
            return Err(ExactSelectorGenerationFixtureErrorV1::InvalidOffset);
        }
        let index = usize::try_from(index)
            .map_err(|_| ExactSelectorGenerationFixtureErrorV1::InvalidOffset)?;
        let start = usize::try_from(read_u64(self.bytes, self.offsets_start + (index * 8))?)
            .map_err(|_| ExactSelectorGenerationFixtureErrorV1::InvalidOffset)?;
        let end = usize::try_from(read_u64(
            self.bytes,
            self.offsets_start + ((index + 1) * 8),
        )?)
        .map_err(|_| ExactSelectorGenerationFixtureErrorV1::InvalidOffset)?;
        if start > end {
            return Err(ExactSelectorGenerationFixtureErrorV1::InvalidOffset);
        }
        let start = self
            .records_start
            .checked_add(start)
            .ok_or(ExactSelectorGenerationFixtureErrorV1::InvalidOffset)?;
        let end = self
            .records_start
            .checked_add(end)
            .ok_or(ExactSelectorGenerationFixtureErrorV1::InvalidOffset)?;
        let record = self
            .bytes
            .get(start..end)
            .ok_or(ExactSelectorGenerationFixtureErrorV1::InvalidOffset)?;
        decode_record(record)
    }
}

pub fn build_exact_selector_generation_fixture_v1(
    identity: &ExactSelectorGenerationIdentityV1,
    mut records: Vec<ExactSelectorGenerationRecordV1>,
) -> Result<Vec<u8>, ExactSelectorGenerationFixtureErrorV1> {
    if identity.language_id.is_empty()
        || identity.provider_id.is_empty()
        || identity.selector_count == 0
        || usize::try_from(identity.selector_count).ok() != Some(records.len())
        || identity.owner_count == 0
        || identity.owner_count != identity.leaf_count
        || records.is_empty()
    {
        return Err(
            ExactSelectorGenerationFixtureErrorV1::IncompleteGeneration {
                record_count: u32::try_from(records.len()).unwrap_or(u32::MAX),
                selector_count: identity.selector_count,
                owner_count: identity.owner_count,
                leaf_count: identity.leaf_count,
            },
        );
    }
    let language_len = u16::try_from(identity.language_id.len())
        .map_err(|_| ExactSelectorGenerationFixtureErrorV1::IdentityTooLong)?;
    let provider_len = u16::try_from(identity.provider_id.len())
        .map_err(|_| ExactSelectorGenerationFixtureErrorV1::IdentityTooLong)?;
    records
        .sort_unstable_by(|left, right| left.structural_selector.cmp(&right.structural_selector));
    if records
        .windows(2)
        .any(|window| window[0].structural_selector == window[1].structural_selector)
    {
        return Err(ExactSelectorGenerationFixtureErrorV1::DuplicateSelector);
    }
    let covered_owner_count = records
        .iter()
        .map(|record| record.owner_path.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let materialization_evidence_is_complete = records.iter().all(|record| {
        !record.owner_path.is_empty()
            && record.source_byte_range.start < record.source_byte_range.end
            && !record.projection.is_empty()
            && record.owner_subtree_digest.iter().any(|byte| *byte != 0)
            && record.source_blob_digest.iter().any(|byte| *byte != 0)
            && record
                .normalized_parser_facts_digest
                .iter()
                .any(|byte| *byte != 0)
    });
    if u32::try_from(covered_owner_count).ok() != Some(identity.owner_count)
        || !materialization_evidence_is_complete
    {
        return Err(
            ExactSelectorGenerationFixtureErrorV1::IncompleteGeneration {
                record_count: u32::try_from(records.len()).unwrap_or(u32::MAX),
                selector_count: identity.selector_count,
                owner_count: identity.owner_count,
                leaf_count: identity.leaf_count,
            },
        );
    }
    let record_count = u32::try_from(records.len())
        .map_err(|_| ExactSelectorGenerationFixtureErrorV1::TooManyRecords)?;
    let mut encoded_records = Vec::with_capacity(records.len());
    for record in &records {
        encoded_records.push(encode_record(record)?);
    }
    let offsets_len = (encoded_records.len() + 1)
        .checked_mul(8)
        .ok_or(ExactSelectorGenerationFixtureErrorV1::TooManyRecords)?;
    let records_len = encoded_records.iter().try_fold(0_usize, |length, record| {
        length
            .checked_add(record.len())
            .ok_or(ExactSelectorGenerationFixtureErrorV1::TooManyRecords)
    })?;
    let capacity = FIXED_HEADER_LEN
        .checked_add(identity.language_id.len())
        .and_then(|length| length.checked_add(identity.provider_id.len()))
        .and_then(|length| length.checked_add(offsets_len))
        .and_then(|length| length.checked_add(records_len))
        .ok_or(ExactSelectorGenerationFixtureErrorV1::TooManyRecords)?;
    let mut bytes = Vec::with_capacity(capacity);
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&record_count.to_le_bytes());
    bytes.extend_from_slice(&identity.owner_count.to_le_bytes());
    bytes.extend_from_slice(&identity.leaf_count.to_le_bytes());
    bytes.extend_from_slice(&identity.selector_count.to_le_bytes());
    bytes.extend_from_slice(&language_len.to_le_bytes());
    bytes.extend_from_slice(&provider_len.to_le_bytes());
    bytes.extend_from_slice(&identity.workspace_root_digest);
    bytes.extend_from_slice(&identity.workspace_identity_digest);
    bytes.extend_from_slice(&identity.parser_identity_digest);
    bytes.extend_from_slice(&identity.query_pack_digest);
    bytes.extend_from_slice(&identity.generation_digest);
    bytes.extend_from_slice(&[0_u8; DIGEST_LEN]);
    bytes.extend_from_slice(identity.language_id.as_bytes());
    bytes.extend_from_slice(identity.provider_id.as_bytes());
    let mut offset = 0_u64;
    bytes.extend_from_slice(&offset.to_le_bytes());
    for record in &encoded_records {
        offset = offset
            .checked_add(
                u64::try_from(record.len())
                    .map_err(|_| ExactSelectorGenerationFixtureErrorV1::TooManyRecords)?,
            )
            .ok_or(ExactSelectorGenerationFixtureErrorV1::TooManyRecords)?;
        bytes.extend_from_slice(&offset.to_le_bytes());
    }
    for record in encoded_records {
        bytes.extend_from_slice(&record);
    }
    let fixture_digest = blake3::hash(&bytes);
    bytes[FIXTURE_DIGEST_OFFSET..FIXTURE_DIGEST_OFFSET + DIGEST_LEN]
        .copy_from_slice(fixture_digest.as_bytes());
    Ok(bytes)
}

pub fn fixture_digest_v1(
    bytes: &[u8],
) -> Result<&[u8; DIGEST_LEN], ExactSelectorGenerationFixtureErrorV1> {
    digest_at(bytes, FIXTURE_DIGEST_OFFSET)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExactSelectorGenerationFixtureErrorV1 {
    DuplicateSelector,
    FixtureMismatch,
    GenerationMismatch,
    IdentityTooLong,
    IncompleteGeneration {
        record_count: u32,
        selector_count: u32,
        owner_count: u32,
        leaf_count: u32,
    },
    InvalidHeader,
    InvalidOffset,
    InvalidProjectionMode(u8),
    InvalidUtf8,
    RangeOutsideProjection,
    TooManyRecords,
}

impl fmt::Display for ExactSelectorGenerationFixtureErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ExactSelectorGenerationFixtureErrorV1 {}

fn encode_record(
    record: &ExactSelectorGenerationRecordV1,
) -> Result<Vec<u8>, ExactSelectorGenerationFixtureErrorV1> {
    if record.structural_selector.is_empty()
        || record.owner_path.is_empty()
        || record.source_byte_range.start > record.source_byte_range.end
    {
        return Err(ExactSelectorGenerationFixtureErrorV1::RangeOutsideProjection);
    }
    let selector_len = u32::try_from(record.structural_selector.len())
        .map_err(|_| ExactSelectorGenerationFixtureErrorV1::TooManyRecords)?;
    let owner_len = u32::try_from(record.owner_path.len())
        .map_err(|_| ExactSelectorGenerationFixtureErrorV1::TooManyRecords)?;
    let projection_len = u32::try_from(record.projection.len())
        .map_err(|_| ExactSelectorGenerationFixtureErrorV1::TooManyRecords)?;
    let mut encoded = Vec::with_capacity(
        RECORD_HEADER_LEN
            + record.structural_selector.len()
            + record.owner_path.len()
            + record.projection.len(),
    );
    encoded.extend_from_slice(&selector_len.to_le_bytes());
    encoded.extend_from_slice(&owner_len.to_le_bytes());
    encoded.extend_from_slice(&projection_len.to_le_bytes());
    encoded.push(record.projection_mode as u8);
    encoded.extend_from_slice(&record.source_byte_range.start.to_le_bytes());
    encoded.extend_from_slice(&record.source_byte_range.end.to_le_bytes());
    encoded.extend_from_slice(&record.owner_subtree_digest);
    encoded.extend_from_slice(&record.source_blob_digest);
    encoded.extend_from_slice(&record.normalized_parser_facts_digest);
    encoded.extend_from_slice(record.structural_selector.as_bytes());
    encoded.extend_from_slice(record.owner_path.as_bytes());
    encoded.extend_from_slice(&record.projection);
    Ok(encoded)
}

fn decode_record(
    record: &[u8],
) -> Result<ExactSelectorGenerationRecordViewV1<'_>, ExactSelectorGenerationFixtureErrorV1> {
    if record.len() < RECORD_HEADER_LEN {
        return Err(ExactSelectorGenerationFixtureErrorV1::InvalidOffset);
    }
    let selector_len = usize::try_from(read_u32(record, 0)?)
        .map_err(|_| ExactSelectorGenerationFixtureErrorV1::InvalidOffset)?;
    let owner_len = usize::try_from(read_u32(record, 4)?)
        .map_err(|_| ExactSelectorGenerationFixtureErrorV1::InvalidOffset)?;
    let projection_len = usize::try_from(read_u32(record, 8)?)
        .map_err(|_| ExactSelectorGenerationFixtureErrorV1::InvalidOffset)?;
    let projection_mode = ExactSelectorProjectionModeV1::from_byte(record[12])?;
    let source_start = read_u64(record, 13)?;
    let source_end = read_u64(record, 21)?;
    let owner_subtree_digest = digest_at(record, 29)?;
    let source_blob_digest = digest_at(record, 61)?;
    let normalized_parser_facts_digest = digest_at(record, 93)?;
    let selector_start = RECORD_HEADER_LEN;
    let selector_end = selector_start
        .checked_add(selector_len)
        .ok_or(ExactSelectorGenerationFixtureErrorV1::InvalidOffset)?;
    let owner_end = selector_end
        .checked_add(owner_len)
        .ok_or(ExactSelectorGenerationFixtureErrorV1::InvalidOffset)?;
    let projection_end = owner_end
        .checked_add(projection_len)
        .ok_or(ExactSelectorGenerationFixtureErrorV1::InvalidOffset)?;
    if projection_end != record.len() {
        return Err(ExactSelectorGenerationFixtureErrorV1::InvalidOffset);
    }
    Ok(ExactSelectorGenerationRecordViewV1 {
        structural_selector: utf8_slice(record, selector_start, selector_end)?,
        owner_path: utf8_slice(record, selector_end, owner_end)?,
        owner_subtree_digest,
        source_blob_digest,
        normalized_parser_facts_digest,
        projection_mode,
        source_byte_range: source_start..source_end,
        projection: record
            .get(owner_end..projection_end)
            .ok_or(ExactSelectorGenerationFixtureErrorV1::InvalidOffset)?,
    })
}

fn digest_at(
    bytes: &[u8],
    start: usize,
) -> Result<&[u8; DIGEST_LEN], ExactSelectorGenerationFixtureErrorV1> {
    bytes
        .get(start..start + DIGEST_LEN)
        .and_then(|digest| digest.try_into().ok())
        .ok_or(ExactSelectorGenerationFixtureErrorV1::InvalidHeader)
}

fn read_u16(bytes: &[u8], start: usize) -> Result<u16, ExactSelectorGenerationFixtureErrorV1> {
    bytes
        .get(start..start + 2)
        .and_then(|value| value.try_into().ok())
        .map(u16::from_le_bytes)
        .ok_or(ExactSelectorGenerationFixtureErrorV1::InvalidHeader)
}

fn read_u32(bytes: &[u8], start: usize) -> Result<u32, ExactSelectorGenerationFixtureErrorV1> {
    bytes
        .get(start..start + 4)
        .and_then(|value| value.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or(ExactSelectorGenerationFixtureErrorV1::InvalidHeader)
}

fn read_u64(bytes: &[u8], start: usize) -> Result<u64, ExactSelectorGenerationFixtureErrorV1> {
    bytes
        .get(start..start + 8)
        .and_then(|value| value.try_into().ok())
        .map(u64::from_le_bytes)
        .ok_or(ExactSelectorGenerationFixtureErrorV1::InvalidHeader)
}

fn utf8_slice(
    bytes: &[u8],
    start: usize,
    end: usize,
) -> Result<&str, ExactSelectorGenerationFixtureErrorV1> {
    std::str::from_utf8(
        bytes
            .get(start..end)
            .ok_or(ExactSelectorGenerationFixtureErrorV1::InvalidOffset)?,
    )
    .map_err(|_| ExactSelectorGenerationFixtureErrorV1::InvalidUtf8)
}

#[cfg(test)]
#[path = "../../tests/unit/exact_selector_generation_fixture.rs"]
mod tests;
